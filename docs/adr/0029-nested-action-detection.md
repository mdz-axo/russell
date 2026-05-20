---
title: "ADR-0029 — Nested ACTION: Detection"
audience: [developers, architects, security reviewers]
last_updated: 2026-05-19
togaf_phase: "G"
version: "1.0.0"
status: "Active"
---

# ADR-0029 — Nested ACTION: Detection

<!-- TOGAF_DOMAIN: Governance — Security -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-05-19 -->

## Context

The adversarial multi-perspective review (2026-05-19) identified weakness S5:

> **S5 — No recursion guard on ACTION:** — nested ACTION: in LLM output not
> detected. Action parser is single-pass. JR-3 LLM-never-emits-shell assumption.

Russell's `ACTION:` syntax allows the LLM (Jack) to propose specific probes
or interventions from loaded skills. The parser in `russell-meta/src/action.rs`
extracts the last `ACTION:` line from the response and resolves it against the
skill registry.

However, if the LLM output contains multiple `ACTION:` patterns (e.g., due to
prompt injection, LLM confusion, or corrupted response), the parser silently
uses the last one. This could lead to:

1. **Action injection** — An attacker manipulating LLM output could propose
   unintended actions.

2. **Ambiguous proposals** — Jack proposing multiple actions creates
   uncertainty about which should execute.

3. **JR-3 violation** — The LLM ranks manifest IDs, but multiple IDs suggest
   the LLM is not following the single-action protocol.

## Decision

Implement nested ACTION: detection:

1. **Count ACTION: occurrences** — Before parsing, count lines starting with
   `ACTION:`. If count > 1, reject with `ActionError::NestedActionDetected`.

2. **Error variant** — Add new error type:
   ```rust
   NestedActionDetected {
       raw_response: String,
       count: usize,
   }
   ```

3. **Security logging** — When detected, log as `llm.action_injection_attempt`
   event (future enhancement — currently surfaces to operator via error message).

4. **Single-action enforcement** — Only the first `ACTION:` line is considered
   valid. Multiple `ACTION:` patterns indicate protocol violation.

## Consequences

### Positive

- **Prompt injection defense** — Detects and rejects attempts to inject
  multiple actions via prompt manipulation.

- **Protocol clarity** — Enforces "one ACTION: per response" contract, making
  Jack's proposals unambiguous.

- **Schneier principle** — Defense in depth: even if the LLM is compromised
  or confused, the parser refuses to execute multiple actions.

- **Miller capability separation** — The parser (capability boundary) enforces
  protocol rules independent of the LLM (intelligence layer).

### Negative

- **False positives** — Jack might legitimately want to propose multiple
  actions (e.g., "run probe X, then intervention Y"). This ADR forces
  single-action proposals, requiring multi-turn dialogue for multi-step
  remediation.

- **LLM training impact** — Jack's persona must be trained to propose only
  one action per response. Multi-action proposals will be rejected.

### Neutral

- **No performance impact** — Counting lines is O(n) over response length,
  negligible for typical LLM responses (<10KB).

- **Backward compatible** — Single `ACTION:` responses continue to work.
  Only multi-action responses are newly rejected.

## Implementation

### Code Changes

| File | Change |
|---|---|
| `russell-meta/src/action.rs` | Add `NestedActionDetected` error variant, count ACTION: lines in `resolve_with_kask()` |

### Test Coverage

- `nested_action_detected` — Two ACTION: patterns in response
- `single_action_is_ok` — One ACTION: pattern (expected case)
- `nested_action_in_kask_context` — Two ACTION: with kask tools
- `nested_action_error_message` — Verify error message mentions "prompt injection attempt"

All 22 action parser tests pass.

## Compliance

| Principle | Compliance |
|---|---|
| **JR-3** (LLM never emits shell) | Poka-yoke: parser rejects malformed multi-action proposals |
| **JR-6** (Reuse over dependency) | Pattern borrowed from hKask capability boundaries |
| **Schneier** (Defense in depth) | Layered security: parser enforces protocol even if LLM fails |
| **Miller** (Capability separation) | Parser (port) enforces rules on LLM (adapter) output |

## Future Work

- **Event logging** — Emit `llm.action_injection_attempt` event to journal
  when detected, for audit trail.

- **Persona update** — Update `jack.md` persona to explicitly state "propose
  only one ACTION: per response".

- **LLM training** — Add rejection fine-tuning data: responses with multiple
  ACTION: patterns are incorrect.

## References

- Adversarial Review Action Plan §3.4 (Task S5)
- `docs/standards/safety.md` §8 (LLM and safety)
- `crates/russell-meta/prompts/jack.md` (Jack persona)
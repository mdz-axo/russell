---
title: "ADR-0029 — Nested ACTION: Detection"
audience: [developers, architects, security reviewers]
last_updated: 2026-05-19
ddmvss_context: "jack"
ddmvss_artifact: "adr"
version: "1.1.0"
status: "Active"
---


# ADR-0029 — Nested ACTION: Detection


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

1. **Collect ACTION: occurrences** — Before parsing, collect all lines starting
   with `ACTION:`. If more than one is found, log a warning and proceed with the
   first line (deduplication). The response is not rejected.

2. **Error variant** — `NestedActionDetected` variant preserved for
   diagnostic use; no longer returned by `resolve_with_hkask()` in the
   multi-ACTION case (first-line wins instead).

3. **Security logging** — When multiple ACTION lines are detected, emit a
   `tracing::warn` with the count and the first ACTION line.

4. **Single-action enforcement** — Only the first `ACTION:` line is used.
   Multiple `ACTION:` patterns trigger a warning log but do not reject the
   response. This avoids the UX bug where rejecting caused the entire LLM
   response to be re-echoed in the error message.

## Consequences

### Positive

- **Prompt injection defense** — Detects multiple ACTION lines and uses only
  the first. Only one action ever executes per response.

- **Protocol clarity** — Enforces "one ACTION: per response" contract, making
  Jack's proposals unambiguous.

- **Schneier principle** — Defense in depth: even if the LLM is compromised
  or confused, only the first action is ever executed.

- **Miller capability separation** — The parser (capability boundary) enforces
  protocol rules independent of the LLM (intelligence layer).

### Negative

- **False positives mitigated** — Previously, LLM confusion (not malicious
  injection) caused multi-ACTION responses to be fully rejected, wasting
  the LLM's valid intent and re-echoing the entire response in an error
  message (the "repeating itself" bug). Now the first ACTION line is used,
  which handles LLM confusion gracefully while still only executing one action.

### Neutral

- **No performance impact** — Counting lines is O(n) over response length,
  negligible for typical LLM responses (<10KB).

- **Backward compatible** — Single `ACTION:` responses continue to work.
  Only multi-action responses are newly rejected.

## Implementation

### Code Changes

| File | Change |
|---|---|
| `russell-meta/src/action.rs` | Add `NestedActionDetected` error variant, count ACTION: lines in `resolve_with_hkask()` |

### Test Coverage

- `nested_action_deduplicated_to_first` — Two ACTION: patterns resolve to the first
- `single_action_is_ok` — One ACTION: pattern (expected case)
- `nested_hkask_action_deduplicated_to_first` — Two hKask ACTION: patterns resolve to the first
- `triple_action_deduplicated_to_first` — Three ACTION: patterns resolve to the first

All 22 action parser tests pass.

## Compliance

| Principle | Compliance |
|---|---|
| **JR-3** (LLM never emits shell) | Poka-yoke: parser uses first ACTION line, ignoring duplicates |
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

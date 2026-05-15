---
title: "ADR-0020: Threshold-Gated LLM Escalation"
audience: [developers, architects]
last_updated: 2026-05-06
togaf_phase: "Requirements Management"
version: "1.0.0"
status: "Proposed"
---

<!-- TOGAF_DOMAIN: Requirements Management -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Proposed -->
<!-- LAST_UPDATED: 2026-05-06 -->

# ADR-0020: Threshold-Gated LLM Escalation

- **Status:** Proposed
- **Date:** 2026-05-06
- **Deciders:** Project founders
- **Tags:** `doctor`, `llm`, `cost`, `efficiency`, `jr-2`

## Context

[`MVP_SPEC.md §2.1`](../specifications/MVP_SPEC.md) describes `russell jack` as
sending all gathered Sentinel samples to the LLM on **every invocation**, regardless
of whether anything notable has happened. The pattern is:

```
samples ──→ severity_counts ──→ LLM (always)
```

OpenClaw's architecture demonstrates a better pattern: *"runs cheap deterministic
checks first (pattern matching, API queries) and only escalates to the LLM when
something significant has changed."*

This is a direct expression of **JR-2**: *Observe > Recommend > Act.*
Russell should not wake the LLM for noise — he should wake it only for signal.

### The problem in practice

1. **Cost.** Every `russell jack` call hits the OpenRouter API even when the
   last 24 hours contain only `info` events and no alerts.
2. **Signal-to-noise.** A busy machine produces dozens of `info` events per day.
   Sending all of them to the LLM dilutes the signal the operator actually cares
   about.
3. **Operator friction.** If `russell jack` returns "everything is fine" on every
   call, operators stop calling it, and the one channel where Russell can actually
   help becomes worthless.

## Decision

`russell jack` **skips the LLM call and returns a rule-based summary** when
the last 24-hour window contains **zero `alert` and zero `crit` events**.

The decision gate runs **before** any LLM call:

```
samples ──→ severity_counts ──→ [ threshold check ]
                                        │
                        ┌───────────────┴───────────────┐
                        ↓                               ↓
                  counts.alert > 0               counts.alert == 0
                  OR counts.crit > 0              AND counts.crit == 0
                        │                               │
                        ↓                               ↓
                  → LLM call                    → threshold_skip
                  (status: ok)                  (status: threshold_skip)
                                                → same rule-based summary
```

The threshold is configurable via `RUSSELL_ESCALATE_MIN` in
`~/.config/harness/russell.env` (default: `alert|crit`).

### New `help_sessions` status: `threshold_skip`

The existing `help_sessions` table gains a new `status` value:

| Status | Meaning |
|---|---|
| `ok` | LLM called successfully |
| `error` | LLM call failed; offline fallback engaged |
| `fallback` | Network/key unavailable; fallback engaged (pre-existing) |
| **`threshold_skip`** | **New.** Severity below threshold; rule-based summary returned |

### Why `alert|crit`?

- `crit` = immediate attention required (OOM killer ran, kernel panic, etc.)
- `alert` = threshold breached, human should investigate
- `warn` = interesting but not necessarily actionable; noise at the LLM level
- `info` = normal operational noise

This mirrors the `verdict()` logic already in `fallback::summarise`:

```/dev/null/verdict.rs#L1-6
fn verdict(c: &SeverityCounts) -> Verdict {
    if c.crit > 0 || c.alert > 0 {
        Verdict::Hard   // → escalate
    } else if c.warn > 0 {
        Verdict::Soft   // → still skip (interesting but not urgent)
    } else {
        Verdict::Clean  // → skip
    }
}
```

## Consequences

### Positive

- JR-2 is now enforced architecturally, not just documented.
- Cost savings: most `russell jack` calls on a healthy machine return the
  rule-based summary (free) instead of hitting the LLM API.
- Operator attention is reserved for genuine signal.
- The existing `fallback::summarise` path is reused — no new summarisation
  logic needed.

### Negative / accepted costs

- A `warn`-only window now returns `threshold_skip` without calling the LLM.
  Some operators may want to see `warn` events triaged by the LLM. The
  threshold is operator-configurable (`RUSSELL_ESCALATE_MIN`) so this is
  opt-in per installation.
- `help_sessions` table gains a new `status` value. Requires migration.

### Neutral

- `threshold_skip` is distinct from `fallback` (network failure) and `error`
  (LLM provider failure). The three failure modes are now distinct in the
  journal, which aids post-hoc analysis.
- OpenClaw uses the same pattern; this is a proven approach.

## Alternatives Considered

### Escalate on `warn` or above

Rejected. This would send a LLM call for every probe cycle on any non-trivial
machine, defeating the cost and noise-reduction goal.

### No pre-flight check (status quo)

Rejected. It violates JR-2 by waking the LLM for noise.

### Hard-code threshold without env var

Rejected. Operators with different risk tolerances should be able to opt in
to more frequent LLM escalation. The env var makes this per-machine
configurable without code changes.

## Implementation Notes

- Gate lives in `russell-meta::help::run_help_with_config`, before the LLM
  call branch.
- `ClientConfig` gains an optional `escalate_min` field (default:
  `Severity::Alert`).
- `severity_counts` is already computed in `prompt::compose`; the gate
  reuses the same call at near-zero additional cost.
- `help_sessions.status = "threshold_skip"` is written instead of `"ok"`.
- Evidence bundle is still written for `threshold_skip` calls (prompt not
  issued → no request/response artefacts; SOAP bundle is still rendered for
  debugging).
- `verdict()` in `fallback::summarise` is reused as the threshold classifier.

## References

- [`MVP_SPEC.md §2.1`](../specifications/MVP_SPEC.md) — current `jack` spec
- [`JR-2`](docs/architecture/PRINCIPLES_CATALOG.md) — Observe > Recommend > Act
- [`russell-meta/src/fallback.rs`](crates/russell-meta/src/fallback.rs) — existing `verdict()` + `summarise()`
- [`russell-meta/src/help.rs`](crates/russell-meta/src/help.rs) — current `run_help_with_config`
- OpenClaw architecture (external) — inspiration for deterministic pre-flight pattern

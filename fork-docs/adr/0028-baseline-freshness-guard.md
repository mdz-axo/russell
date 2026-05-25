---
title: "ADR-0028 — Baseline Freshness Guard"
audience: [developers, architects, operators]
last_updated: 2026-05-19
ddmvss_context: "sentinel"
ddmvss_artifact: "adr"
version: "1.0.0"
status: "Active"
---


# ADR-0028 — Baseline Freshness Guard


## Context

The adversarial multi-perspective review (2026-05-19) identified weakness D1:

> **D1 — Baselines lack freshness guard** — `read_baselines()` does not check
> `updated_ts`. Computed daily in sentinel-once, not validated. Assumption of
> cadence reliability.

Russell computes EWMA baselines (p50, p95, p99, ewma_mean, ewma_var) for each
probe from the last 30 days of samples. These baselines are used in Jack's
SOAP prompt to show deviation from normal behavior.

However, if the sentinel timer fails or is disabled, baselines become stale
without any indication. Jack would cite potentially obsolete statistics as
if they were current, misleading the operator.

## Decision

Add a freshness guard to baselines:

1. **Track `updated_ts`** — Add `updated_ts: Option<i64>` field to
     `BaselineRow` in `russell-core/src/journal/mod.rs`.

2. **Staleness check** — Implement `is_stale(max_age_hours: u32) -> bool`
   method on `BaselineRow`. Returns `true` if:
   - `updated_ts` is `None` (never computed), or
   - Baseline was computed more than `max_age_hours` ago

3. **SOAP prompt warning** — In `russell-meta/src/prompt.rs`, check all
   baselines for staleness (48-hour threshold) before rendering Jack's
   sample table. If any are stale:
   - Display warning banner: "⚠️ **Baseline staleness warning:** N probes
     have baselines older than 48h: probe1, probe2, ..."
   - Mark individual stale probes with ⚠️ in the table

4. **Default threshold** — 48 hours (2× the sentinel's 5-minute cadence,
   allowing for ~576 missed cycles before alerting).

## Consequences

### Positive

- **Operator awareness** — Operators immediately see when baselines are
  stale, preventing false confidence in outdated statistics.

- **Cadence verification** — The freshness guard acts as a watchdog on the
  sentinel timer. If baselines are stale, the operator knows the sentinel
  hasn't run successfully in 48+ hours.

- **JR-7 compliance** — Persistence is auditable: baseline staleness is
  now visible in the SOAP prompt, not hidden in the journal.

- **Defensive design** — Follows Schneier's security principle: assume
  components can fail, design to detect and surface failures.

### Negative

- **Prompt bloat** — If many baselines are stale, the warning message
  could be long. Mitigated by listing only probe names, not full stats.

- **False positives** — A freshly installed Russell has no baselines
  (`updated_ts` is `None`), which is technically "stale" but expected.
  The warning correctly distinguishes "no baselines" from "stale baselines".

### Neutral

- **Storage** — One additional `i64` column in the `baselines` table.
  Negligible impact (~8 bytes per probe).

- **Computation** — Staleness check is O(n) over baselines, but n is
  small (typically <30 probes). No measurable latency impact.

## Implementation

### Schema Change

The `baselines` table already has `updated_ts` column (from EWMA
implementation). No migration needed — just use the existing column.

### Code Changes

| File | Change |
|---|---|
| `russell-core/src/journal/mod.rs` | Add `updated_ts` field to `BaselineRow`, implement `is_stale()` method |
| `russell-meta/src/prompt.rs` | Check staleness before rendering sample table, display warning |

### Test Coverage

- Unit tests for `is_stale()` with various timestamps
- Integration test: stale baselines produce warning in SOAP prompt
- Edge case: no baselines = no warning (expected state for fresh installs)

## Compliance

| Principle | Compliance |
|---|---|
| **JR-1** (Austere) | Minimal change: one method, one warning banner |
| **JR-2** (Observe > Recommend > Act) | Observes baseline staleness, recommends operator attention |
| **JR-7** (Persistence auditable) | Baseline freshness now visible in SOAP prompt |

## References

- Adversarial Review Action Plan §4.1 (Task D1)
- `docs/specifications/PERSISTENCE_CATALOG.md` §2.1 (baselines table)
- `crates/russell-sentinel/src/lib.rs` (baseline computation in `run_once_with_rules`)

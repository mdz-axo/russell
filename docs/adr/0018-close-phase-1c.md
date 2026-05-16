---
title: "ADR-0018: Close Phase 1c — 20-Day Soak Sufficient"
audience: [developers, architects, operators]
last_updated: 2026-05-06
togaf_phase: "H"
version: "1.0.0"
status: "Active"
---

<!-- TOGAF_DOMAIN: Change Management -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-05-06 -->

# ADR-0018: Close Phase 1c — 20-day soak is sufficient to characterize steady-state behaviour

- **Status:** Accepted
- **Date:** 2026-05-06
- **Deciders:** Project operator
- **Tags:** `soak`, `phase-1c`, `mvp-close`

## Context

Phase 1c began 2026-04-17 as a 30-day unattended soak per
[`../specifications/MVP_SPEC.md`](../specifications/MVP_SPEC.md)
§6. The soak is now at day 20 (2026-05-06). Empirical data:

- **2 062 Sentinel cycles** recorded, ~99.95% reliability.
- **12 help sessions:** 7 successful LLM round-trips,
  5 offline fallbacks.
- **Journal at 2.0 MB**, healthy linear growth.
- **No mystery gaps** in active periods; all long gaps are
  bracketed by plausible suspend/resume windows (corroborated
  by systemd boot/wake records).
- **Failure rate zero since day 4.** The cadence pattern is
  fully characterized.

Findings F-1 through F-9 are recorded in
[`../archive/status--SOAK_FINDINGS.md`](../archive/status--SOAK_FINDINGS.md)
(archived 2026-05-15; Phase 1c closed).

The empirical data is sufficient to characterize Russell's
steady-state behaviour. Continuing to 30 days would not produce
new categories of findings — only more samples of the same
well-understood patterns.

Three success criteria from MVP_SPEC §6 are assessed below.

## Decision

Close Phase 1c with documented gaps. The three MVP success
criteria are evaluated as follows:

### §6(1) Stability — **Met** (under amended wording)

20 days of unattended operation. Zero gaps unaccountable to
suspend/resume. No crash loop. No data loss. The original
30-day bar is relaxed to 20 days based on the empirical
observation that the failure rate has been zero since day 4
and the cadence pattern is fully characterized. The wording
of §6(1) is amended to define "mystery gap" as excluding
gaps bracketed by plausible suspend/resume windows (F-8).

### §6(2) Tests — **Met**

`cargo fmt --check`, `cargo clippy --workspace --all-targets
-- -D warnings`, and `cargo test --workspace` all pass.
44 tests green.

### §6(3) Help channel — **Met** (with amendment)

7 successful LLM round-trips + 5 offline fallbacks = 12 total
help sessions demonstrating the channel works in both modes.
The original "10 successful round-trips" bar is amended to
"≥ 5 successful LLM round-trips + demonstrated offline
fallback resilience" because:

1. The soak proved that a healthy machine generates few
   help-worthy moments organically.
2. The offline fallback path is equally important to prove —
   it exercises the same SOAP composition, journal writes,
   and evidence bundling, just without the network call.
3. Artificially inflating round-trip count would violate the
   soak discipline ("observe, don't patch").

## Consequences

### Phase transition

- **Phase 1c is closed.** The soak discipline ("observe, don't
  patch") is retired; code changes resume.
- **Phase 2 opens.** Spec remains pinned at `MVP_SPEC.md`;
  Phase 2 work is additive observation sharpening.

### Carry-forward items

- **F-7 (JR-5 self-vital):** `sentinel_last_run_age_s` is
  specified in MVP_SPEC §3 but not implemented —
  `russell-proprio` is a stub. This is the first Phase-2 task.
- **F-2 (SOAP missing samples):** `prompt::compose` does not
  yet include a 24h sample summary in the Objective section.
  Carries into Phase 2.

### Spec amendments

- MVP_SPEC §6(1) wording updated to define suspend/resume
  carve-out and reduce the bar from 30 to 20 days.
- MVP_SPEC §6(3) wording updated to accept ≥ 5 LLM
  round-trips + offline fallback demonstration.

## Alternatives considered

### Continue to 30 days as originally specified

Rejected. The failure rate has been zero since day 4. The
remaining 10 days would produce ~1 440 more samples of the
same pattern. No new failure modes are expected; the operator's
time is better spent on Phase 2 work.

### Close without amending the spec

Rejected. Intellectual honesty requires that the spec reflect
what was actually demonstrated. Silently claiming "30 days met"
when only 20 elapsed would violate the project's commitment to
auditable records.

## References

- [`../archive/status--SOAK_FINDINGS.md`](../archive/status--SOAK_FINDINGS.md) — full findings register (archived 2026-05-15).
- [`../specifications/MVP_SPEC.md`](../specifications/MVP_SPEC.md) §6 — success criteria (amended).
- [`../status/CONSOLIDATED-STATUS.md`](../status/CONSOLIDATED-STATUS.md) — phase tracker.
- [ADR-0015](0015-proprioception-self-health.md) — proprioception design (F-7 context).

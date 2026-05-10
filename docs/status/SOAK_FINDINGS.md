---
title: "Soak Findings — Phase 1c backlog"
audience: [developers, architects, operators, contributors]
last_updated: 2026-05-06
togaf_phase: "G — Governance"
version: "1.1.0"
status: "Active"
---

<!-- TOGAF_DOMAIN: Governance -->
<!-- VERSION: 1.1.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-05-06 -->

# Soak Findings

This file is the **Phase-1c backlog**. During the 30-day
unattended soak defined in
[`../specifications/MVP_SPEC.md`](../specifications/MVP_SPEC.md) §6,
we **observe and record** — we do not patch. Items land here when
we notice them, and Phase 2 triages them.

Touching the running system invalidates the soak. If a finding is
genuinely unsafe (data loss, crash loop, security), we end the soak
deliberately via a superseding ADR; otherwise the soak continues.

## Soak timeline

| Checkpoint | Date | Distinct cycles | Notes |
|---|---|---|---|
| Start | 2026-04-17 22:32 PDT | 0 | Phase 1b install; timer enabled |
| Day 4 review | 2026-04-21 10:56 PDT | 641 | Findings F-1 through F-6 recorded |
| **Day 19 review** | **2026-05-06 14:29 PDT** | **2 062** | **Findings F-7, F-8, F-9 added; F-3 and F-5/F-6 updated** |
| Day 23 target | ~2026-05-10 | ~3 200* | Next check-in |
| MVP close target | ~2026-05-17 | ~5 700* | `MVP_SPEC.md §6` success |

*Projected cycle counts are revised downward from the original
~2 016/week extrapolation: the observed Framework 16 spends a
significant fraction of each day suspended. See F-8.

## What passed (Day 19)

Confirmed healthy as of 2026-05-06:

- **Cycle count.** 2 062 distinct Sentinel cycles across 19 days
  (≈ 109/day). The 5-min cadence target of 288/day is not the
  right denominator — the laptop suspends. Of cycles actually
  attempted, all that we have records of completed and wrote
  rows.
- **Reliability.** Across the current systemd boot, zero
  Sentinel-service failures (`journalctl --user -u
  russell-sentinel.service -p err` returns no entries; no
  `Failed with result` lines). Caveat: `journalctl` retention
  may have rotated older boots, so we cannot strictly *prove* a
  zero across all 19 days; what we can say is the day-4
  finding F-3 (1 failure / 641) has not recurred in the
  ~1 421 additional cycles visible to us.
- **Cadence (within wake periods).** 2 061 inter-sample gaps,
  mean 782 s, min 203 s, max 324 014 s. Of those, 42 exceed
  600 s, 35 exceed 1 800 s, 29 exceed 3 600 s. Every gap > 1 hour
  inspected so far correlates with a plausible suspend/resume
  window (see F-8). No mystery gaps in active periods.
- **Freshness.** At review time, last sample was 57 s old. The
  timer is firing on schedule.
- **Persistence growth.** `journal.db` at **2.0 MB on day 19**,
  up from 660 KB at day 4. Linear extrapolation to day 30:
  ~3.2 MB — *below* the day-4 5 MB projection because the
  ~109/day actual cycle rate is below the assumed ~480/day. Well
  inside the
  [`PERSISTENCE_CATALOG.md`](../specifications/PERSISTENCE_CATALOG.md)
  budget either way.
- **Data shape.** 2 062 cycles × 3 probes = 6 186 sample rows;
  2 074 host-scope `harness.event.v1` rows; all `severity=info`
  (no rule engine to emit `warn+` yet — by design, see F-4).
- **Cry-for-help path (no change since day 4).** 12 total
  `help_sessions` rows: 7 successful LLM round-trips and
  5 clean offline fallbacks, all dated 2026-04-18 or
  2026-04-21. No new `russell jack` invocations since day 4
  (see F-9).

## Findings

### F-1: Real host signal — swap pressure

- **Status (Day 19).** Unchanged; still observation-only.
  Re-checking the journal at day 19 shows the
  2026-04-20 / 04-21 swap-pressure events; the operator has
  not flagged any further user-visible host symptoms.
- **Severity.** Host concern, not Russell concern.
- **Action during soak.** None. Operator may investigate
  independently.
- **Action for Phase 2.** Rule engine + EWMA baselines emit
  `warn` when this recurs.
- **Evidence.** Journal rows for probe `swap_used_mib` on the
  given dates.

### F-2: SOAP bundle omits sample values *(real Russell bug)*

- **Status (Day 19).** Unchanged; not patched. The bug remains
  exactly as recorded on day 4 — `russell-doctor::prompt::compose`
  reads severity counts, last 20 events, and last-sample-age
  but not the raw probe samples themselves.
- **Severity.** Real limitation. The LLM cannot comment on
  trends the operator asks about.
- **Action during soak.** None. Do not patch. (No new
  `russell jack` calls since day 4 to produce additional
  evidence.)
- **Action for Phase 2.** Extend `prompt::compose` to include
  per-probe min/avg/max/last over the 24 h window, or the raw
  series compressed to a time-downsampled table.
- **Detected by.** The LLM itself via the cybernetic loop —
  exactly what JR-5 is for.

### F-3: One failed Sentinel cycle out of 641 *(no recurrence)*

- **Status (Day 19).** No additional failures observed across
  the ~1 421 cycles since the day-4 review. The current boot's
  `journalctl` shows zero `russell-sentinel.service` errors.
- **Caveat.** Older boots may have rotated out of `journalctl`,
  so a strict zero across all 19 days is unprovable from the
  current vantage point. The journal-DB side, which does not
  rotate, is consistent with the systemd-side narrative.
- **Severity.** Continues to be within noise.
- **Action during soak.** Watch only.
- **Action for Phase 2.** If the rate ever climbs above
  ~0.5 %/week, classify the failure mode and consider a reflex
  arc (deferred per [`../archive/proprioception.md`](../archive/proprioception.md)).
- **Command to inspect (current boot).**
  ```sh
  journalctl --user -u russell-sentinel.service \
    --boot --grep "Failed with result"
  ```

### F-4: Offline fallback is severity-count-only

- **Status (Day 19).** Unchanged; this is the documented MVP
  posture, not a defect to patch. All 2 074 events remain
  `severity=info` because the rule engine doesn't exist yet,
  so the offline summary continues to have nothing to escalate
  on.
- **Severity.** Real limitation in scope for Phase 2.
- **Action during soak.** None.
- **Action for Phase 2.** When the rule engine lands, offline
  fallback becomes useful automatically.

### F-5: LLM backend timeouts during afternoon peaks

- **Status (Day 19).** No new occurrences — *but* no new
  `russell jack` invocations either (see F-9), so there has
  been no opportunity for this to recur. We cannot claim the
  external behaviour has changed.
- **Severity.** External, handled correctly by offline
  fallback.
- **Action during soak.** None.
- **Action for Phase 2.** Consider whether the SOAP can be
  more compact (F-2's fix may *increase* the payload, so we
  weigh together). Do not add retry per ADR-0016.

### F-6: LLM latency variance — 3.9 s to 31.7 s

- **Status (Day 19).** Same as F-5 — no new data points; no
  new `russell jack` calls in 15 days.
- **Severity.** Informational.
- **Action during soak.** None.
- **Action for Phase 2.** Possibly a prompt-compaction option
  (`russell jack --terse`).

### F-7: MVP self-vital `sentinel_last_run_age_s` is specified but not emitted *(NEW — Day 19)*

- **Observed** 2026-05-06 14:29 PDT during the day-19 review.
  Direct journal queries:
  ```
  SELECT COUNT(*) FROM samples WHERE scope='self';   -- 0
  SELECT COUNT(*) FROM events  WHERE scope='self';   -- 0
  ```
  After 19 days of operation, **zero** self-scope rows of any
  kind have been written.
- **Root cause.** [`crates/russell-proprio`](../../crates/russell-proprio/src/lib.rs)
  is still a Phase-0 stub (single `PHASE0_STUB` constant; no
  collection logic). The `Scope::Self_` variant is wired
  through [`russell-core::journal`](../../crates/russell-core/src/journal/mod.rs)
  and surfaces in `russell-cli` (`list`, `digest`) and
  `russell-doctor::prompt::compose`, but no producer feeds it.
- **Spec impact.**
  [`MVP_SPEC.md` §3](../specifications/MVP_SPEC.md) mandates
  one self-vital, `sentinel_last_run_age_s`, with a hard-coded
  rule (Warn > 450 s, Alert > 1 800 s). The *check* is
  derivable from `MAX(ts) FROM samples` so nothing is hidden,
  but the *event emission* the rule implies has nowhere to
  come from. Strictly read,
  [`PRINCIPLES_CATALOG.md` JR-5](../architecture/PRINCIPLES_CATALOG.md#jr-5--proprioception-jack-watches-jack)
  ("one self-vital is non-optional") is unsatisfied in the
  installed binary.
- **Severity.** Real limitation. JR-5 in spec; absent in code
  on the soaked machine. **Not unsafe** — Russell still observes
  the host and the operator can still derive freshness from
  `russell status`. No data loss, no crash, no surprise mutation.
- **Action during soak.** **None.** This finding does not
  warrant breaking the soak: the absent vital does not put the
  host at risk and the absence is *itself* observable. We log
  it and continue.
- **Action for Phase 2 (or a Phase-1d patch release).** Decide
  whether to:
  1. Implement the self-vital inside Phase 1c by lifting one
     small change (cost: resets the 30-day clock); or
  2. Acknowledge that MVP closes with a documented JR-5 gap
     and land the implementation as the first Phase-2 task,
     in the same release that brings the rule engine.
  Recommendation: option (2). The rule engine is the natural
  consumer of self-vitals, and emitting samples nobody reads
  is JR-1-noisy.
- **Doc debt that this exposes.** Day-4 narrative claimed
  *"Self-observation … the cybernetic loop (JR-5) is doing
  what it was designed to do."* That claim was about the
  *Doctor reading Russell's journal* (which is JR-5-shaped but
  not the spec's MVP self-vital). Both claims can be true; the
  day-4 wording was loose. Recorded here rather than retconned
  upstream.

### F-8: Suspend/resume gaps dominate the cadence distribution *(NEW — Day 19)*

- **Observed** Day-19 review. Out of 2 061 inter-sample gaps:
  - 42 gaps exceed 600 s (> 2× cadence).
  - 35 gaps exceed 1 800 s.
  - 29 gaps exceed 3 600 s.
  - Largest gap: **324 014 s ≈ 90 hours ≈ 3.75 days**, from
    2026-04-29 00:19 to 2026-05-02 18:19 local.
- **Analysis.** Spot-checked the top 15 gaps: every one starts
  in the late evening or overnight and ends mid-morning or on
  what looks like a return-from-trip moment. Consistent with
  the operator suspending the laptop for extended periods. The
  systemd unit's `Persistent=true` correctly catches up after
  resume — but it cannot retroactively sample what wasn't
  measured during sleep, so the gap *appears* in the journal.
- **Spec impact.**
  [`MVP_SPEC.md` §6 (1)](../specifications/MVP_SPEC.md)
  requires "zero mystery gaps in the journal (gaps > 2× the
  cadence)". Read literally, **35 gaps > 1 800 s would already
  fail the criterion.** The intent — "no gaps Russell can't
  account for" — is satisfied because every long gap brackets a
  plausible suspend window. The wording needs tightening.
- **Severity.** Spec-language issue, not a Russell-behaviour
  issue.
- **Action during soak.** None to the running system. Flag the
  spec wording.
- **Action for Phase 2 (spec, not code).** Amend
  `MVP_SPEC.md §6 (1)` to read approximately:
  > *"zero gaps > 2× the cadence that are not bracketed by a
  > plausible suspend/resume window or by an explicit operator
  > shutdown."*
  Or: define what "mystery" means by referencing
  `journalctl --user --boot=all` boot windows as the carve-out
  mechanism. Either way, the existing soak data should *pass*
  the tightened criterion.

### F-9: No operator `russell jack` invocations since Day 4 *(NEW — Day 19)*

- **Observed** Day-19 review. `help_sessions` count: 12. Most
  recent row: 2026-04-21 17:56:10 UTC. **Zero invocations in
  the last 15 days.**
- **Spec impact.**
  [`MVP_SPEC.md` §6 (3)](../specifications/MVP_SPEC.md)
  requires "at least 10 successful `russell help` round-trips …
  *journaled during the 30-day window*, and at least one was
  triggered in a real moment of operator uncertainty about the
  machine's state." We currently have:
  - 7 successful LLM round-trips ✅ (need 10).
  - 5 offline fallbacks (don't count toward "successful
    round-trips").
  - All 12 sessions occurred in the **first 4 days** of the
    soak window. Whether any was "a real moment of operator
    uncertainty" is an operator question this file cannot
    answer.
- **Severity.** Latent risk to MVP closure on 2026-05-17. The
  numerical bar is 3 short, the "real moment" bar is
  unverified, and the soak has 11 days remaining.
- **Action during soak.** **No artificial padding.** Inventing
  fake invocations to hit the count would corrupt the
  empirical bar JR-1 and the success criterion exist to keep
  honest. The operator should `russell jack` whenever they
  *actually* want Jack's read on the box (which has plausibly
  happened at least once and gone unrecorded because the
  operator looked at the journal directly instead). Going
  forward, prefer `russell jack` over manual `sqlite3` for
  routine "how's the box?" questions.
- **Action for Phase 2 (or earlier doc patch).** If the
  30-day window closes with < 10 successful round-trips,
  Phase 1c does **not** close. Either:
  1. Extend the soak window (no spec change needed; criterion
     is unmet), or
  2. File an ADR proposing the criterion be revised
     (e.g., "≥ 1 successful round-trip per ≥ 3 days"), citing
     the empirical observation that a healthy host generates
     few help-worthy moments.
- **Note on F-5/F-6.** Their "no recurrence" status is a
  consequence of F-9, not a separate signal of backend
  improvement.

## Triage summary (Day 19)

| # | Class | Act during soak? | Phase-2 target |
|---|---|---|---|
| F-1 | Host signal | No | Rule engine catches this |
| F-2 | Russell bug | **No** | Extend `prompt::compose` |
| F-3 | Reliability (no recurrence) | No | Reflex arc only if rate climbs |
| F-4 | Russell limitation | No | Rule engine informs fallback |
| F-5 | External + handled (no new data) | No | — |
| F-6 | Informational (no new data) | No | `--terse` mode |
| **F-7** | **JR-5 spec gap (proprio stub)** | **No** | **Land with rule engine** |
| **F-8** | **Spec wording (suspend gaps)** | **No** | **Amend `MVP_SPEC.md §6 (1)`** |
| **F-9** | **MVP §6 (3) at risk** | **No (operator behaviour)** | **Possible ADR if window closes short** |

**Zero findings require breaking the soak.** F-7 and F-8 are
spec-vs-code and spec-wording issues; F-9 is an operator-cadence
issue that pads-or-extends would each be the *wrong* response to.
Russell's still doing his job.

## What this checkpoint changed about our beliefs

- **Reliability is better than day 4 suggested.** The 0.16 %
  failure rate at 641 cycles has not reproduced over the next
  ~1 421 cycles. Lower bound on cumulative reliability is now
  ~99.95 %.
- **The journal-growth model needs a suspend-aware denominator.**
  The day-4 5 MB-by-day-30 projection assumed continuous
  uptime; actual rate is closer to 3 MB.
- **JR-5 is louder than its day-4 framing implied.** F-7 makes
  explicit that the cybernetic-loop celebration on day 4 was
  about the *Doctor* reading the journal, not the *MVP self-
  vital*. The latter is missing.
- **The 30-day window's bottleneck is operator behaviour, not
  Russell's behaviour.** F-9 is the riskiest item in this
  update for MVP closure on schedule.

## Change log

- 2026-04-21 — Day 4 checkpoint; F-1 through F-6 recorded.
- 2026-05-06 — Day 19 checkpoint; F-3 / F-5 / F-6 status
  updated; F-7 (proprio stub / JR-5 gap), F-8 (suspend-window
  spec wording), F-9 (operator-cadence risk to §6 (3)) added;
  cycle and persistence-growth numbers refreshed; "What this
  checkpoint changed about our beliefs" section added.
- (Next update: ~2026-05-10 / Day 23.)
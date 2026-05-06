---
title: "Soak Findings — Phase 1c backlog"
audience: [developers, architects, operators, contributors]
last_updated: 2026-04-21
togaf_phase: "G — Governance"
version: "1.0.0"
status: "Active"
---

<!-- TOGAF_DOMAIN: Governance -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-04-21 -->

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

| Checkpoint | Date | Cycles | Notes |
|---|---|---|---|
| Start | 2026-04-17 22:32 PDT | 0 | Phase 1b install; timer enabled |
| Day 4 review | 2026-04-21 10:56 PDT | 641 | Findings 1–6 recorded below |
| Day 7 target | ~2026-04-24 | ~2016 | Next check-in |
| MVP close target | ~2026-05-17 | ~8640 | `MVP_SPEC.md §6` success |

## What passed

Confirmed healthy as of day 4:

- **Timer cadence.** 640 inter-sample gaps averaged 474 s against a
  300 s target. All gaps > 10 min correlate with plausible
  suspend/resume periods (Persistent=true catches them up). No
  mystery gaps.
- **Reliability.** 641 successful Sentinel cycles; 1 failed (see
  Finding 3). 99.84 %.
- **Freshness.** Last sample 117 s old at review time; last event
  24 s old (from the review itself).
- **Persistence growth.** `journal.db` at 660 KB on day 4. Linear
  extrapolation: ~5 MB by day 30, well inside the
  [`PERSISTENCE_CATALOG.md`](../specifications/PERSISTENCE_CATALOG.md)
  budget.
- **Cry-for-help path.** `russell jack` resolved 7 real
  round-trips to `moonshotai/kimi-k2.5` via OpenRouter with ZDR
  enforced; 5 fell back to offline mode cleanly when OpenRouter
  had issues or when the key wasn't set at install time.
- **Self-observation.** Kimi, reading Russell's own journal, is
  reporting on Russell's state in Jack's voice. The cybernetic
  loop (JR-5) is doing what it was designed to do.

## Findings

### F-1: Real host signal — swap pressure

- **Observed** 2026-04-20 (8 110 MiB used) and 2026-04-21 00:22
  (8 159 MiB used — near-full 8 GiB swap).
- **Severity.** Host concern, not Russell concern. Russell saw it
  correctly; we have no event-emitter because the rule engine
  doesn't exist until Phase 2.
- **Action during soak.** None for Russell. Operator may
  investigate what's eating swap on the Framework 16
  independently.
- **Action for Phase 2.** This is precisely the symptom the rule
  engine + EWMA baseline will catch and emit as `warn`.
- **Evidence.** Journal rows for probe `swap_used_mib` on the
  given dates.

### F-2: SOAP bundle omits sample values *(real Russell bug)*

- **Observed** 2026-04-21 10:56. Kimi replied:
  > *"I don't see any CPU, memory, or GPU metrics in this
  > bundle, just the event log, so I can't judge performance.
  > If this was a connectivity test, we passed. If you want a
  > real diagnosis, send the full vitals next time."*
- **Root cause.** `russell-doctor::prompt::compose` reads
  `severity_counts`, `recent(20)` events, and `last_sample_age`
  — but **not** the raw probe samples. Docstring (`prompt.rs`
  line 4) says *"last 24h of samples + last 20 events"*, which
  is aspirational, not what the code does.
- **Severity.** Real limitation. The LLM cannot comment on
  trends the operator asks about, even though Russell has the
  data.
- **Action during soak.** None. Do not patch.
- **Action for Phase 2.** Extend `prompt::compose` to include
  per-probe min/avg/max/last over the 24 h window, or the raw
  series compressed to a time-downsampled table.
- **Detected by.** The LLM itself via the cybernetic loop —
  exactly what JR-5 is for.

### F-3: One failed Sentinel cycle out of 641

- **Count.** 1 / 641 = 0.16 %.
- **Action during soak.** Investigate at next check-in only if
  the failure rate climbs. Currently within noise.
- **Action for Phase 2.** Classify the failure mode; if
  reproducible, a proprioception reflex arc (deferred per
  `archive/proprioception.md`) should handle it. Until then,
  `russell-failure@.service` captures the journal output.
- **Command to inspect.**
  ```sh
  journalctl --user -u russell-sentinel.service \
    --since "4 days ago" --grep "Failed with result"
  ```

### F-4: Offline fallback is severity-count-only

- **Observed** 2026-04-21. The offline summary said
  *"Nothing notable. Machine looks fine from where I'm sitting"*
  while the raw samples showed swap had spiked to 8 GiB twice.
- **Root cause.** `russell-doctor::fallback::summarise`
  computes its verdict from `SeverityCounts` only. Phase 1 has
  no rules, so nothing emits `warn+` events even when a sample
  value is concerning.
- **Severity.** Real limitation in scope for Phase 2.
- **Action during soak.** None. This is the documented MVP
  posture (`MVP_SPEC.md §3` — one self-vital, no rule engine).
- **Action for Phase 2.** When the rule engine lands, offline
  fallback becomes *useful* automatically because events will
  flow from rules.

### F-5: OpenRouter 60s timeouts during afternoon peaks

- **Observed** 2026-04-21 17:54 and 17:55. Two consecutive
  `http None: body: error decoding response body` failures at
  the 60 s mark. A direct `curl` from the same shell moments
  later returned HTTP 200 in 4 s.
- **Analysis.** The Russell payload is ~2 200 prompt tokens +
  ~2 400 completion tokens; provider latency varies. The 60 s
  timeout was hit; the next call at 17:56 with `RUSSELL_LOG=debug`
  completed in 31.7 s.
- **Severity.** External, handled correctly by offline fallback.
  No user-visible silence.
- **Action during soak.** None.
- **Action for Phase 2.** Consider whether the SOAP can be more
  compact (F-2's fix may *increase* the payload, so we weigh
  together). Do not add retry per ADR-0016.

### F-6: Kimi latency variance — 3.9 s to 31.7 s

- **Observed** same session (2026-04-21 17:54–17:56).
- **Analysis.** Direct `curl` with 8-token prompt: 3.9 s.
  Russell's payload (~2.2 k prompt tokens): 31.7 s. The
  difference tracks completion token count + model reasoning
  time; not a Russell issue.
- **Severity.** Informational.
- **Action during soak.** None.
- **Action for Phase 2.** Possibly a prompt-compaction option
  (`russell jack --terse`) that trims the Objective section to
  severity counts + last 5 events for quick checks.

## Triage summary

| # | Class | Act during soak? | Phase-2 target |
|---|---|---|---|
| F-1 | Host signal | No | Rule engine catches this |
| F-2 | Russell bug | **No** | Extend `prompt::compose` |
| F-3 | Reliability | No (watch) | Reflex arc if reproducible |
| F-4 | Russell limitation | No | Rule engine informs fallback |
| F-5 | External + handled | No | — |
| F-6 | Informational | No | `--terse` mode |

**Zero findings require breaking the soak.** Russell's
doing his job.

## Change log

- 2026-04-21 — Day 4 checkpoint; F-1 through F-6 recorded.
- (Next update: ~2026-04-24.)

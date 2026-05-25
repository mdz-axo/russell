---
title: "ADR-0021: Proprioception Phase 2 — Reflex Arcs and Self-Vitals"
audience: [developers, architects]
last_updated: 2026-05-09
ddmvss_context: "proprioception"
ddmvss_artifact: "adr"
version: "1.0.0"
status: "Active"
---



# ADR-0021: Proprioception Phase 2 — Reflex Arcs and Self-Vitals

- **Status:** Accepted
- **Date:** 2026-05-09
- **Deciders:** Project founders
- **Tags:** `proprioception`, `self-health`, `jr-5`, `jr-2`

## Context

[`ADR-0015`](0015-proprioception-self-health.md) defines four proprioception
components: Meta-Sentinel, Meta-Doctor, Reflex arcs, and Autoimmune guard.
The MVP (`MVP_SPEC.md §3`) delivered only `sentinel_last_run_age_s` — the
single self-vital required by JR-5.

Phase 2 proprioception means implementing the remaining components,
specifically the reflex arcs that can detect and (where MVP-compatible)
correct degraded Russell states. The key constraint from ADR-0015:
*proprioception never mutates host state outside Russell's own files and
user units.*

This ADR covers the **detection-only** reflex arcs (Phase 2A), which do not
require mutations. The **corrective** reflex arcs (Phase 2B — journal unstick,
subprocess reaper) require mutation and IDRS, and are therefore deferred
to when Russell's skill system is live.

## Decision

Implement three new self-vitals and the `AutoimmuneGuard`:

| Vital | Source | Warn threshold | Alert threshold |
|---|---|---|---|
| `journal_writer_stall_s` | Time since last journal `INSERT` via clock-of-last-write | > 60s | > 300s |
| `llm_p95_latency_ms` | `help_sessions` table, p95 of `latency_ms` in last 24h | > 8 000 ms | > 20 000 ms |
| `timer_drift_s` | `systemctl --user show` of Russell's own timer | > 90s | > 300s |
| `help_error_rate_pct` | `help_sessions` table, error+fallback+threshold_skip / total in last 24h | > 20% | > 50% |

Each vital:
- Writes a `scope='self'` sample to the `samples` table
- Emits a `scope='self'` event at `warn` or `alert` severity when threshold is breached
- Is non-mutating (pure read from journal, filesystem, or systemctl)

### AutoimmuneGuard

```/dev/null/autoimmune_guard.rs#L1-6
// Process-wide guard preventing re-entrant meta-Doctor runs.
// Held for the duration of a meta-Doctor execution.
// Re-entrant attempts return error.code: autoimmune_block and journal a
// proprio.autoimmune.blocked event.
pub struct AutoimmuneGuard(tokio::sync::Mutex<()>);

impl AutoimmuneGuard {
    pub async fn enter(&self) -> Result<AutoimmuneGuardGuard<'_>>;
}

pub enum AutoimmuneError {
    Blocked { held_for_ms: u64 },
}
```

### Reflex Arc Summary

| Arc | Phase | Status after this ADR |
|---|---|---|
| Skill watchdog (subprocess timeout) | 2B | Deferred (needs skill system) |
| Journal writer unstick | 2B | Deferred (needs mutation) |
| LLM stall aborter | 2A | **Implemented** (reqwest timeout handles HTTP layer; Russell self-vital detects pattern) |
| MCP slow-loris aborter | 2B | Deferred (needs MCP server ADR-0003) |
| `journal_writer_stall_s` detection | 2A | **Implemented** (detect only) |
| `llm_p95_latency_ms` detection | 2A | **Implemented** (detect only) |
| `timer_drift_s` detection | 2A | **Implemented** (detect only) |
| `help_error_rate_pct` detection | 2A | **Implemented** (detect only) |
| AutoimmuneGuard | 2A | **Implemented** (foundation for meta-Doctor) |

## Implementation

### Location

All new self-vitals live in `russell-proprio` alongside the existing
`sentinel_last_run_age_s` vital. The `run_once` function is extended to
call each vital in sequence, accumulating results.

### Journal writer stall (`journal_writer_stall_s`)

The `JournalWriter` records a `last_write_unix_s` atomically updated after
each successful `append` / `append_sample` call. The vital reads this field
and computes `now - last_write_unix_s`.

If the journal writer is stuck (e.g. WAL write lock held), this vital will
show the stall time growing. The corrective reflex (checkpoint) is deferred
to Phase 2B.

### LLM p95 latency (`llm_p95_latency_ms`)

Reads `latency_ms` from `help_sessions` rows in the last 24h. Computes p95
by sorting and selecting the 95th percentile. Emits warn if > 8s, alert
if > 20s. This detects when Ollama or OpenRouter is becoming slow without
requiring a live LLM call.

### Timer drift (`timer_drift_s`)

Runs `systemctl --user show russell-sentinel.timer --property=LastTriggerUSec`
to get the last trigger time. Computes `now - last_trigger_unix`. If >
90s (1.5× cadence), emits warn. Gracefully degrades if systemctl fails
(no event emitted, sample not written — the vital is skipped silently).

### Help error rate (`help_error_rate_pct`)

Reads `help_sessions` rows in last 24h. Computes
`(error + fallback + threshold_skip) / total * 100`. High rates indicate
network problems or API key issues.

### AutoimmuneGuard

```/dev/null/autoimmune_guard_impl.rs#L1-12
use tokio::sync::Mutex;

/// Process-wide guard preventing re-entrant meta-Doctor runs.
/// When held, any attempt to enter meta-Doctor returns
/// `AutoimmuneError::Blocked`.
#[derive(Debug)]
pub struct AutoimmuneGuard(Mutex<()>);

#[derive(Debug)]
pub struct GuardGuard<'a>(&'a Mutex<()>);

impl AutoimmuneGuard {
    /// Enter the guard. Blocks if another meta-Doctor run is active.
    /// Returns guard which releases on drop.
    pub async fn enter(&self) -> GuardGuard<'_> {
        self.0.lock().await;
        GuardGuard(self.0)
    }

    /// Try to enter without blocking. Returns `None` if already held.
    pub async fn try_enter(&self) -> Option<GuardGuard<'_>> {
        self.0.try_lock().await.map(GuardGuard)
    }
}

impl<'a> Drop for GuardGuard<'a> {
    fn drop(&mut self) {
        // Mutex drops naturally — release is synchronous and guaranteed
    }
}
```

## Consequences

### Positive

- Russell can now detect degraded internal state (slow LLM, journal stall,
  timer drift) before the operator notices
- `llm_p95_latency_ms` provides early warning of Ollama going slow without
  requiring a live probe call
- `help_error_rate_pct` gives operators a single number for "is the help
  channel healthy?"
- AutoimmuneGuard enables safe meta-Doctor runs without recursive
  self-triage

### Negative / accepted costs

- Additional journal rows per Sentinel cycle (+4 self-vitals)
- `timer_drift_s` depends on systemd user session — may be unavailable
  in some container/SSH-only environments (gracefully skipped)
- `journal_writer_stall_s` requires atomic clock in JournalWriter —
  adds a small `std::sync::atomic` to the writer's internal state

### Neutral

- All four vitals are read-only; no mutation introduced
- Existing `sentinel_last_run_age_s` behavior is unchanged

## Alternatives Considered

### All reflex arcs deferred until Phase 2B

Rejected. The detection vitals are MVP-compatible (no mutation needed)
and provide real operational value immediately. Delaying them means
Russell goes blind to slow-LLM and journal-stall scenarios for longer.

### Timer drift via inotify on systemd socket

Rejected. Over-engineered. `systemctl show` is a read-only call that
already tells us what we need. The timer daemon updates its state
on disk after each trigger; reading that is sufficient.

### Combine all self-vitals into one `run_self_checks` function

Considered. Eventually correct, but the current single-function approach
(`run_once`) is MVP-appropriate. When more vitals are added in later
Phase 2 work, a per-vital loop with early-exit on critical events
becomes appropriate.

## References

- [`ADR-0015`](0015-proprioception-self-health.md) — full proprioception spec
- [`ADR-0020`](deferred/0020-threshold-gated-llm-escalation.md) — threshold-gated LLM (related pattern)
- [`../archive/proprioception.md`](../archive/proprioception.md) §4 — reflex arc table
- [`russell-proprio/src/lib.rs`](../../crates/russell-proprio/src/lib.rs) — current implementation

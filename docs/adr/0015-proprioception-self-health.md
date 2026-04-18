---
title: "ADR-0015: Proprioception — Self-Health"
audience: [developers, architects]
last_updated: 2026-04-18
togaf_phase: "H — Change Management"
version: "1.0.0"
status: "Active"
---

<!-- TOGAF_DOMAIN: Change Management -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-04-18 -->

<!--
audience: contributors adding loops, tasks, or subprocesses to Russell
last-reviewed: 2026-04-17
-->

# ADR-0015: Proprioception — Russell watches its own runtime as a first-class concern

- **Status:** Accepted
- **Date:** 2026-04-17
- **Deciders:** Project founders
- **Tags:** `proprioception`, `self-health`, `safety`

## Context

Russell is a closed-loop controller with a Sentinel, a
Doctor, a skill dispatcher, a journal writer, an MCP server,
and an LLM client. Each is a potential failure source:

- Timer drift makes cadences lie.
- A skill subprocess hangs and zombifies.
- The journal writer deadlocks under WAL stress.
- An MCP request waits forever.
- The LLM call stalls on a cold model swap.

A controller that cannot detect its own degraded state will
report confidently about the host while itself being the
most broken service on the box. We need a self-observation
pathway with the same cadence, contract, and evidence model
as the host-observation one.

## Decision

Russell has a **reflexive nervous system** with four
components:

1. **Meta-Sentinel.** A telemetry collector that samples
   Russell's own vitals on the same 5-minute cadence as
   the host Sentinel. Vitals listed in
   [`../archive/proprioception.md`](../archive/proprioception.md)
   §3. Writes to a `proprio_samples` table that mirrors
   `samples` column-for-column with an added `scope`
   field.

2. **Meta-Doctor.** The same Doctor supervisor pointed at
   Russell. Consumes self-symptoms, runs `skills/self/`
   skills (same manifest schema, same IDRS contract,
   scoped to Russell's own resources), writes SOAP
   bundles under `evidence/self/`, events into
   `proprio_events`.

3. **Reflex arcs.** Fast-path handlers for faults that
   cannot wait for the next Sentinel cycle:
   subprocess watchdog, journal-writer unstick,
   MCP slow-loris aborter, LLM stall aborter.
   Enumerated in `proprioception.md` §4.

4. **Autoimmune check.** A process-wide
   `AutoimmuneGuard` that prevents self-triage from
   invoking self-triage. Held for the duration of a
   meta-Doctor run; attempted re-entrant calls are
   rejected with `error.code: autoimmune_block` and
   journaled.

Invariants:

- Proprioception **never mutates host state outside
  Russell's own files and user units**. The meta-Doctor
  cannot invoke a host-scope skill.
- Proprioception follows IDRS exactly like host
  interventions. `self/journal-compactor` declares
  `risk: low` and rolls back to previous WAL
  geometry; `self/timer-rekick` declares
  `rollback: none_needed` because "re-enable"
  is its own inverse.
- Proprioception is **opt-out-only**: an operator may
  disable specific reflex arcs via
  `~/.config/harness/proprio.toml`, but the meta-
  Sentinel itself cannot be turned off without
  disabling Russell entirely
  (`~/.config/harness/disable`).

## Consequences

### Positive

- Russell cannot be silently degraded; every loop has
  a named vital and a reflex arc if one is needed.
- Operators see a weekly-digest "Self" section and can
  spot trends.
- The same SOAP / evidence pathway produces records
  that are reviewable after the fact.

### Negative / accepted costs

- Another subsystem to build (a whole crate:
  `russell-proprio`). Sized deliberately small:
  meta-Sentinel is ~150 LOC, meta-Doctor is
  dispatcher re-use.
- The journal carries more rows. Offset by the
  meta-Sentinel's own `self/journal-compactor` skill.

### Neutral

- Self-health is conventional in cybernetic systems
  (viability, VSM). We are just naming it.

## Alternatives considered

### External supervisor (systemd watchdog only)

Rejected. systemd's `WatchdogSec=` catches hard hangs
but cannot distinguish "LLM slow" from "LLM
dead." We use it as defense in depth, but it
complements rather than replaces proprioception.

### Fold self-health into the host Sentinel

Rejected. Scope confusion: a rule meaning "restart
Russell" should not be expressible in the same file
as "alert on host GPU hang." The `scope` field and
a separate table keep the two worlds honest.

### Skip proprioception in v1

Rejected in design. The closed-loop controller needs
self-observation from day one or the first time it
misbehaves will be catastrophic.

## Implementation notes

- The `AutoimmuneGuard` is a `tokio::sync::Mutex`
  held across the meta-Doctor run. A panic releases
  the guard; the meta-Sentinel detects a held guard
  older than a configured TTL and logs
  `proprio.autoimmune.guard_lost` (it does **not**
  break the guard; a new guard is re-armed on the
  next cycle by design).
- `skills/self/` is a reserved prefix; the dispatcher
  refuses to execute `skills/self/*` on anything
  other than a meta-Doctor run.
- New loops introduced in any crate must answer the
  proprioception checklist
  (`proprioception.md` §9); reviewers enforce.

## References

- [`../archive/proprioception.md`](../archive/proprioception.md)
- [`../standards/safety.md`](../standards/safety.md) §9
- [ADR-0007](deferred/0007-yaml-manifest-subprocess-skill-model.md)
- [ADR-0008](0008-llm-triage-never-emits-shell.md)
- Stafford Beer, *Brain of the Firm* — self-
  observation is System 3\*.

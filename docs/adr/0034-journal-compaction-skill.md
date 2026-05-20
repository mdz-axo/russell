---
title: "ADR-0034 — Journal Compaction Skill"
audience: [developers, operators]
last_updated: 2026-05-19
togaf_phase: "G"
version: "1.0.0"
status: "Active"
---

# ADR-0034 — Journal Compaction Skill

<!-- TOGAF_DOMAIN: Operations — Data Management -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-05-19 -->

## Context

The adversarial multi-perspective review (2026-05-19) identified weakness D2:

> **D2 — No journal compaction** — unbounded growth until Phase 2. MVP_SPEC §6 deferred compaction. JR-1 austerity.

Russell's SQLite journal grows unbounded as samples and events accumulate. On a 5-minute cadence with 25+ probes, this generates:
- ~288 samples/hour × 24 = ~6,912 samples/day
- ~2,500 samples/month
- ~30,000 samples/year

Without compaction, the journal will eventually exhaust disk space, especially on systems with limited storage.

The MVP deferred compaction (MVP_SPEC §6), but operational necessity requires implementation before Phase 2.

## Decision

Implement a `journal-compactor` skill with IDRS-protected interventions:

1. **Probe: `probe-size`** — Estimates journal size and sample age distribution:
   - Total file size in bytes
   - Sample count
   - Oldest/newest sample age in days
   - Count of samples older than 365 days

2. **Intervention: `vacuum-journal`** (risk: low, auto-executable):
   - Runs `sqlite3 journal.db "VACUUM;"`
   - Reclaims unused space from deleted rows
   - Idempotent — safe to run repeatedly
   - Rollback: `none_needed` — VACUUM cannot be undone but is non-destructive

3. **Intervention: `prune-old-samples`** (risk: medium, requires human):
   - Deletes host samples older than retention threshold (default 365 days)
   - Runs VACUUM after deletion to reclaim space
   - **DATA LOSS** — samples are permanently deleted
   - Listed in `safety.require_human_for` — requires explicit operator consent
   - Rollback: `none_needed` — deletion is irreversible

4. **Evaluation: `verify-integrity`** — Post-intervention check:
   - SQLite `PRAGMA integrity_check`
   - Event/sample count validation
   - Hash chain verification (simplified — full check via `russell verify-journal`)

5. **Safety constraints**:
   - `max_auto_risk: low` — Only `vacuum-journal` auto-executes
   - `require_human_for: [prune-old-samples]` — Explicit consent for data loss

## Consequences

### Positive

- **Operational sustainability** — Prevents disk exhaustion from unbounded journal growth.

- **IDRS compliance** — Compaction operations are journaled with evidence bundles.

- **Operator control** — `prune-old-samples` requires explicit consent; operator chooses retention threshold.

- **Integrity verification** — Post-intervention checks detect corruption early.

- **JR-1 compliance** — Austere implementation: uses SQLite built-in VACUUM, no external dependencies.

### Negative

- **Data loss risk** — `prune-old-samples` permanently deletes historical data. Mitigated by requiring human consent.

- **Downtime during VACUUM** — Large journals may take minutes to compact. Mitigated by 300s timeout.

- **No incremental compaction** — VACUUM rebuilds entire database file. Future optimization: incremental vacuum.

### Neutral

- **Manual operation** — Skill runs on-demand, not automatically. Future: scheduled compaction via systemd timer.

- **No compression** — VACUUM reclaims space but doesn't compress data. Future: consider WAL compression.

## Implementation

### Skill Structure

```
skills/journal-compactor/
├── manifest.yaml
└── scripts/
    ├── probe-size.sh
    ├── vacuum-journal.sh
    ├── prune-old-samples.sh
    └── verify-integrity.sh
```

### Files Created

| File | Purpose |
|---|---|
| `skills/journal-compactor/manifest.yaml` | Skill definition with probes, interventions, safety |
| `scripts/probe-size.sh` | Estimates journal size and age distribution |
| `scripts/vacuum-journal.sh` | Runs SQLite VACUUM |
| `scripts/prune-old-samples.sh` | Deletes old samples, then VACUUM |
| `scripts/verify-integrity.sh` | Post-intervention integrity check |

### Usage

```bash
# Check journal size
russell skill run journal-compactor/probe-size

# Compact journal (safe, auto-executable)
russell skill run journal-compactor/vacuum-journal

# Prune old samples (requires consent)
russell skill run journal-compactor/prune-old-samples
# → Prompts for operator confirmation due to data loss
```

### Installation

```bash
# Copy to installed skills directory
cp -r skills/journal-compactor ~/.local/share/harness/skills/

# Verify installation
russell skill list | grep journal-compactor
```

## Compliance

| Principle | Compliance |
|---|---|
| **JR-1** (Austere) | Uses SQLite built-in VACUUM; no external dependencies |
| **JR-2** (Observe > Recommend > Act) | Probe observes size; intervention acts with consent |
| **JR-6** (Reuse over dependency) | Reuses existing skill framework, IDRS contract |
| **JR-7** (Persistence auditable) | Compaction operations journaled with evidence bundles |

## Future Work

- **Scheduled compaction** — Add systemd timer to run `probe-size` weekly, alert if journal >1GB.

- **Incremental vacuum** — Use `PRAGMA incremental_vacuum` for large journals to reduce downtime.

- **Compression** — Consider WAL mode with compression for space efficiency.

- **Retention policy** — Make retention threshold configurable per-operator preference.

- **Archive before prune** — Option to export old samples to compressed archive before deletion.

## References

- Adversarial Review Action Plan §4.2 (Task D2)
- `docs/specifications/MVP_SPEC.md` §6 (deferred features)
- `docs/specifications/PERSISTENCE_CATALOG.md` §2 (journal structure)
- SQLite documentation: [`VACUUM`](https://sqlite.org/lang_vacuum.html)

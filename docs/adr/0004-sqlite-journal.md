---
title: "ADR-0004: SQLite Journal"
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
audience: journal / persistence contributors
last-reviewed: 2026-04-17
-->

# ADR-0004: Persistence — SQLite for the journal

- **Status:** Accepted
- **Date:** 2026-04-17
- **Deciders:** Project founders
- **Tags:** `persistence`, `journal`, `schema`

## Context

[`cybernetic-health-harness.md` §8](../../cybernetic-health-harness.md)
specifies an event journal with three tables: `samples`, `events`,
`baselines`. Expected write pattern:

- Sentinel: ~14 rows every 5 minutes = ~4k rows/day into
  `samples`.
- Doctor / tiers: ~10s–100s of events/day into `events`.
- Baselines: written a few times per day per probe, reads on
  every Sentinel cycle.

Read pattern is append-append-scan-range-lookup. The data is
single-host and the working set is small (< 1 GiB over years).

## Decision

The journal is a **single SQLite database** at
`~/.local/state/harness/journal.db`, accessed via the
[`rusqlite`](https://docs.rs/rusqlite) crate with the
`bundled` feature.

Operational parameters:

- **WAL mode** (`PRAGMA journal_mode=WAL`) for concurrent reads
  alongside writes.
- `PRAGMA synchronous=NORMAL` — safe under WAL; a crash may
  lose the last-transaction sample but never corrupts the DB.
- `PRAGMA foreign_keys=ON` (where tables have FKs).
- `PRAGMA temp_store=MEMORY` for the rendering-heavy
  `digest_render` pass.
- Busy timeout of 5 seconds.
- Connection pool (r2d2 or hand-rolled `tokio::sync`-gated)
  for readers; **writers serialized through a single
  `spawn_blocking` task** owned by `russell-core::journal::
  writer`.

Migration strategy:

- SQL files under
  `crates/russell-core/src/journal/migrations/`, zero-padded
  (`0001_init.sql`, `0002_proprio_tables.sql`, …).
- A versions table tracks applied migrations.
- Migrations are forward-only and never edited after merge.
- Each migration has a test that opens a fresh DB, runs all
  migrations, and snapshots `PRAGMA table_info(...)`.

Backup: the operator is responsible for backing up
`~/.local/state/harness/`. The weekly digest includes a row
reporting the journal's size and the age of the most recent
checkpoint.

## Consequences

### Positive

- Zero-install: `bundled` rusqlite ships SQLite statically;
  no `libsqlite3-dev` required.
- Transactions give us atomic multi-row events
  (samples + event + baseline update in one commit).
- SQL gives us cheap range queries for the weekly digest
  and the `journal_query` MCP tool.
- Tooling: the operator can inspect directly with
  `sqlite3 journal.db`.

### Negative / accepted costs

- A single-writer contract adds latency under bursty load.
  The `journal_writer_lag_ms` vital
  (see [proprioception.md](../archive/proprioception.md))
  exists precisely to detect that latency rising.
- WAL files (`-wal`, `-shm`) live alongside the DB. The
  proprioception reflex arc `self/journal-compactor` handles
  WAL growth.
- Schema changes require a migration. This is a feature:
  rolling back a migration is part of the IDRS story.

### Neutral

- `bundled` increases binary size by a few MB; acceptable
  for a single-host tool.

## Alternatives considered

### Plain JSONL files

Rejected. Range queries, baseline rollups, and digest
rendering all become custom code. No ACID; concurrent
writers (Sentinel + Doctor) would race.

### DuckDB

Rejected. Richer analytics but heavier dependency, less
familiar to most contributors, and WAL-mode SQLite is
sufficient for our read volume.

### PostgreSQL (local)

Rejected. Requires a running daemon; violates the
zero-install feel and adds a failure mode
(postgres-not-running) that Russell would then have to
self-triage.

### sled / redb (embedded KV)

Rejected. No SQL means writing our own query engine for the
digest and `journal_query`. The query API on the MCP
surface is hard to beat with bespoke code.

## Implementation notes

- The writer task exposes a `JournalWriter` handle with
  `enqueue_event`, `enqueue_samples`, `update_baseline`
  async methods. These return once the write is durable.
- Readers go through a `JournalReader` type that holds a
  pooled connection; all read methods take `&self`.
- No other crate opens the DB directly
  ([`coding-rust.md` §13](../standards/coding-rust.md)).
- Proprioception tables are column-for-column mirrors of
  `samples` / `events` with a `scope` column.

## References

- rusqlite docs: https://docs.rs/rusqlite
- SQLite WAL: https://sqlite.org/wal.html
- [`cybernetic-health-harness.md` §8](../../cybernetic-health-harness.md)
- [ADR-0015](0015-proprioception-self-health.md) — proprio tables.

---
title: "ADR-0009: Tokio Runtime (Deferred)"
audience: [developers, architects]
last_updated: 2026-04-18
ddmvss_context: "cross-cutting"
ddmvss_artifact: "adr"
version: "1.0.0"
status: "Deferred"
---



> **Deferred.** This ADR's subject is outside the MVP boundary per
> [`../../specifications/MVP_SPEC.md`](../../specifications/MVP_SPEC.md) §5.
> It remains **Accepted** — when its phase opens, it ships this way.


<!--
audience: async runtime and concurrency contributors
last-reviewed: 2026-04-17
-->

# ADR-0009: Async runtime — Tokio multi-thread

- **Status:** Accepted
- **Date:** 2026-04-17
- **Deciders:** Project founders
- **Tags:** `runtime`, `async`, `concurrency`

## Context

Russell needs concurrent I/O (MCP stdio read/write, subprocess
output streaming, SQLite writer task, periodic timers from
systemd reaching the process, LLM HTTP calls to local Ollama).
Rust's async ecosystem effectively offers Tokio or async-std;
Tokio is by far the better-supported choice today, and the MCP
SDKs for Rust are Tokio-native.

Some subsystems (SQLite writes, subprocess spawn on certain
kernels) are blocking and must not starve the async runtime.

## Decision

1. **Tokio multi-thread runtime**, version 1.x, with default
   worker-thread count (CPU count).
2. The binary's `main` is `#[tokio::main(flavor =
   "multi_thread")]`.
3. Library crates do **not** spawn their own runtime; they
   accept either `&tokio::runtime::Handle` for
   spawn-capable APIs or `#[tokio::test]` instrumentation
   for tests.
4. Blocking work (SQLite writer, subprocess stdin pipe
   setup, file `fsync`) routes through
   `tokio::task::spawn_blocking`. The dedicated blocking
   pool is sized by Tokio's default (512) and is **not**
   intended for high-throughput CPU work.
5. CPU-heavy work (parsing very large dmesg tails,
   rendering the digest) uses `rayon` if benchmarks show
   async tasks backing up. This is an optimization
   path, not a default.
6. Cancellation propagates via
   `tokio_util::sync::CancellationToken`. Every long-lived
   task accepts a token; subprocess supervisors in
   `russell-skills::dispatch` attach the token to
   `tokio::process::Child::kill_on_drop(true)` for safety.
7. Shared state uses `tokio::sync` primitives (`Mutex`,
   `RwLock`, `Notify`, `broadcast`, `mpsc`, `oneshot`).
   `std::sync::Mutex` is allowed only for brief
   non-crossing-await sections with a comment explaining
   why.

## Consequences

### Positive

- Single well-understood runtime across every crate.
- First-class subprocess API via `tokio::process`.
- Cooperative cancellation is a natural fit for the
  Doctor's "evaluate, then maybe rollback" flow.

### Negative / accepted costs

- Multi-thread Tokio demands `Send + 'static` for spawned
  futures, which occasionally complicates ergonomics
  (e.g. `!Send` types like `rusqlite::Connection`
  must be confined to `spawn_blocking` or to a single
  thread).
- Some future dependencies may ship async-std or smol
  variants; we will prefer Tokio-native or
  adapter-friendly crates.

### Neutral

- Tokio binary size is not meaningfully different from
  async-std for our needs.

## Alternatives considered

### async-std

Rejected. Ecosystem momentum is firmly Tokio; the MCP
SDKs, `rusqlite` async wrappers, `reqwest`, `sqlx`, and
`tracing` ecosystem all target Tokio first.

### smol / `async-executor`

Rejected. Good runtimes but smaller ecosystems; the
trade-off does not help a project that needs mature
subprocess and journald integration.

### Synchronous (threads + blocking)

Rejected for the MCP surface. JSON-RPC framing with
multiple concurrent tools in flight is much cleaner on
async; we would end up rebuilding a small runtime.

## Implementation notes

- `russell-core::task` exposes a thin wrapper over
  `tokio::task::JoinHandle` that records span
  information for proprioception. Every spawned task
  must go through this wrapper so the `self_status`
  MCP tool can report on live tasks.
- The blocking pool is monitored: a proprio vital
  `blocking_pool_queue_depth` is sampled on Sentinel
  cadence. Rising queue depth is a warning class.

## References

- Tokio docs: https://tokio.rs
- [`../../standards/coding-rust.md`](../../standards/coding-rust.md) §5
- [ADR-0004](../0004-sqlite-journal.md) — writer uses
  `spawn_blocking`.
- [ADR-0015](../0015-proprioception-self-health.md) — blocking
  pool observation.

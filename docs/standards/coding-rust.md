---
title: "Rust Coding Standard"
audience: [developers]
last_updated: 2026-04-18
togaf_phase: "Preliminary"
version: "1.0.0"
status: "Active"
---

<!-- TOGAF_DOMAIN: Preliminary -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-04-18 -->

<!--
audience: Rust contributors to Russell
last-reviewed: 2026-04-17
-->

# Rust coding standard

Mechanics of the toolchain live in
[`CONTRIBUTING.md`](../../CONTRIBUTING.md); this document covers
**what the code should look like** once you are building it.

## 1. Edition and toolchain

- Rust edition **2024** across every crate.
- Pinned `rust-toolchain.toml` at workspace root; never
  `rustup override` in a contributor checkout (the pin is the
  contract).
- Components: `rustfmt`, `clippy`, `rust-src`.

## 2. Module layout

```
russell/
├── Cargo.toml                 # workspace
└── crates/
    ├── russell-core/          # profile, journal, IDRS runtime, event schema
    ├── russell-sentinel/      # telemetry collector, EWMA baselines
    ├── russell-meta/        # symptom triage, SOAP assembly, LLM client
    ├── russell-skills/        # manifest loader, dispatcher, poka-yoke
    ├── russell-mcp/           # MCP server (stdio transport)
    ├── russell-cli/           # binary entry point; ties the above together
    └── russell-proprio/       # meta-sentinel, self-triage, reflex arcs
```

See [ADR-0013](../adr/0013-rust-workspace-layout.md) for why these
are separate crates. Cross-crate dependencies are a DAG rooted at
`russell-core`; no crate depends on `russell-cli`.

## 3. Error handling

- Library crates return `Result<T, ThisCrateError>` where
  `ThisCrateError` is an enum built with
  [`thiserror`](https://docs.rs/thiserror). Implement
  `source()` via `#[from]` / `#[source]`; never stringify a
  lower error.
- Binary crate (`russell-cli`) may use
  [`anyhow`](https://docs.rs/anyhow) and `eyre`-style
  reporting at top-level `main`.
- `panic!` / `unwrap` / `expect` are tolerated only in:
  - tests,
  - `main.rs` startup (fail-fast),
  - invariants that the type system genuinely cannot express,
    with a `// SAFETY:` or `// INVARIANT:` comment.
- Every public fallible function has a rustdoc `# Errors`
  section.

## 4. `unsafe` discipline

- No `unsafe` in `russell-core`, `russell-meta`,
  `russell-skills`, `russell-cli` except through audited,
  well-known crates.
- If you introduce `unsafe` anywhere, write a `// SAFETY:`
  comment paragraph that proves the invariant and cite the
  source of the invariant.
- An `unsafe` block added without a SAFETY comment is a
  review blocker.

## 5. Async runtime

- Tokio, multi-thread, with `#[tokio::main(flavor =
  "multi_thread")]` in the binary. Library crates accept a
  `&tokio::runtime::Handle` rather than spawning their own
  runtime.
- Blocking work (subprocess spawn, SQLite writes) goes through
  `tokio::task::spawn_blocking` or a dedicated blocking
  threadpool. See [ADR-0009](../adr/deferred/0009-tokio-runtime-lifted.md).

## 6. Logging

- `tracing` + `tracing-subscriber` + `tracing-journald`
  (fallback to `tracing-subscriber::fmt` when journald is
  unavailable).
- One span per MCP tool invocation, one span per skill probe,
  one span per skill intervention.
- Fields use snake_case and match the journal event schema
  (`harness.event.v1`). No ad-hoc keys.
- Levels: `error` for user-visible failure, `warn` for a
  condition Russell intends to act on, `info` for state
  transitions, `debug` for developer-only detail, `trace`
  off in release builds.

## 7. Naming and formatting

- `rustfmt` is authoritative; the workspace ships a
  `rustfmt.toml` that pins `edition = "2024"` and
  `newline_style = "Unix"`. No other overrides.
- Public items use verb-object names:
  `load_manifest`, not `manifest_load`.
- Abbreviations are lowercased in snake_case identifiers:
  `mcp`, not `MCP`; `soap_bundle`, not `SOAPBundle`. Type
  names keep the convention `McpTool`, `SoapBundle`.
- Prefer newtypes over `String`/`i64` for IDs:
  `SkillId(String)`, `EvidenceId(Ulid)`,
  `ProbeName(&'static str)`.

## 8. Clippy posture

- CI runs `cargo clippy --workspace --all-targets -- -D
  warnings`.
- Narrow `#[allow(clippy::xxx)]` annotations are OK with a
  one-line comment explaining why.
- Workspace-wide allowances go in `clippy.toml` and require a
  citation to an ADR.

## 9. Testing placement

- Unit tests colocate in `mod tests` at the bottom of the
  file under test.
- Integration tests live under each crate's `tests/`
  directory.
- Snapshot tests use
  [`insta`](https://insta.rs/). Never accept a snapshot
  blindly; diff it in `cargo insta review`.
- Property tests use
  [`proptest`](https://proptest-rs.github.io/proptest/) for
  manifest parsing, rules parsing, and MCP wire round-trips.

## 10. Dependencies

- Every new direct dependency needs a one-line note in the
  relevant PR describing what it buys us that the standard
  library cannot.
- `cargo deny` runs in CI against `deny.toml`: advisories
  block the build; license policy is **MIT OR Apache-2.0
  OR BSD-3-Clause OR ISC OR Unicode-DFS-2016**; other
  licenses require an ADR carve-out.
- Prefer a smaller, focused crate over a mega-framework.
  When in doubt, file an issue before adding the dep.

## 11. Documentation within code

- Every public item in `russell-core`, `russell-meta`,
  `russell-skills`, and `russell-mcp` has a rustdoc comment.
- Use `# Examples`, `# Errors`, `# Panics` sections per the
  [API Guidelines](https://rust-lang.github.io/api-guidelines/).
- Link cross-crate items with intra-doc links (`[`SkillId`]`).

## 12. Concurrency primitives

- Shared state under a `tokio::sync::Mutex` or
  `tokio::sync::RwLock`; never `std::sync::Mutex` across an
  `.await` boundary.
- One-off signals use `tokio::sync::oneshot`; fan-out uses
  `broadcast`; work queues use `mpsc`.
- Cancellation propagates via `tokio_util::sync::CancellationToken`.

## 13. SQLite access

- All journal I/O routes through `russell-core::journal`,
  which wraps `rusqlite` behind a typed API. No other crate
  opens the DB directly.
- Writers serialize through a single `spawn_blocking` task;
  readers may go through a connection pool. See
  [ADR-0004](../adr/0004-sqlite-journal.md).

## 14. MCP server conventions

- One handler function per tool, returning a typed
  request/response pair. Tool schemas live in a dedicated
  `schema` module and are snapshot-tested.
- Every tool declares a `risk_band` at registration time;
  tools with `risk_band >= Medium` MUST route through the
  proposal / `confirm_proposal` pattern rather than acting
  directly. See [ADR-0003](../adr/0003-mcp-transport.md) and
  [`../archive/mcp-surface.md`](../archive/mcp-surface.md).

## 15. What not to do

- Do not read or write files under `~/.local/state/harness/`
  from outside `russell-core::journal` or `russell-core::paths`.
- Do not spawn subprocesses from outside `russell-skills::dispatch`.
- Do not call the LLM from outside `russell-meta::llm`.
- Do not add a CLI subcommand without a matching MCP tool
  (or vice versa) unless an ADR justifies the asymmetry.

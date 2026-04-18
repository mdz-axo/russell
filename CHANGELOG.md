# Changelog

All notable changes to Russell will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added — Phase 0 skeleton
- Cargo workspace with seven crates per ADR-0013
  (`russell-core`, `russell-sentinel`, `russell-skills`,
  `russell-doctor`, `russell-proprio`, `russell-mcp`,
  `russell-cli`). Non-core crates are Phase-0 placeholders
  so the dependency DAG is correct from day one.
- `russell-core`:
  - `paths` — XDG-aware resolution for config/state/data roots
    plus every well-known subdirectory, with idempotent
    `ensure_dirs`.
  - `event` — `harness.event.v1` record type with ULID IDs,
    lowercase `severity` / `scope` enums, and schema-version
    enforcement on load (per ADR-0006 / safety §1).
  - `profile` — `russell.profile.v1` machine chart, atomic
    write via `.tmp` + `rename`, `UnknownSchema` rejection
    (per ADR-0006).
  - `journal` — SQLite journal with WAL mode, `synchronous=NORMAL`,
    busy-timeout, forward-only numbered migrations
    (`migrations/0001_init.sql` creates `samples`,
    `events`, `baselines`, `confirmations`, and
    `schema_migrations`), typed `JournalWriter` /
    `JournalReader`, severity bucket reporter. Per ADR-0004.
  - `telemetry` — `tracing-subscriber` init honoring
    `RUSSELL_LOG` / `RUST_LOG`. Per ADR-0010.
  - `time` — RFC-3339 and unix-seconds helpers, centralised
    so a future test harness can shim the clock.
  - `error::CoreError` with `thiserror`, per
    `docs/standards/coding-rust.md` §3.
- `russell-sentinel`: Phase-0 probe set
  (`mem_available_mib`, `swap_used_mib`, `loadavg_1m`)
  reading `/proc/meminfo` and `/proc/loadavg`. `run_once`
  appends samples to the journal with `scope = host`.
- `russell-cli`: `russell` binary exposing `status`, `list`,
  `profile [--init]`, `digest [--since-hours]`, and a
  `sentinel-once` helper. Every subcommand honours
  `--root <path>` for sandboxed runs.
- Workspace configuration: `rust-toolchain.toml` pinned to
  1.94.1, `rustfmt.toml`, `.cargo/config.toml` aliases,
  `.gitignore`.
- 22 unit tests green under `cargo test --workspace`;
  `cargo clippy --workspace --all-targets -- -D warnings`
  and `cargo fmt --check` clean.

### Added — prior (documentation scaffold)
- Foundational standards and ADRs (0001–0015) covering scope, MCP
  transport, persistence, privileged operations, profile abstraction,
  skill model, LLM triage, licensing, runtime, observability, testing,
  config formats, workspace layout, and proprioception.
- `AGENTS.md` orientation document for human and AI contributors.
- `CONTRIBUTING.md` covering dev environment, tests, and local MCP
  wiring.
- Architecture overview, MCP surface catalog, and proprioception
  design under `docs/architecture/`.
- Skill manifest, SOAP bundle, and ADR templates under
  `docs/templates/`.

### Notes
- Phase-0 success criterion per
  `cybernetic-health-harness.md` §20 requires 7 consecutive
  days of Sentinel samples and rendered digests — validated
  functionally by round-trip; the 7-day soak is runtime,
  not a code-shaped gate.
- Next: Phase 1 (timer-driven Sentinel, rule evaluation, the
  first three Tier I modules `symptom-sweep`, `gpu-sanity`,
  `digest-desktop`).

[Unreleased]: https://example.invalid/russell/compare/HEAD...HEAD

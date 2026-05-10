# Changelog

All notable changes to Russell will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed — 2026-05-09: Ollama-first default, DeepSeek V4 Pro

- **ADR-0016 v2:** Flip default backend from OpenRouter to local
  Ollama. Default model is `deepseekv4pro`. OpenRouter remains
  available as explicit opt-in (`RUSSELL_DOCTOR_BACKEND=openrouter`).
- **Ollama auto-start.** `russell jack` checks for Ollama
  reachability (`GET /api/tags`, 3s timeout) and runs
  `systemctl --user start ollama` if it's not responding.
  Best-effort: does not install or configure Ollama.
- **Backend selection simplified.** No more silent auto-detection
  of OpenRouter via `OPENROUTER_API_KEY`. Default is Ollama.
  Set `RUSSELL_DOCTOR_BACKEND=openrouter` to opt in explicitly.
- **Code changes:**
  - `Backend::from_env()` now returns `Ollama` as default.
  - `ClientConfig::from_env()` defaults model to `deepseekv4pro`.
  - `help.rs` gains `ollama_health_check()` and `ollama_start()`.
- **Docs updated:** `AGENTS.md`, `README.md`, `MVP_SPEC.md`,
  `PRINCIPLES_CATALOG.md`, `PERSISTENCE_CATALOG.md`,
  `THE_JACK.md`, `INSTALL.md`, `CONSOLIDATED-STATUS.md`,
  `REUSE_MANIFEST.md`, `0016-doctor-and-llm-router.md` (v2).

### Added — 2026-05-06: Phase 1c closed, self-vital, Kask integration

- **Phase 1c closed** per [ADR-0018](docs/adr/0018-close-phase-1c.md).
  20-day soak: 2 062 cycles, ~99.95% reliability. Findings F-1
  through F-9 recorded; F-7 (self-vital) and F-2 (SOAP samples)
  carry into Phase 2.
- **`russell-proprio`: JR-5 self-vital implemented.**
  `sentinel_last_run_age_s` reads the journal for the most recent
  Sentinel sample, computes staleness in seconds, writes a
  `scope='self'` sample. Severity: `ok` (≤360s), `warning` (>360s).
- **Self-vital timing fix.** Proprio now runs BEFORE host probes in
  each sentinel cycle, so the self-vital is never stale-by-one.
- **`rust-toolchain.toml` fixed.** Pinned to `stable` channel;
  dropped `rust-src` component (was causing snap-based rustup hangs
  on exact-version pins).
- **Kask integration: `arsenal-mcp-russell` MCP server + Duncan
  Curator.** 6 MCP tools (`russell_host_snapshot`,
  `russell_journal_query`, `russell_recent_events`,
  `russell_probe_history`, `russell_health_summary`,
  `russell_curator_assess`) live in the Kask repo. Duncan
  (infrastructure Curator in `stack-control-plane`) reads Russell's
  journal via MCP and produces structured health assessments.
  Registered in `~/.config/stack/mcp-registry.json`. No cross-crate
  dependency — communication via SQLite journal + MCP.
- **Test count: 50** (was 44). Breakdown: 29 core + 14 doctor +
  6 proprio + 1 sentinel.

### Added — Phase 1b: Install artifacts + systemd units

- **systemd user units** under `packaging/systemd/`:
  - `russell-sentinel.{timer,service}` — 5-minute cadence, persistent
    across sleep, 15s jitter. User-scope hardening: `ProtectSystem=strict`,
    `ProtectHome=read-only`, `ReadWritePaths` narrowed to state/share,
    `PrivateTmp`, `NoNewPrivileges`. No `SystemCallFilter` — that requires
    caps user units can't drop.
  - `russell-digest.{timer,service}` — Sunday 09:00 local with 10m jitter,
    renders Markdown digest into `~/.local/state/harness/digest/YYYY-WNN.md`.
  - `russell-failure@.service` — templated capture of 50 lines of
    `journalctl` into `~/.local/state/harness/runs/failure-*.log`.
- **`packaging/bin/install.sh`** — idempotent operator installer:
  builds, installs binary to `~/.local/bin/russell`, installs units to
  `~/.config/systemd/user/`, seeds `~/.config/harness/russell.env`
  from `.env.example` (template) or repo `.env` (if populated with a
  real key), ensures state directories, reloads systemd, enables +
  starts timers, runs a smoke `sentinel-once`, prints `status`.
  Flags: `--no-start`, `--release`.
- **`packaging/bin/uninstall.sh`** — clean removal; preserves data by
  default, `--purge` for destructive wipe.
- **`.env.example`** — non-secret operator template, 39 lines.
  `.gitignore` uses `!.env.example` to permit commit.
- **`docs/operations/INSTALL.md`** — 216-line operator runbook: install,
  verify, daily ops, update, uninstall, troubleshoot.
- **Env discovery layering** — `russell-core::env::load_discovered` now
  layers files in precedence order (process env > config > repo > cwd)
  instead of first-file-wins. Empty values in a file are skipped so a
  blank template doesn't mask real keys from a lower-precedence file.
  Added 4 new tests; **44 total passing** (up from 39).
- **Live verification on the Framework 16 / HX 370 / Ubuntu 25.10**
  machine (`MACHINE_PROFILE.md`): timer fires every 5 min, Sentinel
  captures 3 samples per cycle at 5.1 MB peak memory / 25 ms CPU.
  Real LLM round-trip via Ollama tested end-to-end; Jack
  reads his own event history and narrates the machine's state in
  persona-accurate voice.

### Changed

- `.gitignore` hardened: `.env`, `.env.*` ignored; `.env.example` /
  `.env.template` explicitly whitelisted. `.env` was never tracked.

### Next

- **Phase 1c — 30-day unattended soak** on the Framework 16. No new
  code until the soak completes per `MVP_SPEC.md` §6.
- Optional: `russell digest` HTML rendering for the Sunday email.

## [Unreleased-prior]

### Added — Phase 1: The Doctor (`russell jack`)

- **ADR-0016** — MVP Doctor spec: single round-trip, Ollama
  default (`deepseekv4pro`), OpenRouter opt-in, offline
  fallback mandatory.
- **ADR-0017** — Reuse over dependency: JR-6 mechanism codified.
- **`russell-doctor` crate** — Phase-1 implementation:
  - `LlmClient` trait, minimal Russell-shaped types.
  - `OpenRouterClient` backend — pattern-copied from
    `slate/stack/crates/stack-llm/src/{openai,wire}.rs` per
    `REUSE_MANIFEST.md` row 1. Drops streaming, tool-calling,
    structured-output, retry. Adds per-request ZDR enforcement
    (when using OpenRouter). Retains reasoning-details content
    normalisation for models that emit it.
  - `MockClient` for tests and `RUSSELL_DOCTOR_BACKEND=mock`.
  - `fallback::summarise` — the offline rule-based response.
    Jack is never silent.
  - `prompt::compose` — SOAP-shaped Markdown prompt builder.
  - `run_help_with_config` — the orchestrator. Takes an
    explicit `ClientConfig` so tests don't race on process env.
  - `JACK_PERSONA` — `crates/russell-doctor/prompts/jack.md`
    embedded via `include_str!` so Jack always has his voice.
- **`russell jack [--note "..."]`** CLI verb added. Named
  `jack` (not `help`) because clap reserves `help`; the name
  honours the persona per `THE_JACK.md` anyway.
- **`help_sessions` table** — migration `0002_help_sessions.sql`.
  Records every Doctor round-trip with backend, model, chars,
  latency, status, error_kind, evidence_ref.
- **`russell-core::env::load_env_file`** — minimal loader for
  `~/.config/harness/russell.env`. Existing env always wins.
- **Evidence bundle format** — `~/.local/state/harness/evidence/help/<ulid>/`
  with `soap.md`, `request.json`, `response.json`,
  `transcript.jsonl`.
- **39 tests passing** (up from 22): 24 in `russell-core`,
  14 in `russell-doctor`, 1 in `russell-sentinel`.

### Changed

- `russell-core` relaxed `#![forbid(unsafe_code)]` →
  `#![deny(unsafe_code)]` to allow narrow `#[allow(unsafe_code)]`
  annotations on environment-mutation calls (Rust 2024 edition
  made `env::set_var` unsafe). Every use site carries a SAFETY
  comment.
- `CLI main` is now `#[tokio::main(flavor = "multi_thread")]`
  to support the Doctor's async HTTP call.

### Verified

- `cargo fmt --check` ✅
- `cargo clippy --workspace --all-targets -- -D warnings` ✅
- `cargo test --workspace` ✅ (39 passing)
- End-to-end sandbox: `sentinel-once` + `jack --note` produces
  response, journal event, `help_sessions` row, and evidence bundle.

## [Unreleased-prior]

### Added — Documentation pivot (JR-1 austerity + UDQL-lite governance)

- **Principles catalog** — `docs/architecture/PRINCIPLES_CATALOG.md` —
  JR-1 through JR-7. *Though she be but little, she is fierce.*
- **Documentation standard** — `docs/standards/DOCUMENTATION_STANDARDS.md` —
  UDQL-derived governance (authority hierarchy, critical set,
  mandatory update gate, Mermaid `DIAGRAM_ALIGNMENT`, audience
  vocabulary, voice register, freshness tracking, Diataxis,
  TOGAF phase tags).
- **MVP spec** — `docs/specifications/MVP_SPEC.md` — pinned
  boundary: six read-only verbs, the help channel, the single
  proprioception vital.
- **Persistence catalog** — `docs/specifications/PERSISTENCE_CATALOG.md` —
  every byte Russell writes, named.
- **Reuse manifest** — `docs/operations/REUSE_MANIFEST.md` —
  register for copied files from `peripheral` and `slate/stack`.
- **TOGAF traceability matrix** — `docs/architecture/TOGAF_TRACEABILITY_MATRIX.md`.
- **Documentation portal** — `docs/README.md` — single
  navigation entrypoint + critical-set declaration.
- **Consolidated status** — `docs/status/CONSOLIDATED-STATUS.md` —
  Phase 0 done; Phase 1 planned.
- **THE JACK** — `docs/architecture/THE_JACK.md` — the persona
  design (Jack Russell Terrier × Jack McFarland × Rust/Linux/
  cybernetics).
- **Persona file** — `crates/russell-doctor/prompts/jack.md` —
  the LLM system prompt Jack speaks with.
- **Russell-native AGENTS.md** — the binding orientation
  document. Inherited Peripheral rules moved to
  `docs/standards/agent-operating-rules.md`.
- Directory READMEs for all `docs/` subdirectories.
- YAML frontmatter + TOGAF metadata on every authoritative
  document.

### Changed

- **Architecture scope retreated to JR-1 austerity.** Seven
  ADRs moved to `docs/adr/deferred/` (0003, 0005, 0007, 0009,
  0010, 0012, 0014); their subjects are outside the MVP
  boundary but remain **Accepted** — they ship this way when
  their phase opens.
- **MCP surface and full proprioception design archived** to
  `docs/archive/` with provenance entries in
  `docs/archive/README.md`. The active ADR-0015 preserves the
  one-vital MVP proprioception.

### Added — Phase 0 Rust skeleton (prior)

- Cargo workspace with seven crates per ADR-0013 (`russell-core`,
  `russell-sentinel`, `russell-skills`, `russell-doctor`,
  `russell-proprio`, `russell-mcp`, `russell-cli`). Non-core
  crates are Phase-0 placeholders.
- `russell-core` — `paths`, `event` (`harness.event.v1`),
  `profile` (`russell.profile.v1`), `journal` (SQLite + WAL +
  numbered migrations), `telemetry`, `time`, `error`.
- `russell-sentinel` — three `/proc`-based probes
  (`mem_available_mib`, `swap_used_mib`, `loadavg_1m`).
- `russell-cli` — five read-only verbs: `status`, `list`,
  `profile [--init]`, `digest [--since-hours]`, `sentinel-once`.
- 22 unit tests passing. `cargo fmt --check`,
  `cargo clippy -- -D warnings`, `cargo test` all green.

### Added — Documentation scaffold (prior)

- Foundational ADRs (0001–0015), AGENTS.md, CONTRIBUTING.md,
  architecture overview, MCP surface (since archived), proprioception
  design (since archived), templates.

### Next

- **Phase 1 — MVP Doctor.** Implement `russell help` by copying
  `stack-llm` per `REUSE_MANIFEST.md` §4.1, authoring ADR-0016
  (Doctor and LLM router) and ADR-0017 (Reuse-over-dependency),
  adding migration `0002_help_sessions.sql`, wiring the env
  loader, and writing the offline fallback. Then `cargo test`
  through the mock backend.

[Unreleased]: https://example.invalid/russell/compare/HEAD...HEAD

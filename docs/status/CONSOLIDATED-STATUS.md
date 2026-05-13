---
title: "Russell Consolidated Status"
audience: [operators, developers, contributors, architects, agents]
last_updated: 2026-05-12
togaf_phase: "G"
version: "2.1.0"
status: "Active"
---

# Russell Consolidated Status

<!-- TOGAF_DOMAIN: Governance -->
<!-- VERSION: 2.1.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-05-12 -->

**Single source of truth for "where is the project?"** Updated at the end
of every meaningful development session.

## 1. Headline

- **Phase 0 (skeleton, read-only observation) — COMPLETE** as of 2026-04-18.
- **Phase 1 (MVP Nurse — `russell jack`) — COMPLETE.**
- **Phase 1b (install artifacts + systemd units) — SHIPPED + installed.**
- **Phase 1c (20-day unattended soak) — CLOSED.**
- **Phase 2 (observation sharpened) — ACTIVE.** Self-vitals (5), rule engine, EWMA baselines, process probes (7), GPU probes (5), disk probes (2), systemd probes (3). Baseline deviation integrated into Jack's SOAP objective.
- **Phase 3 (skills and dispatch) — ACTIVE.** IDRS-gated skill dispatcher wired for production. `okapi-watcher` skill operational with `restart-okapi` intervention. Consent flow in `russell chat` — probes auto-execute, interventions accept natural-language consent ("ok", "yes", etc.) or `/approve`. `ACTION:` syntax parsed and executed by both `russell jack` and `russell chat`.
- **Architecture:** JR-1 austerity maintained throughout. Seven ADRs deferred.

## 2. What exists today

### Documentation

- Authority hierarchy + principles catalog (JR-1 through JR-7).
- UDQL-derived documentation standard.
- Pinned MVP boundary (`MVP_SPEC.md`).
- Persistence catalog (`PERSISTENCE_CATALOG.md`).
- Reuse manifest (`REUSE_MANIFEST.md`).
- TOGAF traceability matrix.
- 16 active ADRs + 8 deferred.
- 5 templates (ADR, skill manifest, SOAP bundle, daily log, review entry).

### Code

- Rust workspace with 7 crates (6 active, 1 stub —
  `russell-mcp` is a placeholder per ADR-0003).
- `russell-core` implements paths, event schema, profile,
  journal (SQLite + WAL + migrations), telemetry, time.
- `russell-sentinel` implements 25 probes across 7 categories:
  memory (3), swap (1), load (1), processes (7), GPU (5),
  disks (3), network (2), systemd (3). Plus Okapi probes via
  separate timer.
- `russell-proprio` implements the JR-5 self-vital plus 4 Phase-2A
  vitals: `sentinel_last_run_age_s`, `journal_writer_stall_s`,
  `llm_p95_latency_ms`, `timer_drift_s`, `help_error_rate_pct`.
  Includes `AutoimmuneGuard` (process-wide mutex for future meta-Doctor).
  Detects degraded internal state (slow LLM, journal stall, timer drift)
  before the operator notices. All vitals are read-only; no mutation.
- `russell-cli` implements ten verbs: `status`, `list`,
  `profile [--init]`, `digest`, `sentinel-once`, `jack`,
  `chat`, `skill list`, `skill run <id>`, `okapi-probe`,
  `proprio`.
- 159 tests passing.
- `cargo fmt --check` ✅, `cargo clippy -- -D warnings` ✅,
  `cargo test` ✅.

### Kask Integration

- **`arsenal-mcp-russell`** MCP tool server (6 tools) lives in the
  Kask repo (`~/Clones/kask`). Reads Russell's SQLite journal
  read-only; exposes `russell_host_snapshot`, `russell_journal_query`,
  `russell_recent_events`, `russell_probe_history`,
  `russell_health_summary`, and `russell_curator_assess`.
- **Duncan** — infrastructure Curator in Kask's
  `stack-control-plane`. Calls `russell_curator_assess` to produce
  health reports from Russell's telemetry.
- Registered in `~/.config/stack/mcp-registry.json`.
- **Integration boundary:** no cross-crate dependency between Russell
  and Kask. Communication is via Russell's SQLite journal (read-only
  from Kask's side) + the MCP tool server.

### Not yet

- Kask-aware probes: Ollama, disk, systemd services (Phase B of integration).
- Full MCP server surface (Phase 4).
- Corrective reflex arcs — require mutation and IDRS (deferred).
- Tier I / II / III separate cadences (Phase 4).
- Remote skill registry (Phase 4).
- Chaos probes (deferred).

## 3. Phase-by-phase plan

Versioning is empirical: a phase closes when its success criteria
are met on the observed machine.

### Phase 0 — Skeleton (COMPLETE)

- **Goal:** Cargo workspace compiles; read-only CLI verbs work;
  journal round-trips.
- **Success:** Achieved 2026-04-18. 22 tests green; end-to-end
  sandbox verified.

### Phase 1 — MVP Doctor (CURRENT)

- **Goal:** `russell jack` calls the configured LLM (default
  Ollama + `deepseekv4pro`; opt-in OpenRouter with ZDR),
  journals the round-trip, prints the response. Offline
  fallback works.
- **Tasks:**
  1. ~~Author ADR-0016 (Doctor and LLM router for MVP).~~ **Done 2026-04-18.**
  2. ~~Author ADR-0017 (Reuse over dependency).~~ **Done 2026-04-18.**
  3. ~~Copy `stack-llm` files per §4.1 of `REUSE_MANIFEST.md`;
     update §3 of that file with actual rows.~~ **Done 2026-04-18 (row 1 populated; openai.rs + wire.rs pattern-copied into `russell-doctor::openrouter`).**
  4. ~~Author persona `crates/russell-doctor/prompts/jack.md`.~~ **Done 2026-04-18 (131 lines).**
  5. ~~Implement `russell-doctor::help::compose` (SOAP assembly).~~ **Done.**
  6. ~~Implement `russell-doctor::help::call` (LLM round-trip).~~ **Done (`openrouter::OpenRouterClient`).**
  7. ~~Implement `russell-doctor::help::fallback` (offline
     rule-based summary).~~ **Done.**
  8. ~~Add migration `0002_help_sessions.sql`.~~ **Done.**
  9. ~~Implement `russell-cli::commands::help` → `russell jack`.~~ **Done.**
  10. ~~Wire env loader in `russell-core::env`.~~ **Done.**
  11. ~~Tests: mock backend round-trip, offline fallback,
      migration idempotence, SOAP composition snapshot.~~ **Done (14 new tests in `russell-doctor`, 3 in `russell-core::env`).**
- **Success:**
  1. `russell jack` returns a response from the configured LLM
     (Ollama by default; OpenRouter if opted in).
  2. `russell jack` returns a rule-based summary when the LLM
     is unreachable.
  3. `help_sessions` rows are journaled and surface in
     `russell list` / `russell digest`.
  4. Evidence bundle under `evidence/help/<id>/` contains
     `soap.md`, `request.json`, `response.json`,
     `transcript.jsonl`.
  5. All CI gates green.

### Phase 1b — Install artifacts (COMPLETE)

- **Goal:** Russell installs cleanly as a systemd user timer.
- **Shipped 2026-04-18:**
  - `packaging/systemd/russell-sentinel.{timer,service}` — 5-minute cadence, persistent (catches up after sleep).
  - `packaging/systemd/russell-digest.{timer,service}` — Sunday 09:00 weekly.
  - `packaging/systemd/russell-failure@.service` — templated failure capture.
  - `packaging/bin/install.sh` / `uninstall.sh` — idempotent, no-root.
  - `.env.example` — template; seeded into `~/.config/harness/russell.env` on first install.
  - [`docs/operations/INSTALL.md`](../operations/INSTALL.md) — operator runbook.
  - Env discovery in `russell-core::env::load_discovered`: process env → `~/.config/harness/russell.env` → repo `.env` → `./.env`.
- **Next empirical gate:** 7-day unattended soak on the observed machine.

### Phase 1c — MVP Close (the 20-day soak) — COMPLETE

- **Goal:** Unattended operation per
  [`../specifications/MVP_SPEC.md`](../specifications/MVP_SPEC.md) §6.
- **Success:** Closed 2026-05-06 per ADR-0018. 20 days, 2 062 cycles, 99.95% reliability.

### Phase 2 — Observation sharpened (CURRENT)

Rule engine, EWMA baselines, self-vital, sample summary in SOAP.
Phase 1c is closed; work begins.

- [x] JR-5 self-vital (`sentinel_last_run_age_s`) in `russell-proprio`
- [x] Markdown memory layer (ADR-0022): paths, persistence catalog, identity files, Doctor integration, daily log verb (`russell digest --format daily-log`)
- [x] Proprioception Phase 2A (ADR-0021): 4 new self-vitals (`journal_writer_stall_s`, `llm_p95_latency_ms`, `timer_drift_s`, `help_error_rate_pct`) + `AutoimmuneGuard`
- [x] Rule engine: per-probe TOML rules (`rules.d/*.toml`) with operator-overridable thresholds, `RuleSet` in `russell-core`, wired into `sentinel-once`
- [x] EWMA baselines (30-day rolling p50/p95/p99): `compute_baselines()` query + `upsert_baseline()` writer + daily refresh in sentinel-once
- [x] Fix F-2: extend `prompt::compose` with 24h sample summary (per-probe min/avg/max/last/count table)

### Phase 3 — Skills and dispatch (COMPLETE)

Skill manifest loader, dispatcher, first host-scope skill, IDRS
journaling, risk enforcement, rollback, `russell chat`.
ADR-0007 deferral lifted per ADR-0023.

- [x] ADR-0023: formal lift of ADR-0007 deferral
- [x] `russell-skills` crate: manifest parser, validation, symptom catalog
- [x] Subprocess dispatcher: env scrubbing, timeout, stdout/stderr capture, dry-run
- [x] CLI verbs: `russell skill list`, `russell skill run <id> [--dry-run]`
- [x] First skill: `gpu-doctor` fixture (manifest + rocm-smi probe script)
- [x] Nurse integration: skills table in SOAP prompt, RECOMMEND format
- [x] IDRS journaling: `run_and_journal` with evidence bundles per run
- [x] Risk-band enforcement: `max_auto_risk` cap, `check_risk()` gate
- [x] Rollback execution: `run_intervention_with_rollback()` chains reverse interventions
- [x] `russell chat` — interactive readline REPL with Jack's chat persona
- [x] Persona shift: Jack is a nurse, not a doctor

### Phase 4 — MCP surface, real skills, operational depth (NEXT)

MCP server is still a stub crate. No live Ollama/GPU/disk probes
beyond the MVP three. No operator-facing skill authoring tools.

## 4. Open questions

- (ADR-0016 v2) Default backend is Ollama with model
  `deepseekv4pro`. OpenRouter is opt-in via
  `RUSSELL_DOCTOR_BACKEND=openrouter`.
- ADR-0016 settled the retry policy: single round-trip, no retry.
- `russell-doctor` split into its own crate during Phase 1.

## 5. Risk register (current)

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| OpenRouter API surface changes | Low | Medium | Copy pattern isolates us; sync log in `REUSE_MANIFEST.md` §6 |
| `stack-llm` upstream diverges | Low–Med | Low | JR-6 discipline; we copied deliberately |
| Operator's Ollama not installed or model not pulled | Med | Med | Offline fallback always works; operator docs updated |
| Ollama or OpenRouter becomes slow | Low | Medium | Phase-2 llm_p95_latency_ms vital |
| Journal grows unboundedly | Med (long run) | Med | Phase-2 compaction skill; MVP note in `PERSISTENCE_CATALOG.md` |

## 6. Housekeeping debt

- None at 2026-04-18. Corpus is freshly audited.

## 7. How this file is maintained

This file is updated at the end of every development session that
produces a meaningful change. The update is part of the same
commit as the change. Stale status here = broken contract with
every other reader of this corpus.

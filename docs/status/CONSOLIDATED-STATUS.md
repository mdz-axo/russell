---
title: "Russell Consolidated Status"
audience: [operators, developers, contributors, architects, agents]
last_updated: 2026-05-23
togaf_phase: "G"
version: "3.1.0"
status: "Active"
---

# Russell Consolidated Status

<!-- TOGAF_DOMAIN: Governance -->
<!-- VERSION: 3.1.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-05-23 -->

**Single source of truth for "where is the project?"** Updated at the end
of every meaningful development session.

## 1. Headline

- **Security Hardening (Phase 5) — COMPLETE** (2026-05-23). Unified risk types, DNS rebinding protection, configurable endpoints, service token auth, hardened hash chain, hexagonal ports. 288 tests pass.
- **Documentation Refresh — COMPLETE** (2026-05-22). 35 files archived, 79 retained. Link integrity improved.
- **Phase 0 (skeleton, read-only observation) — COMPLETE** as of 2026-04-18.
- **Phase 1 (MVP Nurse — `russell jack`) — COMPLETE.**
- **Phase 1b (install artifacts + systemd units) — SHIPPED + installed.**
- **Phase 1c (20-day unattended soak) — CLOSED.**
- **Phase 2 (observation sharpened) — COMPLETE.** Self-vitals (5), rule engine, EWMA baselines, process probes (7), GPU probes (5), disk probes (2), systemd probes (3). Baseline deviation integrated into Jack's SOAP objective.
- **Phase 3 (skills and dispatch) — COMPLETE.** Extended with skill lifecycle management: workshop REPL (`russell workshop`), registry cache (`local-cache.yaml`), safety scanner (7 rules for manifest + KNOWLEDGE.md), skill discovery pipeline, and scenario testing skill (`scenario-tester`). 12 skills loaded (5 actionable with probes, 7 knowledge). `russell skill run` respects manifest `timeout:` field.
- **Phase 4 (MCP surface, real skills, operational depth) — COMPLETE.** All skill lifecycle gaps closed (2026-05-20): `fetch <url> <name>` downloads skills from URLs with safety scanning, `build <name>` creates skill skeletons on disk, `adapt <name>` modifies manifests via LLM + editor, `search --remote` loads `~/.config/harness/registry-sources.yaml` and queries Brave Search API, batch operations (`prune --all-stale`, `install --all-evaluated`). `skill-manager` bundled meta-skill enables Jack to build, install, prune, restore, and delete skills from chat via ACTION syntax. Registry telemetry wired: `probe_runs`, `intervention_runs`, `avg_probe_duration_ms`, and `last_probe_run_at` updated on every execution. Quality scoring (`compute_score()`) operational. End-to-end scenario pipeline: `scenario-full` probe chains run-okapi → evaluate → journal. 288 tests pass. 21 scenario tests pass.
- **Architecture:** JR-1 austerity maintained throughout. Seven ADRs deferred.

## 2. What exists today

### Documentation

- **79 active files** (2026-05-22 refresh; 35 archived)
- Authority hierarchy + principles catalog (JR-1 through JR-7).
- UDQL-derived documentation standard.
- Pinned MVP boundary (`MVP_SPEC.md`).
- Persistence catalog (`PERSISTENCE_CATALOG.md`).
- Reuse manifest (`REUSE_MANIFEST.md`).
- TOGAF traceability matrix.
- 17 active ADRs + 7 deferred.
- 5 templates (ADR, skill manifest, SOAP bundle, daily log, review entry).
- Archive: `docs/archive/2026-05-22-documentation-refresh/` (phase logs, analysis, superseded proposals)

### Code

- Rust workspace with 11 crates (all active).
- `russell-core` implements paths, event schema (`Severity`/`Scope`
  with `Display`+`FromStr`), profile, journal (SQLite + WAL +
  migrations), telemetry, time (`approx_days_between`).
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
- `russell-cli` implements 18 subcommands: `status`, `pod-status`,
  `pod-activate`, `pod-deactivate`, `pod-persona-show`,
  `pod-artifacts-list`, `pod-artifacts-export`, `list`,
  `digest`, `sentinel-once`, `jack`, `skill-list`, `skill-run`,
  `skill-install`, `skill-prune`, `proprio`, `self-triage`,
  `docs`, `verify-journal`.
- 288 tests passing.
- 21 scenario tests passing (`scenario-tests.sh`).
- 12 skills loaded (okapi-watcher, web-search, skill-discovery,
  skill-workshop, skill-maintenance, scenario-tester, sysadmin,
  pragmatic-cybernetics, pragmatic-semantics, ubuntu-jack,
  oom-watcher, **skill-manager**). 5 actionable (okapi-watcher,
  sysadmin, scenario-tester, oom-watcher, skill-manager), 7 knowledge.
- Skill registry telemetry active: `probe_runs`,
  `intervention_runs`, `avg_probe_duration_ms` (EWMA), and
  `last_probe_run_at` updated on every execution in chat and CLI.
- Quality scoring operational via `compute_score()`.

### hKask Integration

- **Bidirectional MCP integration** (ADR-0025). Russell is both
  a server and a client to hKask:
  - **hKask → Russell:** `arsenal-mcp-russell` MCP tool server
    (7 tools: `russell_host_snapshot`, `russell_self_vital`,
    `russell_journal_query`, `russell_help_sessions`,
    `russell_curator_assess`, `russell_cadence_health`,
    `russell_token_status`). Lives in hKask repo
    (`~/Clones/hKask`); reads Russell's SQLite journal read-only.
  - **Russell → hKask:** `russell-mcp` client crate connects to
    hKask's `stack-api` gateway (`http://127.0.0.1:8080`) with
    bearer-token auth. 193 tools across 16 MCP servers: web
    search, scholar, RSS, finance, image/video generation, email,
    SMS/voice, embeddings, document knowledge, capability
    ontology, fine-tuning, Okapi metrics, gallery, maintenance,
    and Russell telemetry.
- 16 MCP servers registered in `~/.config/stack/mcp-registry.json`.
- **Duncan** — infrastructure Curator in hKask's
  `stack-control-plane`. Calls `russell_curator_assess` to produce
  health reports from Russell's telemetry.
- **Integration boundary:** no cross-crate dependency between Russell
  and hKask. Communication is via SQLite journal (hKask reads Russell)
  + HTTP REST to stack-api (Russell calls hKask tools).

### Not yet

- Corrective reflex arcs — require mutation and IDRS.
- Tier I / II / III separate cadences.
- Remote skill registry sync (`registry-sources.yaml` schema defined, `fetch --remote` deferred).
- Skill probe telemetry (`probe_runs` counters wired to Dispatcher — resolved 2026-05-14).
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

### Phase 4 — MCP surface, real skills, operational depth (CURRENT)

`russell-mcp` is an operational MCP client (ADR-0025) connecting to
Kask's `stack-api` gateway. Russell has access to 193 tools across
16 MCP servers registered in the Kask mcp-registry. Skill lifecycle
is operational with registry cache, workshop REPL, safety scanner,
and scenario testing pipeline. The `skill-manager` bundled meta-skill
enables in-chat skill management (build, install, prune, restore,
delete) via ACTION syntax. Registry telemetry (`probe_runs`,
`intervention_runs`, EWMA duration, last-run timestamps) is wired into
the dispatch path. The skill catalogue covers 12+ symptoms with installed
skills (up from 3 at Phase 3 close). Scenario metrics feed into the
sentinel rule engine. The `oom-watcher` skill demonstrates end-to-end
build→install→test→sentinel flow.

- [x] **EWMA statistics**: `ewma_mean`/`ewma_var` in `BaselineRow`, computed with
  7-day half-life over timestamp-ordered series. Stored via `upsert_baseline()`
  and surfaced in Jack's SOAP sample table alongside p95.
- [x] **Rate-of-change rules**: `rate_warn`/`rate_alert`/`rate_crit` fields on
  `Rule`, factory defaults for GPU temp (0.5/1.5/5.0 °C/s), VRAM used
  (51.2/102.4/512.0 MiB/s), and disk usage (0.5/2.0/5.0 %/s).
  `evaluate_samples_with_rates()` emits `rate_breach` events when the absolute
  rate exceeds thresholds. Rate computed from the previous journaled sample.
- [x] **Reflex arc engine**: `ReflexSet` in `russell-core::reflex`, TOML-based
  arcs (`arc.d/*.toml`) mapping (probe, severity) → (intervention, cooldown,
  max_retries). Factory default arc: `disk_root_used_pct/alert → sysadmin/sweep-caches`
  with 1h cooldown. Evaluated after threshold/rate breaches in `sentinel-once`;
  emits `reflex_proposed` events consumed by the Nurse SOAP.
- [x] **Reflex → Nurse wiring**: `build_reflex_section()` in `prompt.rs` renders
  dedicated "Reflex arcs — proposed interventions" table in Jack's SOAP.
  `JournalReader::list_reflex_events()` and `count_reflex_events()` for cooldown
  enforcement.
- [x] **IDRS rollback & evaluation wired**: `run_intervention_with_rollback()`
  used in chat.rs consent path; `eval_checks` resolved from skill manifests and
  passed through `ResolvedAction::Intervention`. Rollback strategies
  (RollbackId, NoneNeeded, Reboot) propagated from manifest to dispatcher.
- [x] **HKaskTool IDRS parity**: evidence bundles for HKask MCP tool calls
  (`evidence/hkask/<tool>/<ts>/result.txt + event.json`), per-tool timeout (30s/120s),
  unified `RiskBand` enum (no more string comparison).
- [x] **AutoimmuneGuard wired**: `AUTOIMMUNE` static guard acquired in
  `run_once()`, `run_once_with()`, and `run_once_with_hkask()`.
- [x] **Sentinel watchdog**: `TimeoutStartSec=120` on `russell-sentinel.service`.

## 4. Open questions

- (ADR-0016 v2) Default backend is Ollama with model
  `deepseekv4pro`. OpenRouter is opt-in via
  `RUSSELL_DOCTOR_BACKEND=openrouter`.
- ADR-0016 settled the retry policy: single round-trip, no retry.
- `russell-doctor` split into its own crate during Phase 1.
  (Renamed to `russell-meta` per ADR-0026, 2026-05-15. Historical
  references to `russell-doctor` in changelog entries below refer to
  the same crate now named `russell-meta`.)

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

---
title: "Russell Consolidated Status"
audience: [operators, developers, contributors, architects, agents]
last_updated: 2026-05-06
togaf_phase: "G — Governance"
version: "2.0.0"
status: "Active"
---

# Russell Consolidated Status

<!-- TOGAF_DOMAIN: Governance -->
<!-- VERSION: 2.0.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-05-06 -->

**Single source of truth for "where are we?"** Updated at the end
of every meaningful development session.

## 1. Headline

- **Phase 0 (skeleton, read-only observation) — COMPLETE** as of
  2026-04-18.
- **Phase 1 (MVP Doctor — `russell jack`) — IMPLEMENTED + verified against real Kimi K2.5.**
- **Phase 1b (install artifacts + systemd units) — SHIPPED + installed on the observed machine.** 5-min timer firing; 44 tests green.
- **Phase 1c (20-day unattended soak) — CLOSED.** Day 20 (2026-05-06): 2 062 cycles, ~99.95% reliability. Closed per [ADR-0018](../adr/0018-close-phase-1c.md). Findings F-1 through F-9 recorded in [`SOAK_FINDINGS.md`](SOAK_FINDINGS.md); F-7 (JR-5 self-vital) and F-2 (SOAP samples) carry into Phase 2.
- **Phase 2 (observation sharpened) — OPEN.** Self-vital, rule engine, EWMA baselines. Spec pinned at [`../specifications/MVP_SPEC.md`](../specifications/MVP_SPEC.md).
- **Architecture pivoted to JR-1 austerity** on 2026-04-18.
  Seven ADRs deferred to `adr/deferred/`; two architecture docs
  archived.

## 2. What exists today

### Documentation

- Authority hierarchy + principles catalog (JR-1 through JR-7).
- UDQL-derived documentation standard.
- Pinned MVP boundary (`MVP_SPEC.md`).
- Persistence catalog (`PERSISTENCE_CATALOG.md`).
- Reuse manifest (`REUSE_MANIFEST.md`, forward-looking).
- TOGAF traceability matrix.
- 10 active ADRs + 7 deferred.
- 3 templates (ADR, skill manifest, SOAP bundle).

### Code

- Rust workspace with 7 crates (2 active, 5 stubs per JR-6
  scaffold-for-later discipline).
- `russell-core` implements paths, event schema, profile,
  journal (SQLite + WAL + migrations), telemetry, time.
- `russell-sentinel` implements three `/proc`-based probes.
- `russell-cli` implements five read-only verbs: `status`,
  `list`, `profile [--init]`, `digest`, `sentinel-once`.
- 44 tests passing.
- `cargo fmt --check` ✅, `cargo clippy -- -D warnings` ✅,
  `cargo test` ✅.

### Not yet

- JR-5 self-vital (`sentinel_last_run_age_s`) in `russell-proprio` (Phase 2).
- Rule engine (`rules.d/*.toml`, lift ADR-0012) (Phase 2).
- EWMA baselines (30-day rolling p50/p95/p99) (Phase 2).
- Fix F-2: extend `prompt::compose` with 24h sample summary (Phase 2).
- Skill dispatcher, tier engines, MCP server (post-MVP).

## 3. Phase-by-phase plan

Versioning is empirical: a phase closes when its success criteria
are met on the observed machine.

### Phase 0 — Skeleton (COMPLETE)

- **Goal:** Cargo workspace compiles; read-only CLI verbs work;
  journal round-trips.
- **Success:** Achieved 2026-04-18. 22 tests green; end-to-end
  sandbox verified.

### Phase 1 — MVP Doctor (CURRENT)

- **Goal:** `russell help` calls Kimi K2 via OpenRouter (ZDR),
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
  1. `russell help` returns a response from Kimi K2 when
     `OPENROUTER_API_KEY` is set.
  2. `russell help` returns a rule-based summary when the key
     is unset.
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

- [ ] JR-5 self-vital (`sentinel_last_run_age_s`) in `russell-proprio`
- [ ] Rule engine (`rules.d/*.toml`, lift ADR-0012)
- [ ] EWMA baselines (30-day rolling p50/p95/p99)
- [ ] Fix F-2: extend `prompt::compose` with 24h sample summary

### Phase 3 — Skills and dispatch

Skill manifest loader, dispatcher, first host-scope skill
(`gpu-doctor`). Requires formal lifting of ADR-0007's
deferral.

### Phase 4+ — Tracks the design document

Remote skill registry, MCP surface, full proprioception, chaos
probes. Each requires lifting its ADR's deferral with evidence
that the simpler layer beneath has stabilised.

## 4. Open questions

- (ADR-0016 will answer) Exact default model — `moonshotai/kimi-k2.5`
  as MVP default; `moonshotai/kimi-k2` is an accepted alternative
  if multimodal is not needed.
- (ADR-0016 will answer) Retry policy — copied from `stack-llm`;
  determine max attempts.
- (Phase 1) Whether `russell-doctor` earns its own crate from day
  one (decision: **yes**, per user direction).

## 5. Risk register (current)

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| OpenRouter API surface changes | Low | Medium | Copy pattern isolates us; sync log in `REUSE_MANIFEST.md` §6 |
| `stack-llm` upstream diverges | Low–Med | Low | JR-6 discipline; we copied deliberately |
| Operator forgets to set `OPENROUTER_API_KEY` | High | Low | Offline fallback always works |
| ZDR endpoint unavailable for chosen model | Low | Low | Fallback to local Ollama or offline path |
| Journal grows unboundedly | Med (long run) | Med | Phase-2 compaction skill; MVP note in `PERSISTENCE_CATALOG.md` |

## 6. Housekeeping debt

- None at 2026-04-18. Corpus is freshly audited.

## 7. How this file is maintained

This file is updated at the end of every development session that
produces a meaningful change. The update is part of the same
commit as the change. Stale status here = broken contract with
every other reader of this corpus.

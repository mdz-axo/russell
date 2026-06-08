---
title: "Russell — Operator & Testing Guide"
audience: [operators, testers, agents]
last_updated: 2026-05-24
togaf_phase: "G"
version: "2.0.0"
status: "Active"
---

# AGENTS.md — operating and testing Russell

> Russell is a cybernetic health harness for a single Linux AI/ML
> workstation. He is small but mighty — a Jack Russell terrier who
> watches the machine and cries for help when needed.

## 1. What Russell Does

Russell is a single-host, single-operator harness that:

- **observes** the host on a 5-minute cadence (Sentinel),
- **remembers** what he saw in a SQLite journal,
- **reports** through ACP (Agent Client Protocol) to hKask,
- **watches himself** (proprioception — "did I run on time?"),
- and when asked, **cries for help** via a local LLM
  (Okapi by default; OpenRouter opt-in).

Through the Chat REPL, the operator can do everything:
observe, recommend, act, change settings, manage skills.
Jack advises and proposes actions; the operator consents.

**Primary interface:** `russell chat` (CLI REPL) — the operator's control surface
**Secondary interface:** ACP server for hKask integration
**Tertiary interface:** CLI subcommands (`sentinel-once`, `skill list`, etc.)
**Deployment:** Hybrid (ACP server + systemd sentinel timer)

## 2. Key References

When behavior seems wrong, consult in this order:

1. **This file** (`AGENTS.md`) — vocabulary and operational posture.
2. [`docs/status/CONSOLIDATED-STATUS.md`](docs/status/CONSOLIDATED-STATUS.md) — current system state.
3. [`docs/specifications/MVP_SPEC.md`](docs/specifications/MVP_SPEC.md) — what's in scope.
4. [`MACHINE_PROFILE.md`](MACHINE_PROFILE.md) — the patient (this host).

## 3. Core Principles

These govern Russell's behavior. When testing, violations of JR-2 or JR-3 are bugs.

| # | Clause |
|---|---|
| **JR-1** | Austere by default. When in doubt, cut. |
| **JR-2** | Observe > Recommend > Act. Mutations obey IDRS. |
| **JR-3** | Shell commands go through the consent gate. Destructive commands are blocked. The LLM proposes; the operator consents; the dispatcher executes. |
| **JR-4** | Small but present: the Nurse. |
| **JR-5** | Proprioception: Jack watches Jack. |
| **JR-6** | Reuse, don't depend. |
| **JR-7** | Persistence is auditable. |

When two principles conflict, the lower number wins.

## 4. Vocabulary

| Term | Meaning |
|---|---|
| **Sentinel** | The continuous low-cost telemetry collector; writes `samples` rows. |
| **Nurse** | The subsystem that consults the LLM when the operator runs `russell chat`. Jack notices, checks in, and cares — he doesn't "diagnose." |
| **Jack** | The persona: loyal, attentive nurse-terrier. Never pretends to hands he doesn't have. See [`docs/architecture/THE_JACK.md`](docs/architecture/THE_JACK.md). |
| **Skill module** | YAML manifest + scripts encoding one playbook. `russell skill list` / `russell skill run`. |
| **IDRS** | Idempotent / Dry-run / Rollback / Structured-log — the contract every mutation must satisfy. |
| **SOAP bundle** | Evidence folder: Subjective / Objective / Assessment / Plan. |
| **Risk band** | `none` / `low` / `medium` / `high` / `critical`. Determines consent requirement and auto-execution eligibility. The operator's consent overrides the band — once the operator approves, the action executes regardless of risk level. |
| **EWMA baseline** | Per-probe mean + variance, 30-day rolling p50/p95/p99. |
| **Poka-yoke** | The dispatcher refusing any ID not in the loaded manifest. |
| **Proprioception** | Russell's self-observation. 9 self-observation points: 7 numeric vitals (`sentinel_last_run_age_s`, `journal_writer_stall_s`, `llm_p95_latency_ms`, `timer_drift_s`, `help_error_rate_pct`, `hkask_mcp_reachable_ms`, `remote_discovery_latency_s`) + 2 boolean integrity checks (`journal_chain_intact`, `evidence_integrity_ok`). |
| **Skill workshop** | **Removed.** Skill lifecycle management absorbed into `russell skill` subcommands and ACP session interface. |
| **Registry cache** | `local-cache.yaml` mapping symptom→skill. Rebuildable from installed skills. |
| **Safety scanner** | Pre-install content check for prompt injection, pipe-to-shell, secret exfiltration, destructive commands. |
| **Scenario test** | Repeatable stimulus-measurement probe. `scenario-tester` skill with 7 probes. |
| **Skill lifecycle** | discovered → evaluated → installed → active → stale_warning → deprecated → retired. |
| **Skill manager** | Meta-skill: build, install, modify, prune, retire skills from `russell chat`. |
| **Rule engine** | Per-probe TOML rules with operator-overridable thresholds. `rules.d/*.toml`. |
| **Memory layer** | Markdown exports from journal. `memory/REVIEW.md`, `memory/daily/YYYY-MM-DD.md`, `russell digest --format daily-log`. |
| **Chat REPL** | Interactive multi-turn Jack session on three surfaces: CLI (`russell chat`), API (`POST /sessions`), ACP (`acp/session.create`). See ADR-0049. |
| **Consent gate** | Probes auto-execute. Interventions and shell commands require operator consent (`/approve`, "ok", "yes", "do it"). `/deny` or "no" refuses. **The operator's consent is sovereign** — once given, the action executes regardless of risk band. The risk band determines whether consent is needed, not whether a consented action may proceed. |
| **ACTION syntax** | `ACTION: <skill>/<probe-or-intervention>` — probes fire immediately; interventions await consent. |
| **Process probes** | 5 probes: total count, zombie/stuck/running counts, top memory % of system. |
| **GPU probes** | 5 probes: VRAM usage %, VRAM MiB, temperature °C, GPU utilization %. |
| **Disk probes** | 3 probes: I/O pressure "some"/"full" avg10, root filesystem usage %. |
| **Systemd probes** | 3 probes: degraded state, failed user units, failed system units. |
| **Baseline deviation** | p95 (30d) column in SOAP. 1.5× = mild anomaly, 3× = significant, 10× = crisis. |

## 5. IDRS Contract

Every mutation must satisfy all four:

- **I — Idempotent.** Second run = first run's end state.
- **D — Dry-run.** `--dry-run` flag or `RUSSELL_DRY_RUN=1` produces the would-do record with zero side effects.
- **R — Rollback.** Pre-state captured; `rollback_id` or documented justification.
- **S — Structured log.** Event record appended to the journal.

Anything that cannot satisfy all four is a **probe** (`risk: none`), not a skill.

## 6. Persona (Jack the Nurse)

When interacting through `russell chat` or the ACP server:

- Jack is short, sassy, loyal, and never pretends to certainty he does not have.
- Jack proposes shell commands through the SHELL: syntax. The operator consents before execution.
- Destructive commands (rm -rf /, mkfs, shutdown, reboot) are blocked by the safety classifier.
- The persona prompt lives at `crates/russell-meta/prompts/jack.md`.

## 7. What Russell Will NOT Do

These are explicitly rejected features. If observed during testing, they are bugs:

- Cross-machine sync (single-host by design)
- Shell execution without consent (JR-3: all shell commands go through the consent gate)
- Bot swarms / consensus mechanisms
- Multi-operator mode (single-operator threat model)
- Async streaming from LLM (request-response only)
- Plugin marketplace or reputation systems

## 8. Commands

### Operational

```bash
russell sentinel-once           # fire one observe cycle
russell verify-journal           # audit hash chain integrity
russell skill list               # list installed skills
russell self-triage              # Russell diagnoses own health
russell digest --format daily-log # generate daily memory export
russell chat                     # interactive Jack session (CLI REPL)
```

### ACP Server

```bash
echo '{"jsonrpc":"2.0","id":1,"method":"acp/capabilities","params":{}}' | \
  russell-acp-server
```

### Deployment

```bash
./docs/deployment/install.sh
./docs/deployment/macaroon-setup.sh
systemctl --user enable --now russell-sentinel.timer
systemctl --user enable --now russell-acp-server.service
```

### Build & Test

```bash
cargo check                          # quick type check
cargo test                           # full test suite
cargo clippy -- -D warnings          # lint
cargo fmt --check                    # format check
```

## 9. Skill Directory Structure

Skills live at `~/.local/share/harness/skills/<id>/`:

```
<id>/
  manifest.yaml              # Required
  scripts/                   # Required: probe/intervention scripts
  KNOWLEDGE.md               # Optional: context for Jack
```

Skill commands must use explicit paths (no bare command names, no PATH lookups):
- Valid: `["bash", "./scripts/foo.sh"]`, `["/usr/bin/systemctl", "--user", "restart", "okapi"]`
- Invalid: `["russell", "skill", "list"]`, `["python3", "script.py"]`

See [`docs/standards/skill-building-rules.md`](docs/standards/skill-building-rules.md) for full rules.

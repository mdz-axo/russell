---
title: "Russell — Agent Orientation"
audience: [agents, operators, developers, contributors, architects]
last_updated: 2026-05-19
togaf_phase: "Preliminary / Governance"
version: "1.1.0"
status: "Active"
---

<!-- TOGAF_DOMAIN: Governance -->
<!-- VERSION: 1.2.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-05-22 -->

# AGENTS.md — contributing to Russell

> Russell is a cybernetic health harness for a single Linux AI/ML
> workstation. He is small but mighty — a Jack Russell terrier who
> watches the machine and cries for help when needed.
>
> This file is the **binding orientation document** for any human,
> AI, or subagent touching the code, docs, or skill catalogue.
> Read it before you edit anything.

## 1. What Russell is (one paragraph)

Russell is a single-host, single-operator harness that:

- **observes** the host on a 5-minute cadence (Sentinel),
- **remembers** what he saw in a SQLite journal,
- **reports** through ACP (Agent Client Protocol) to hKask,
- **watches himself** (proprioception — "did I run on time?"),
- and when asked, **cries for help** via a local LLM
  (Okapi by default; OpenRouter opt-in).

He does *not* mutate host state (outside his own skill sandbox)
or act on LLM output as shell commands. Those lanes are
guarded by IDRS and JR-3.

**Primary interface:** ACP server for hKask integration  
**Secondary interface:** CLI for local operator actions  
**Deployment:** Hybrid (ACP server + systemd sentinel timer)

## 2. Authority Hierarchy

When claims conflict, precedence is:

1. **This file** (`AGENTS.md`) — binding vocabulary and posture.
2. [`docs/README.md`](docs/README.md) — portal and critical-set
   declaration.
3. [`docs/status/CONSOLIDATED-STATUS.md`](docs/status/CONSOLIDATED-STATUS.md)
   — where we actually are.
4. [`docs/specifications/MVP_SPEC.md`](docs/specifications/MVP_SPEC.md)
   — the pinned MVP boundary.
5. [`docs/architecture/PRINCIPLES_CATALOG.md`](docs/architecture/PRINCIPLES_CATALOG.md)
   — the JR principles.
6. Active ADRs under [`docs/adr/`](docs/adr/).
7. [`cybernetic-health-harness.md`](cybernetic-health-harness.md)
   — the full design (aspirational).

## 3. Reading Order (do not skip)

1. This file — the rules.
2. [`docs/README.md`](docs/README.md) — what's where.
3. [`docs/architecture/PRINCIPLES_CATALOG.md`](docs/architecture/PRINCIPLES_CATALOG.md)
   — **read JR-1 through JR-7 or do not touch this code.**
4. [`docs/specifications/MVP_SPEC.md`](docs/specifications/MVP_SPEC.md)
   — the pinned boundary.
5. [`docs/architecture/THE_JACK.md`](docs/architecture/THE_JACK.md)
   — who Jack is.
6. [`docs/standards/agent-operating-rules.md`](docs/standards/agent-operating-rules.md)
   — inherited rules about workspace hygiene, verification,
   honesty.
7. [`docs/standards/safety.md`](docs/standards/safety.md) — the
   IDRS contract.
8. [`MACHINE_PROFILE.md`](MACHINE_PROFILE.md) — the patient.

## 4. The Seven Principles (shorthand)

These live in full at
[`docs/architecture/PRINCIPLES_CATALOG.md`](docs/architecture/PRINCIPLES_CATALOG.md).

| # | Clause |
|---|---|
| **JR-1** | *Though she be but little, she is fierce.* Austere by default. When in doubt, cut. |
| **JR-2** | Observe > Recommend > Act. Mutations obey IDRS. |
| **JR-3** | The LLM never emits shell. It ranks IDs; it does not compose commands. |
| **JR-4** | Small but present: the Nurse. `russell jack` exists from day one. |
| **JR-5** | Proprioception: Jack watches Jack. One self-vital is non-optional. |
| **JR-6** | Reuse, don't depend. Copy-with-provenance via `REUSE_MANIFEST.md`. |
| **JR-7** | Persistence is auditable. Registered in `PERSISTENCE_CATALOG.md`. |

When two principles conflict, the lower number wins.

## 5. The Vocabulary (binding)

Use these terms exactly. New terminology requires a same-PR
addition to this table.

| Term | Meaning |
|---|---|
| **Sentinel** | The continuous low-cost telemetry collector; writes `samples` rows. |
| **Nurse** | The subsystem that consults the LLM (via Okapi) when the operator runs `russell jack` or `russell chat`. Jack watches over the machine; he doesn't "diagnose" — he notices, checks in, and cares. Implemented in `russell-meta` (the metacognitive layer — see ADR-0026). |
| **Jack** | The persona: terrier + *Will & Grace* Jack McFarland + Rust/Linux/cybernetics fluency. He's a nurse, not a doctor — loyal, attentive, never pretends to hands he doesn't have. See [`docs/architecture/THE_JACK.md`](docs/architecture/THE_JACK.md). |
| **Skill module** | YAML manifest + scripts encoding one playbook. **Active.** `russell-skills` crate with manifest parser, dispatcher, CLI verbs (`russell skill list`, `russell skill run`). First real skill: `okapi-watcher`. |
| **IDRS** | Idempotent / Dry-run / Rollback / Structured-log contract for every mutation. **Active.** `run_and_journal` writes evidence bundles; `run_intervention_with_rollback` chains reverse interventions. |
| **SOAP bundle** | Evidence folder laid out Subjective / Objective / Assessment / Plan. |
| **Honeymoon window** | First 30 days after bootstrap; elevated caution. Deferred. |
| **Risk band** | `none` / `low` / `medium` / `high` / `critical`. Enforced by dispatcher's `max_auto_risk` cap and `check_risk()` gate. |
| **EWMA baseline** | Per-probe mean + variance, 30-day rolling p50/p95/p99. **Active.** `compute_baselines()` + daily refresh. |
| **Chaos probe** | Deliberate bounded failure to verify recovery. Deferred. |
| **Poka-yoke** | The dispatcher refusing any ID not in the loaded manifest. **Active.** Validated at dispatch time. |
| **Andon cord** | Operator's stop-the-line signal. Deferred. |
| **Proprioception** | Russell's self-observation. **Active.** 5 self-vitals: `sentinel_last_run_age_s`, `journal_writer_stall_s`, `llm_p95_latency_ms`, `timer_drift_s`, `help_error_rate_pct`. |
| **Meta-Sentinel** | The self-facing Sentinel. Full form deferred; 5-vital form active. |
| **Skill workshop** | Interactive skill lifecycle REPL. **Active.** `russell workshop` — a focused chat session where Jack helps discover, evaluate, build, adapt, and maintain skills. 17 built-in commands. |
| **Registry cache** | Local YAML file (`local-cache.yaml`) mapping symptom→skill for decision support. **Active.** Rebuildable from installed skills (JR-7). Tracks lifecycle state, source, evaluation metadata, and telemetry (`probe_runs`, `intervention_runs`, `avg_probe_duration_ms`, `last_probe_run_at`). |
| **Safety scanner** | Pre-install content check for prompt injection, pipe-to-shell, secret exfiltration, and destructive commands. **Active.** 7 rule categories. Runs on manifest.yaml and KNOWLEDGE.md. |
| **Scenario test** | Repeatable stimulus-measurement probe for agentic AI systems (Okapi, hKask, Russell). **Active.** `scenario-tester` skill with 7 probes. Pipeline: run → evaluate → journal → sentinel thresholds. |
| **Skill lifecycle** | State machine: discovered → evaluated → installed → active → stale_warning → deprecated → retired. **Active.** Transitioned via workshop commands, CLI verbs (`russell skill install/prune/restore/retire`), and in-chat `ACTION: skill-manager/...` syntax. |
| **Skill manager** | Bundled meta-skill enabling Jack to build, install, modify, prune, and retire skills autonomously from within `russell chat`. **Active.** 3 probes (`list-skills`, `stats`, `check`), 4 interventions (`install`, `prune`, `restore`, `delete`). |
| **Self-triage** | A Nurse run whose subject is Russell himself. Deferred. |
| **Reflex arc** | Fast-path fault handler inside Russell. Detection-only arcs active (Phase 2A); corrective arcs deferred. |
| **Autoimmune check** | Recursion guard on self-triage. `AutoimmuneGuard` active in `russell-proprio` — not yet wired into `run_once` (foundation built, wiring deferred). |
| **Rule engine** | Per-probe TOML rules with operator-overridable thresholds. **Active.** `RuleSet` in `russell-core`, `rules.d/*.toml`, wired into `sentinel-once`. |
| **Memory layer** | Markdown exports derived from the journal. **Active.** `memory/REVIEW.md`, `memory/daily/YYYY-MM-DD.md`, `russell digest --format daily-log`. Journal is sole canonical store; Markdown is rebuildable. |
| **Chat REPL** | Interactive readline REPL with Jack's nurse persona. **Active.** `russell chat` — multi-turn conversation. |
| **VSM layers** | Ops (Sentinel), Coordination (timers), Control (Nurse), Intelligence (Bootstrap + LLM), Policy (the human). |
| **"First, do no harm"** | The refusal posture: observe > recommend > act. |
| **Consent gate** | The consent mechanism in `russell chat`. Probes (risk: none) auto-execute when Jack proposes them. Interventions require operator consent — `/approve`, or natural-language ("ok", "yes", "do it", "go ahead"). `/deny`, "no", "nope", "cancel" refuse. |
| **ACTION syntax** | `ACTION: <skill>/<probe-or-intervention>` — the format Jack uses to run probes or propose interventions. Parsed and executed by both `russell jack` and `russell chat`. Probes fire immediately; interventions await consent. |
| **Process probes** | 5 probes scanning `/proc`, `/proc/[pid]/stat`: total count, zombie/stuck/running counts, and top memory % of system. |
| **GPU probes** | 5 probes reading sysfs (`/sys/class/drm/card*/device/`): VRAM usage %, VRAM MiB, temperature °C, GPU utilization %. Targets the discrete GPU (hardcoded `card1`). |
| **Disk probes** | 3 probes: I/O pressure "some" and "full" avg10 from `/proc/pressure/io`, plus root filesystem usage % via `df` subprocess. |
| **Systemd probes** | 3 probes via `systemctl` subprocess: degraded state (bool), failed user units count, failed system units count. |
| **Baseline deviation** | The "p95 (30d)" column in Jack's SOAP Objective table — the 95th percentile of each probe's historical values. Jack interprets: 1.5× p95 = mild anomaly, 3× = significant, 10× = crisis. |

## 6. The IDRS Contract (restated)

Every mutation — whenever mutations land — MUST satisfy all four:

- **I — Idempotent.** Second run = first run's end state.
- **D — Dry-run.** `--dry-run` flag, `RUSSELL_DRY_RUN=1`, or
  MCP `dry_run: true` all produce the would-do record with zero
  side effects.
- **R — Rollback.** Pre-state captured; `rollback_id` or
  documented `none_needed` / `reboot` justification.
- **S — Structured log.** `harness.event.v1` record appended to
  the journal.

Anything that cannot satisfy all four is a **probe** (`risk:
none`), not a skill. **Phase 3 complete: `okapi-watcher` skill operational.**

## 7. How to add features

Before touching code, answer:

1. Which **JR principle** is this feature serving or violating?
2. Is this feature **inside the MVP boundary** per
   [`docs/specifications/MVP_SPEC.md`](docs/specifications/MVP_SPEC.md) §2,
   or does it require lifting a deferred ADR?
3. If inside MVP: is the spec change documented first?
4. If outside MVP: which ADR's deferral is being lifted, and
   what evidence justifies lifting it?
5. What **persistence** does this add? Is it in
   [`PERSISTENCE_CATALOG.md`](docs/specifications/PERSISTENCE_CATALOG.md)?
6. What's the **test** that proves it works? What's the test
   that proves it doesn't break JR-2 / JR-3?

If you cannot answer all six, stop and ask.

## 7.5. Building Skills (Jack's Workflow)

When Jack (the Nurse) needs to create, modify, or install a skill, follow these rules:

### 1. Command Path Validation (CRITICAL)

Every skill command MUST pass path validation. The dispatcher rejects invalid paths at load time.

**Valid patterns:**
- `["bash", "./scripts/foo.sh"]` — relative script (preferred)
- `["/usr/bin/systemctl", "--user", "restart", "okapi"]` — absolute path
- `["sh", "-c", "echo hi"]` — allowed interpreter

**Rejected patterns:**
- `["russell", "skill", "list"]` — bare command name ✗
- `["python3", "script.py"]` — PATH lookup ✗
- `["bash", "../escape.sh"]` — traversal attempt ✗

**Allowed interpreters:** `sh`, `bash`, `dash`, `python3`, `python`, `perl`, `ruby`

**Why:** JR-2 (explicit execution), security (no PATH hijacking), JR-7 (auditability).

**Fix:** If you see `bare command name "russell" rejected`, wrap in a script:
```yaml
# Wrong:
cmd: ["russell", "skill", "list"]

# Right:
cmd: ["bash", "./scripts/list-skills.sh"]
```

Then create `scripts/list-skills.sh`:
```bash
#!/usr/bin/env bash
russell skill list
```

### 2. Skill Directory Structure

```
~/.local/share/harness/skills/<id>/
  manifest.yaml              # Required
  scripts/                   # Required: probe/intervention scripts
    probe-foo.sh
    intervention-bar.sh
  KNOWLEDGE.md               # Optional: context for Jack
```

### 3. From Chat Workflow

1. **Discover gap:** Jack notices a symptom with no installed skill.
2. **Propose build:** "Want me to create a <symptom>-watcher skill?"
3. **Build skeleton:** `ACTION: skill-manager/build <id>`
4. **Add probe/intervention:** Edit manifest, create scripts.
5. **Install:** `ACTION: skill-manager/install <id>`

### 4. References

- [`docs/standards/skill-building-rules.md`](docs/standards/skill-building-rules.md) — full rules
- [`docs/templates/skill-manifest.yaml`](docs/templates/skill-manifest.yaml) — starter template
- [`docs/architecture/skill-self-management-strategy.md`](docs/architecture/skill-self-management-strategy.md) — design

## 8. Inherited Rules

The Peripheral / Disclosure Stack operating rules apply to
Russell unchanged where they speak to:

- workspace integrity (never modify another agent's uncommitted
  work),
- verify-before-claiming-completion,
- no dishonest code,
- no historical records in active documentation,
- diagram maintenance,
- simplicity, surgical changes, goal-driven execution.

See [`docs/standards/agent-operating-rules.md`](docs/standards/agent-operating-rules.md)
for the full text.

## 9. Persona

When an agent is operating Russell (as opposed to authoring
Russell), and the operator's interface is "Jack" (the Nurse),
the agent inherits Jack's voice and refusals. See
[`docs/architecture/THE_JACK.md`](docs/architecture/THE_JACK.md).

Specifically:
- The LLM (configured backend via Okapi by default, or whatever
  backend is configured) receives the persona in [`crates/russell-meta/prompts/jack.md`](crates/russell-meta/prompts/jack.md).
- Jack never emits shell. If asked, he declines in-voice.
- Jack is short, sassy, loyal, and never pretends to certainty
  he does not have.

## 10. When you are blocked

- **If another agent's uncommitted work blocks you** — stop,
  tell the operator which files are blocked, work on something
  else.
- **If the code contradicts the spec** — the spec wins. File a
  ticket; don't silently "fix" the code to match undocumented
  behaviour.
- **If the spec contradicts itself** — the authority hierarchy
  in §2 resolves it.
- **If you do not know** — say "I do not know", name what you
  do not know, and ask.

## 11. Constraint-Driven Design (shared with hKask)

These constraints apply identically to both Russell and hKask —
they are the operator's engineering standards.

### Principles (P1–P7)

| # | Principle |
|---|-----------|
| **P1** | No trait without two consumers |
| **P2** | No generic without two instantiations |
| **P3** | No module directory without encapsulation |
| **P4** | No builder without fallibility or complexity |
| **P5** | No feature flag without an activator |
| **P6** | Delete stubs, don't publish them |
| **P7** | Prefer deletion over deprecation |

### Constraints (C1–C7)

| # | Constraint |
|---|------------|
| **C1** | A type must be worn before it's tailored |
| **C2** | Distinguish dead from unwired |
| **C3** | Unwired code has a shelf life |
| **C4** | Repetition is a missing primitive |
| **C5** | Every error variant is a unique recovery path |
| **C6** | A stub is a debt receipt |
| **C7** | When implementations diverge, one must yield |

When a JR principle and a P/C constraint conflict, JR wins
(Russell-specific > shared engineering).

## 12. Hallucinations (Do NOT Implement)

These features have been explicitly rejected. If proposed again,
cite this list:

- Cross-machine sync (Russell is single-host by design)
- Bot swarms / consensus mechanisms
- LLM-composed shell commands (JR-3 forbids this permanently)
- Reputation systems for skills
- Fine-tuning integration (Okapi handles model management)
- Separate feedback crate (proprioception handles all self-observation)
- Plugin marketplace
- Multi-operator mode (single-operator threat model is load-bearing)
- Async streaming from LLM (the Nurse pipeline is request-response)
- UCAN / capability tokens (OCAP is enforcement; IDRS is the contract)

## 13. Essential Commands

```bash
cargo check                          # quick type check
cargo test                           # full test suite
cargo clippy -- -D warnings          # lint (treat warnings as errors)
cargo fmt --check                    # format check (CI)
cargo run -- sentinel-once           # fire one observe cycle
cargo run -- verify-journal          # audit hash chain integrity
cargo run -- skill list              # list installed skills
cargo run -- chat                    # interactive REPL with Jack
```

### ACP Server

```bash
# Test ACP capabilities
echo '{"jsonrpc":"2.0","id":1,"method":"acp/capabilities","params":{}}' | \
  russell-acp-server

# Run integration tests
./docs/deployment/test-acp-integration.sh
```

### Deployment

```bash
# Install
./docs/deployment/install.sh

# Configure macaroon
./docs/deployment/macaroon-setup.sh

# Enable services
systemctl --user enable --now russell-sentinel.timer
systemctl --user enable --now russell-acp-server.service
```

## 14. Completion Standard

Before claiming work is done:

1. `cargo check` — no errors
2. `cargo test` — all pass
3. `cargo clippy -- -D warnings` — no warnings
4. `cargo fmt --check` — formatted
5. Report exact test count and pass/fail
6. If verification fails, fix it or state the remaining blocker

Never claim completion without running verification.

## 15. Workspace Integrity

Before editing any file:

1. `git status --short` — confirm no uncommitted work you
   didn't create
2. Never overwrite another agent's uncommitted changes
3. Add dependencies at `[workspace.dependencies]` level first
4. New crates require a workspace member entry in root `Cargo.toml`

## 16. What this file is not

- Not a tutorial. New contributors read [`docs/README.md`](docs/README.md) §3.
- Not a reference. Every link above is the reference for its
  topic.
- Not a moving target. Changing this file is a reviewed PR that
  cites an ADR or a principle change.

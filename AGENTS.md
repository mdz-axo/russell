---
title: "Russell — Agent Orientation"
audience: [agents, operators, developers, contributors, architects]
last_updated: 2026-05-09
togaf_phase: "Preliminary / Governance"
version: "1.0.0"
status: "Active"
---

<!-- TOGAF_DOMAIN: Governance -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-05-09 -->

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
- **reports** through a read-only CLI,
- **watches himself** (proprioception — "did I run on time?"),
- and when asked, **cries for help** via a local LLM
  (Ollama by default, DeepSeek V4 Pro; OpenRouter opt-in).

He does *not* mutate host state, dispatch skills, or act on LLM
output. Those lanes are deferred behind the MVP boundary.

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
| **Nurse** | The subsystem that consults the LLM when the operator runs `russell jack` or `russell chat`. Jack watches over the machine; he doesn't "diagnose" — he notices, checks in, and cares. |
| **Jack** | The persona: terrier + *Will & Grace* Jack McFarland + Rust/Linux/cybernetics fluency. He's a nurse, not a doctor — loyal, attentive, never pretends to hands he doesn't have. See [`docs/architecture/THE_JACK.md`](docs/architecture/THE_JACK.md). |
| **Skill module** | YAML manifest + scripts encoding one playbook. **Active.** `russell-skills` crate with manifest parser, dispatcher, CLI verbs (`russell skill list`, `russell skill run`). |
| **IDRS** | Idempotent / Dry-run / Rollback / Structured-log contract for every mutation. |
| **SOAP bundle** | Evidence folder laid out Subjective / Objective / Assessment / Plan. |
| **Honeymoon window** | First 30 days after bootstrap; elevated caution. Deferred mechanism in MVP. |
| **Risk band** | `none` / `low` / `medium` / `high` / `critical`. Enforced by dispatcher's `max_auto_risk` cap. |
| **EWMA baseline** | Per-probe mean + variance, 30-day rolling p50/p95/p99. **Active.** `compute_baselines()` + daily refresh. |
| **Chaos probe** | Deliberate bounded failure to verify recovery. Deferred. |
| **Poka-yoke** | The dispatcher refusing any ID not in the loaded manifest. **Active.** Validated at dispatch time. |
| **Andon cord** | Operator's stop-the-line signal. Deferred. |
| **Proprioception** | Russell's self-observation. **Active.** 5 self-vitals: `sentinel_last_run_age_s`, `journal_writer_stall_s`, `llm_p95_latency_ms`, `timer_drift_s`, `help_error_rate_pct`. |
| **Meta-Sentinel** | The self-facing Sentinel. Full form deferred; 5-vital form active. |
| **Self-triage** | A Nurse run whose subject is Russell himself. Deferred. |
| **Reflex arc** | Fast-path fault handler inside Russell. Detection-only arcs active (Phase 2A); corrective arcs deferred. |
| **Autoimmune check** | Recursion guard on self-triage. `AutoimmuneGuard` active in `russell-proprio`. |
| **VSM layers** | Ops (Sentinel), Coordination (timers), Control (Nurse), Intelligence (Bootstrap + LLM), Policy (the human). |
| **"First, do no harm"** | The refusal posture: observe > recommend > act. |

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
none`), not a skill. **MVP Russell has no skills.**

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
- The LLM (DeepSeek V4 Pro via Ollama by default, or whatever
  backend is configured) receives the persona in [`crates/russell-doctor/prompts/jack.md`](crates/russell-doctor/prompts/jack.md).
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

## 11. What this file is not

- Not a tutorial. New contributors read [`docs/README.md`](docs/README.md) §3.
- Not a reference. Every link above is the reference for its
  topic.
- Not a moving target. Changing this file is a reviewed PR that
  cites an ADR or a principle change.

<!--
audience: every contributor to Russell, human and AI
last-reviewed: 2026-04-17
-->

# AGENTS.md — contributing to Russell

> Russell is a cybernetic health harness for a single Linux AI/ML workstation,
> exposed to local agent harnesses as a Model Context Protocol (MCP) server.
> This file is the primary orientation document for anyone — human, LLM, or
> subagent — who intends to change the code, the docs, or the skill catalog.

## 1. What Russell is (one paragraph)

Russell observes a Framework 16 / Ryzen AI / Radeon / Ubuntu workstation the
way a primary-care physician watches a patient: cadenced hygiene, continuous
vitals, a disciplined escalation to a specialist when something looks wrong,
and an evidence bundle that anyone can read after the fact. The whole
apparatus is packaged as a single Rust binary that speaks MCP over stdio so
that agent frontends — Claude Desktop, Roo/Cline in VSCodium, Zed — can call
the Sentinel, the Doctor, the journal, and the skill dispatcher as tools.
The LLM is a consultant that ranks a differential over known probe IDs; it
never emits shell.

## 2. Reading order

Do not skip. Every section below references vocabulary defined here.

1. [`README.md`](README.md) — what this folder is.
2. [`MACHINE_PROFILE.md`](MACHINE_PROFILE.md) — the patient's chart.
3. [`cybernetic-health-harness.md`](cybernetic-health-harness.md) — the
   canonical design document. **Treat its vocabulary as binding.**
4. [`docs/architecture/overview.md`](docs/architecture/overview.md) — how
   the locked decisions fit together.
5. [`docs/architecture/mcp-surface.md`](docs/architecture/mcp-surface.md) —
   the MCP tool surface you are extending.
6. [`docs/architecture/proprioception.md`](docs/architecture/proprioception.md)
   — Russell's reflexive nervous system.
7. [`docs/adr/`](docs/adr/) — every locked decision, numbered.
8. The standards (§5 below).

## 3. The medical metaphor is load-bearing

The metaphor is not decoration; it shapes module boundaries, escalation,
log schema, and the default refusal posture. Preserve this vocabulary
verbatim across code, docs, commit messages, and agent prompts:

| Term | Meaning in Russell |
|---|---|
| **Sentinel** | Continuous low-cost telemetry collector; writes `samples` rows. |
| **Doctor** | Supervisor that triages symptoms, assembles a SOAP bundle, consults the LLM, and dispatches skill interventions. |
| **Skill module** | YAML manifest + referenced scripts encoding one diagnostic playbook. Data, not code. |
| **IDRS** | Idempotent / Dry-run / Rollback / Structured-log — the four-property contract every mutating action must satisfy. |
| **SOAP bundle** | Evidence folder laid out as Subjective / Objective / Assessment / Plan. |
| **Risk band** | `none` / `low` / `medium` / `high` / `critical`. `max_auto_risk` caps what the Doctor may run unattended. |
| **Tiered cadences** | Tier I daily, Tier II weekly/monthly, Tier III quarterly, Tier IV urgent / on-symptom. |
| **Honeymoon window** | First 30 days after bootstrap; any `risk>=high` defaults to *propose*, not *apply*. |
| **EWMA baseline** | Per-probe exponentially-weighted mean + variance plus rolling p50/p95/p99. |
| **Chaos-probe** | A scheduled, bounded, deliberate failure used to verify recovery. |
| **Poka-yoke** | Manifest schema validation; dispatcher refuses IDs not in the loaded manifest. |
| **Andon cord** | `russell confirm` / the `confirm_proposal` MCP tool — a human stops the line for any `risk>=medium` auto-disabled action. |
| **VSM layers** | Operations (Sentinel, Tiers), Coordination (timers), Control (Doctor), Intelligence (Bootstrap + LLM), Policy (the human operator). |
| **"First, do no harm"** | Default posture: **observe > recommend > act**. |

These terms are new as of ADR-0015 and belong to Russell's **reflexive
nervous system**:

| Term | Meaning |
|---|---|
| **Proprioception** | Russell's awareness of its own internal state — timer drift, dispatch latency, journal health, MCP error rate, subprocess zombies, LLM call latency, unit states of its own services. |
| **Meta-sentinel** | The internal Sentinel that samples Russell's own vitals on the same cadenced basis as the host Sentinel. |
| **Self-triage** | A Doctor run whose subject is Russell itself — a stuck skill, a wedged journal, a flapping timer. Journaled with `scope=self`. |
| **Reflex arc** | The fast path for self-faults that cannot wait for the next cadence (e.g., a watchdog on a hung skill subprocess). |
| **Autoimmune check** | The recursion guard that prevents self-triage from invoking itself in a loop. |

## 4. The IDRS contract (restated crisply)

Every mutating action — whether invoked via CLI, timer, or the `skill_run`
MCP tool — MUST satisfy all four:

- **I — Idempotent.** Running the action twice converges to the same end
  state. Verifiable with `russell run --module X --verify-idempotent`.
- **D — Dry-Run.** A `--dry-run` flag (or `RUSSELL_DRY_RUN=1`, or
  `dry_run: true` in the MCP call) emits the would-do log but performs
  zero mutations. The dispatcher enforces this at the boundary.
- **R — Rollback.** Every mutating step captures pre-state before it
  runs. Manifest fields: `rollback_id` (points at the reverse
  intervention) or `rollback: reboot` / `none_needed` with written
  justification. Config edits keep `.bak` copies; systemd drop-ins get a
  templated revert unit.
- **S — Structured log.** Every action emits a JSON event that conforms
  to `harness.event.v1` and is appended to the journal; human-readable
  renders derive from that record.

Anything that cannot satisfy all four is not a skill — it is a probe, and
must declare `risk: none`.

## 5. Standards

| Document | Covers |
|---|---|
| [`docs/standards/coding-rust.md`](docs/standards/coding-rust.md) | Rust coding conventions, lint posture, error handling, `unsafe` discipline, module layout. |
| [`docs/standards/documentation.md`](docs/standards/documentation.md) | Doc tiers, voice, Markdown headers, Mermaid discipline, cross-linking. |
| [`docs/standards/adr.md`](docs/standards/adr.md) | How to author and file an ADR. |
| [`docs/standards/commits.md`](docs/standards/commits.md) | Conventional Commits, Russell-specific types and scopes. |
| [`docs/standards/safety.md`](docs/standards/safety.md) | IDRS, risk bands, honeymoon, andon cord, kill switches, confirm flow. **Read this before proposing any mutating action.** |

Mechanics (toolchains, `cargo` aliases, snapshot review, local MCP wiring)
live in [`CONTRIBUTING.md`](CONTRIBUTING.md).

## 6. How to add a new skill

1. Pick an ID. Skill IDs are `kebab-case`: `gpu-doctor`, `battery-doctor`,
   `toolchain-gardener`. Single-token IDs are reserved for core.
2. Copy [`docs/templates/skill-manifest.yaml`](docs/templates/skill-manifest.yaml)
   into `skills/<id>/manifest.yaml`. Fill every field. Probes are
   `risk: none` by definition. Interventions must name a `rollback_id`
   (or declare `rollback: none_needed` / `rollback: reboot` with a
   code-comment justification).
3. Put referenced scripts under `skills/<id>/scripts/`. Bash is fine;
   Python is fine; a binary is fine. The Rust dispatcher invokes them as
   subprocesses and enforces the IDRS contract at the boundary.
4. Register the symptom class if new: add an entry to the symptom catalog
   (see [`docs/adr/0007-yaml-manifest-subprocess-skill-model.md`](docs/adr/0007-yaml-manifest-subprocess-skill-model.md)).
5. Write at least one integration test that exercises `dry_run: true`
   through the `skill_dry_run` MCP tool and snapshots the SOAP bundle
   with `insta`.
6. File an ADR if the skill introduces a new risk-band convention or a
   new probe category.

Do **not** add a skill whose only intervention is `risk >= high` without
an accompanying ADR that justifies why a safer step does not exist.

## 7. How to add a new MCP tool

1. Propose the tool in an ADR (short one — scope, name, input/output
   schema, risk band, confirmation requirement).
2. Add the wire schema to the `russell-mcp` crate. Every tool has a
   `risk_band` field in its descriptor — even read-only tools; they
   declare `none`.
3. Any tool whose risk band is `medium` or above MUST require the caller
   to first obtain a proposal ID via a plan-style tool and then pass it
   to `confirm_proposal` to enact. There is no "one-shot mutating tool"
   in Russell.
4. Register the tool in [`docs/architecture/mcp-surface.md`](docs/architecture/mcp-surface.md).
5. Snapshot-test the tool's output with `insta`.

## 8. How to add an ADR

Follow [`docs/standards/adr.md`](docs/standards/adr.md). Use
[`docs/templates/adr-template.md`](docs/templates/adr-template.md). Number
monotonically and zero-pad to four digits. Never renumber a merged ADR;
supersede it instead.

## 9. Proprioception is a first-class requirement

Russell watches itself the same way it watches the host. When you add a
new loop — a timer, a worker task, a subprocess pool, a retry wrapper,
an LLM call — ask:

- What vital tells me this loop is healthy?
- What EWMA baseline does it feed?
- What reflex arc fires if it wedges before the next cadence?
- Under what condition would a self-triage try to invoke itself and how
  is that prevented?

See [`docs/adr/0015-proprioception-self-health.md`](docs/adr/0015-proprioception-self-health.md)
and [`docs/architecture/proprioception.md`](docs/architecture/proprioception.md).

## 10. The "LLM never emits shell" rule

The LLM is called by the Doctor (and by the meta-Doctor for self-triage)
to rank a differential over the probe and intervention IDs present in
the loaded manifests. It returns IDs and justifications; the dispatcher
translates IDs to commands. If the LLM hallucinates an ID, the
dispatcher rejects the plan. Network egress for LLM calls is opt-in; the
default backend is local Ollama. Every LLM request and response is
logged verbatim into the evidence bundle. See
[`docs/adr/0008-llm-triage-never-emits-shell.md`](docs/adr/0008-llm-triage-never-emits-shell.md).

This rule is not negotiable. An AI contributor who rewrites the prompt
pipeline to let the LLM output shell is introducing a safety regression.

## 11. Safety posture

**observe > recommend > act.** For the first 30 days after bootstrap
(the honeymoon window), any `risk >= high` intervention defaults to
*propose* rather than *apply*. `max_auto_risk` is `low` by default and
per-skill. Two kill switches exist:

- `~/.config/harness/disable` — an empty file makes every timer a no-op
  on next trigger.
- `russell pause <module> --until <rfc3339>` — per-module cooldown.

The `confirm_proposal` MCP tool is the programmatic andon cord; the
`russell confirm <evidence_id>` CLI is the human-facing one. See
[`docs/standards/safety.md`](docs/standards/safety.md).

## 12. Testing expectations

- `cargo test` green on every PR. CI runs `cargo clippy -- -D warnings`
  and `cargo fmt --check`.
- Unit tests live alongside the code.
- Integration tests live under each crate's `tests/`.
- MCP tool outputs and SOAP bundle rendering are covered with
  [`insta`](https://insta.rs/) snapshot tests. Review new snapshots with
  `cargo insta review`; do not blind-accept them.
- Schema and parser invariants (manifest, rules, MCP wire format) are
  covered with [`proptest`](https://proptest-rs.github.io/proptest/).
- VM-based end-to-end runs are Phase-3-plus territory; do not gate PRs
  on them.

## 13. Commit conventions

Conventional Commits, with the Russell-specific types and scopes defined
in [`docs/standards/commits.md`](docs/standards/commits.md). Short form:

```
<type>(<scope>): <imperative subject, no period>

<optional body>

<optional footer; BREAKING CHANGE: ... ; Refs: ADR-0003>
```

Types: `feat`, `fix`, `docs`, `refactor`, `test`, `chore`, `adr`,
`skill`, `proprio`.
Scopes: `core`, `mcp`, `skills`, `doctor`, `sentinel`, `proprio`,
`journal`, `profile`, `cli`.

## 14. Branch naming

- `feat/<scope>-<slug>` for features.
- `fix/<scope>-<slug>` for bug fixes.
- `adr/NNNN-<slug>` for ADR-only branches.
- `skill/<skill-id>` for skill additions.
- `proprio/<slug>` for self-health work.

No long-lived feature branches; rebase on `main`.

## 15. When in doubt

Read the section of [`cybernetic-health-harness.md`](cybernetic-health-harness.md)
that touches your change. Cite it in the commit body or PR description.
The design document is the arbiter of taste disputes that the ADRs do
not settle.

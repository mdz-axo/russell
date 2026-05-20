---
title: "Architecture Overview"
audience: [architects, developers, contributors]
last_updated: 2026-05-06
togaf_phase: "A"
version: "1.1.0"
status: "Active"
---

<!-- TOGAF_DOMAIN: Architecture Vision -->
<!-- VERSION: 1.1.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-05-06 -->

<!--
audience: contributors orienting themselves before editing code
last-reviewed: 2026-04-17
-->

# Architecture overview

This document summarises **how the locked decisions fit together**
right now. It does not re-derive them; for that, read the relevant
ADRs. It also does not replace the canonical design document
([`cybernetic-health-harness.md`](../../cybernetic-health-harness.md));
when the two disagree, the ADR wins. When an ADR is silent, the
design document wins. When both are silent, file an ADR.

## 1. What Russell is, in one diagram

```mermaid
flowchart TB
  subgraph POLICY [Policy — the human operator]
    USER[Operator]
    CONFIRM[russell confirm / confirm_proposal]
  end

  subgraph INTEL [Intelligence]
    BOOT[Bootstrap]
    LLM[Local LLM via Okapi]
  end

  subgraph CONTROL [Control]
    META[Metacognitive layer]
    PROPRIO[Meta-Doctor / self-triage]
  end

  subgraph COORD [Coordination]
    CLOCK[systemd --user timers]
  end

  subgraph OPS [Operations]
    SENTINEL[Sentinel]
    META[Meta-Sentinel]
    TIERS[Tier I / II / III modules]
    SKILLS[Skill dispatcher]
  end

  subgraph PERSIST [Persistence]
    JOURNAL[(SQLite journal.db)]
    PROFILE[profile.json]
    EVIDENCE[Evidence bundles]
    RULES[rules.d/*.toml]
  end

  subgraph SURFACE [Surfaces]
    CLI[russell CLI]
    MCP[MCP stdio server]
    NOTIFY[notify-send]
    DIGEST[HTML digest]
  end

  USER --> CLI & CONFIRM
  MCP <--> AGENTS((Agent frontends))
  CLI & MCP --> META & TIERS & SKILLS
  CLOCK --> TIERS & SENTINEL & META
  SENTINEL --> JOURNAL
  META --> JOURNAL
  TIERS --> META
  META --> SKILLS --> JOURNAL & EVIDENCE
  META --> LLM
  PROPRIO --> META
  BOOT --> PROFILE
  PROFILE --> META & TIERS & SENTINEL
  RULES --> SENTINEL
  JOURNAL --> DIGEST
  META --> NOTIFY
```

<!-- DIAGRAM_ALIGNMENT
id: DIAG-OVERVIEW-001
type: flowchart
verified_date: 2026-05-13
verified_against: AGENTS.md §5 (VSM layers); ADR-0004, ADR-0006, ADR-0008, ADR-0013, ADR-0015
reference_sources: PRINCIPLES_CATALOG.md JR-1 through JR-7; cybernetic-health-harness.md
status: VERIFIED
-->

The five VSM layers — Policy, Intelligence, Control, Coordination,
Operations — are not just diagramming convenience. Each layer has
a corresponding ADR and a corresponding area of the code:

| VSM layer | Locked decision | Code home |
|---|---|---|
| Policy | [ADR-0005](../adr/deferred/0005-privileged-operations.md), [safety.md](../standards/safety.md) | `russell-cli` confirm flow, kill switches |
| Intelligence | [ADR-0008](../adr/0008-llm-triage-never-emits-shell.md) | `russell-meta::openrouter`, `russell-core::profile` |
| Control | [ADR-0007](../adr/deferred/0007-yaml-manifest-subprocess-skill-model.md), [ADR-0015](../adr/0015-proprioception-self-health.md) | `russell-meta`, `russell-proprio` (MVP self-vital) |
| Coordination | [ADR-0009](../adr/deferred/0009-tokio-runtime.md) + systemd timers | Unit files under `packaging/systemd/`; timers are OS-level |
| Operations | [ADR-0004](../adr/0004-sqlite-journal.md), [ADR-0006](../adr/0006-profile-abstraction.md) | `russell-sentinel`, `russell-skills` |

## 2. Crate topology

```mermaid
flowchart LR
  CORE[russell-core]
  SENTINEL[russell-sentinel]
  META[russell-meta]
  SKILLS[russell-skills]
  MCP[russell-mcp]
  PROPRIO[russell-proprio]
  CLI[russell-cli]

  SENTINEL --> CORE
  SKILLS --> CORE
  META --> CORE & SKILLS
  PROPRIO --> CORE
  MCP --> CORE & SENTINEL & META & SKILLS & PROPRIO
  CLI --> CORE & SENTINEL & META & SKILLS & PROPRIO & MCP
```

<!-- DIAGRAM_ALIGNMENT
id: DIAG-OVERVIEW-002
type: flowchart
verified_date: 2026-05-13
verified_against: ADR-0013 (rust-workspace-layout); cargo.toml dependency declarations
reference_sources: PRINCIPLES_CATALOG.md JR-6 (reuse over dependency)
status: VERIFIED
-->

See [ADR-0013](../adr/0013-rust-workspace-layout.md). No crate
depends on `russell-cli`. The dependency DAG is rooted at
`russell-core`.

## 3. Data plane

Three on-disk artifacts are canonical; everything else is
derived.

### 3.1 `profile.json`

- Path: `~/.local/state/harness/profile.json`.
- Author: the Bootstrap.
- Readers: every tier, the Doctor, the Sentinel, the MCP server.
- Schema: [ADR-0006](../adr/0006-profile-abstraction.md).
- Invariant: mutations happen only through the Bootstrap state
  machine. Everywhere else the profile is read-only.

### 3.2 `journal.db`

- Path: `~/.local/state/harness/journal.db`.
- Engine: SQLite with WAL + `synchronous=NORMAL`
  ([ADR-0004](../adr/0004-sqlite-journal.md)).
- Tables: `samples`, `events`, `baselines`, `confirmations`,
  `migrations`. Self-scope data (proprioception) uses
  `scope='self'` in the same `samples` and `events` tables
  ([ADR-0015](../adr/0015-proprioception-self-health.md)).
- Writer: serialized through a single `spawn_blocking` task in
  `russell-core::journal::writer`. Readers use a connection
  pool.
- Migrations are forward-only, zero-padded, never edited after
  merge.

### 3.3 Evidence bundles

- Path: `~/.local/state/harness/evidence/<evidence_id>/`.
- Contents: `soap.md`, `skill.yaml`, per-probe JSON, per-
  intervention JSON, `llm-transcript.jsonl`,
  system-snapshot files (`dmesg.log`, `rocm-smi.json`, etc.).
- Referenced from journal events via `evidence_ref`.
- Expire 90 days after their final state transition, except
  bundles marked `archive`.

## 4. Control plane

### 4.1 Timers (systemd --user)

Russell runs under user-scoped systemd with one exception:
`weekly/apt-upgrade` escalates through a narrow PolKit action
([ADR-0005](../adr/deferred/0005-privileged-operations.md)). All timers
declare `Persistent=true` + `RandomizedDelaySec=` so a sleeping
laptop catches up without thundering herd.

### 4.2 CLI and MCP, two views of the same actions

Every CLI subcommand has a matching MCP tool unless an ADR
explicitly justifies the asymmetry. This is a hard rule so
agents and humans never diverge in capability. The MCP surface
is catalogued in
[`../archive/mcp-surface.md`](../archive/mcp-surface.md).

### 4.3 The Doctor

The Doctor is a **supervisor**, not an LLM wrapper. Its loop:

1. Receive a symptom (Sentinel crit event, CLI, tier
   escalation, proprioception).
2. Load matching skill manifests.
3. Run all `risk: none` probes.
4. Compose the Objective section of the SOAP bundle.
5. Ask the LLM to rank a differential **over the manifest's
   probe and intervention IDs**. Never over freeform text.
6. Execute interventions whose risk is at or below the
   effective cap (per-skill, honeymoon, global), skipping
   those with `requires_confirmation`.
7. Run evaluation steps; if they fail, execute rollbacks.
8. Write the SOAP bundle; emit journal events; notify.

The LLM never emits shell
([ADR-0008](../adr/0008-llm-triage-never-emits-shell.md)).

## 5. Observation plane

The Sentinel fires every 5 minutes via `russell-sentinel.timer`.
It consults `rules.d/*.toml` to compute severity from each
probe's value against the EWMA baseline. Samples and any
generated events land in the journal.

The Meta-Sentinel (`russell-proprio`) observes Russell himself.
Five self-vitals are active (per JR-5):
`sentinel_last_run_age_s`, `journal_writer_stall_s`,
`llm_p95_latency_ms`, `timer_drift_s`, `help_error_rate_pct`.
It runs BEFORE host probes in each cycle so the measurement is
never stale-by-one. Full meta-Sentinel (additional vitals) is
deferred to Phase 4. See
[`../archive/proprioception.md`](../archive/proprioception.md)
for the aspirational design.

## 6. Honeymoon and first 30 days

Russell is deliberately cautious for the first 30 days after
bootstrap. Effective `max_auto_risk` is clamped to `low` for any
skill with `risk: high` interventions, regardless of manifest
settings. Rationale: baselines need data to be meaningful;
without them the Doctor lacks the evidence to justify an
aggressive intervention.

## 7. Where to put a new feature

| Kind of change | Code home | Docs to update |
|---|---|---|
| New probe | `russell-sentinel::probes` | `overview.md` §3.2 if schema changes; ADR if new hardware class |
| New skill | `skills/<id>/` | `AGENTS.md` §6; `skill-self-management-strategy.md` for meta-skills |
| New MCP tool | `russell-mcp::tools` | [`../archive/mcp-surface.md`](../archive/mcp-surface.md); ADR |
| New CLI subcommand | `russell-cli::commands` | `CONTRIBUTING.md` §9; mirror in MCP if user-facing |
| New self-health vital | `russell-proprio::probes` | [`../archive/proprioception.md`](../archive/proprioception.md); ADR if new failure class |

## 8. hKask integration surface

### hKask → Russell (read path)

Russell's journal (`~/.local/state/harness/journal.db`) is read by
`arsenal-mcp-russell` — an MCP tool server that lives in the hKask
repo (`~/Clones/hkask`). It exposes 7 tools:

| MCP tool | Purpose |
|---|---|
| `russell_host_snapshot` | Latest sample from each host probe |
| `russell_self_vital` | Proprioception status |
| `russell_journal_query` | Arbitrary time-range query over samples |
| `russell_help_sessions` | LLM consultation history |
| `russell_curator_assess` | Duncan's structured health assessment |
| `russell_cadence_health` | Observation cadence gap analysis |
| `russell_token_status` | hKask MCP token status |

**Duncan** is an infrastructure Curator in hKask's
`stack-control-plane` that calls `russell_curator_assess` to
produce health reports with findings and recommendations.

### Russell → hKask (MCP client path)

Per ADR-0025, Russell's `russell-mcp` crate is a fully operational
MCP client that calls into hKask's `stack-api` gateway
(`http://127.0.0.1:8080`). Russell has access to 193 tools across
16 MCP servers registered in `~/.config/hkask/mcp-registry.json`:

| Server | Tools | Capability |
|---|---|---|
| `web` | 5 | Brave search, Firecrawl, Browserbase, Exa |
| `scholar` | 12 | Semantic Scholar papers/citations |
| `rss-reader` | 19 | RSS/Atom feed subscriptions |
| `fmp` | 18 | Financial data/stock analysis |
| `fal` | 24 | Image/video/3D generation |
| `telnyx` | 24 | SMS/MMS/voice/WhatsApp |
| `mxroute` | 9 | Email send/read/manage |
| `gallery` | 10 | Vision-powered image gallery |
| `embedding` | 8 | Vector search (Qdrant) |
| `doc-knowledge` | 7 | Document QA/knowledge graphs |
| `evolution` | 3 | Okapi evolution management |
| `spandrel` | 18 | Capability ontology/graph |
| `axolotl` | 12 | Model fine-tuning/training |
| `maintenance` | 9 | Curator health/backup/cleanup |
| `okapi-metrics` | 8 | LLM engine metrics/adapters |
| `russell` | 7 | Host health/journal/proprioception |

**Integration boundary:** no cross-crate dependency. Russell does
not import hKask; hKask does not import Russell. Communication in
both directions is via HTTP REST to `stack-api` (Russell → hKask)
and via the SQLite journal read by `arsenal-mcp-russell`
(hKask → Russell).

See [`../proposals/russell-hkask-integration.md`](../proposals/russell-hkask-integration.md)
for the full design.

## 9. What this document is not

- Not a spec. The spec is the ADRs plus the design document.
- Not an API reference. That lives in rustdoc.
- Not a roadmap. That lives in
  [`cybernetic-health-harness.md`](../../cybernetic-health-harness.md)
  §20.

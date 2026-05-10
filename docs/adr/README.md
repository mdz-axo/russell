---
title: "Architecture Decision Records (Active)"
audience: [developers, architects, contributors]
last_updated: 2026-05-09
togaf_phase: "H — Change Management"
version: "1.0.0"
status: "Active"
---

# Active ADRs

<!-- TOGAF_DOMAIN: Change Management -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Active -->

The 14 active ADRs. ADRs 0001–0018 are load-bearing for MVP.
ADR-0019 opens Phase 2. ADRs 0020–0022 extend Phase 2.
ADR-0023 lifts ADR-0007 and opens Phase 3.
Deferred ADRs live under
[`deferred/`](deferred/) with their own README.

| # | Subject |
|---|---|
| [0001](0001-scope-and-charter.md) | Scope and charter |
| [0002](0002-licensing.md) | Dual MIT / Apache-2.0 |
| [0004](0004-sqlite-journal.md) | SQLite journal + WAL |
| [0006](0006-profile-abstraction.md) | `profile.json` single source of truth |
| [0008](0008-llm-triage-never-emits-shell.md) | The LLM never emits shell |
| [0011](0011-testing-strategy.md) | Testing layers |
| [0013](0013-rust-workspace-layout.md) | Seven-crate workspace, DAG at `russell-core` |
| [0015](0015-proprioception-self-health.md) | Proprioception — Jack watches Jack |
| [0016](0016-doctor-and-llm-router.md) | MVP Doctor — local-first Ollama, OpenRouter opt-in |
| [0017](0017-reuse-over-dependency.md) | Reuse over dependency — copy-with-provenance |
| [0018](0018-close-phase-1c.md) | Close Phase 1c — 20-day soak sufficient |
| [0019](0019-probe-cadence-and-ctha.md) | Probe cadence separation + CTHA instrumentation |
| [0020](0020-threshold-gated-llm-escalation.md) | Threshold-gated LLM escalation |
| [0021](0021-proprioception-phase2-reflex-arcs.md) | Proprioception Phase 2 — reflex arcs and self-vitals |
| [0022](0022-markdown-memory-layer.md) | Markdown memory layer — derived exports for human legibility |
| [0023](0023-lift-adr-0007-phase3-skills.md) | Lift ADR-0007 deferral — Phase 3 Skills and Dispatch |

To author a new ADR, see
[`../standards/adr.md`](../standards/adr.md) and
[`../templates/adr-template.md`](../templates/adr-template.md).

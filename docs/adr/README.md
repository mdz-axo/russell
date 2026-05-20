---
title: "Architecture Decision Records (Active)"
audience: [developers, architects, contributors]
last_updated: 2026-05-14
togaf_phase: "H"
version: "1.1.0"
status: "Active"
---

# Active ADRs

<!-- TOGAF_DOMAIN: Change Management -->
<!-- VERSION: 1.1.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-05-14 -->

The 18 active ADRs. ADRs 0001–0018 are load-bearing for MVP.
ADR-0019 opens Phase 2. ADRs 0020–0022 extend Phase 2.
ADR-0023 lifts ADR-0007 and opens Phase 3. ADR-0024 defines
the skill lifecycle. ADR-0025 establishes the hKask MCP client
(Phase 4).
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
| [0019](0019-probe-cadence-and-okh.md) | Probe cadence separation + OKH instrumentation |
| [0020](0020-threshold-gated-llm-escalation.md) | Threshold-gated LLM escalation |
| [0021](0021-proprioception-phase2-reflex-arcs.md) | Proprioception Phase 2 — reflex arcs and self-vitals |
| [0022](0022-markdown-memory-layer.md) | Markdown memory layer — derived exports for human legibility |
| [0023](0023-lift-adr-0007-phase3-skills.md) | Lift ADR-0007 deferral — Phase 3 Skills and Dispatch |
| [0024](0024-skill-registry-workshop-lifecycle.md) | Skill registry, workshop, and lifecycle — discovery-to-retirement pipeline |
| [0025](0025-hkask-mcp-client-trusted-relationship.md) | hKask MCP Client — Trusted Local Relationship (Phase 4, partially lifts ADR-0003) |

To author a new ADR, see
[`../standards/adr.md`](../standards/adr.md) and
[`../templates/adr-template.md`](../templates/adr-template.md).

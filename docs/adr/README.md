---
title: "Architecture Decision Records (Active)"
audience: [developers, architects, contributors]
last_updated: 2026-04-18
togaf_phase: "H — Change Management"
version: "1.0.0"
status: "Active"
---

# Active ADRs

<!-- TOGAF_DOMAIN: Change Management -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Active -->

The 10 ADRs that are load-bearing for MVP. Deferred ADRs live
under [`deferred/`](deferred/) with their own README.

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
| [0016](0016-doctor-and-llm-router.md) | MVP Doctor — single round-trip to ZDR frontier LLM |
| [0017](0017-reuse-over-dependency.md) | Reuse over dependency — copy-with-provenance |

To author a new ADR, see
[`../standards/adr.md`](../standards/adr.md) and
[`../templates/adr-template.md`](../templates/adr-template.md).

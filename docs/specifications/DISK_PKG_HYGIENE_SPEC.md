---
title: "Disk & Package Hygiene — Specification Suite"
audience: [developers, architects, operators, agents]
last_updated: 2026-05-06
togaf_phase: "B"
version: "0.1.0"
status: "Draft"
phase: "Phase 2 (post-MVP)"
---

<!-- TOGAF_DOMAIN: Business -->
<!-- VERSION: 0.1.0 -->
<!-- STATUS: Draft -->
<!-- LAST_UPDATED: 2026-05-06 -->


# Disk & Package Hygiene — Specification Suite

## Scope

This specification suite designs Russell's capability to **observe**
disk and package ecosystem entropy on the host machine. It is a
Phase 2 feature set — outside the MVP boundary defined in
[`MVP_SPEC.md`](MVP_SPEC.md) §5.

Russell observes. Jack recommends. The operator acts.

## Governing Principles

- **JR-1** — Austere by default. Minimal entity set; no probe exists that Russell cannot observe.
- **JR-2** — Observe only. No mutations. No installs. No removals.
- **JR-3** — The LLM never emits shell. Jack recommends in prose.
- **JR-7** — Persistence is auditable. New artifacts registered in `PERSISTENCE_CATALOG.md`.

## Tool/Connector Discipline

Every component decomposes into:

- **Tool** (adapter) — transforms data. Pure. No side effects. No I/O.
- **Connector** (port) — transfers formed data to/from a boundary. Side effect. No transformation logic.

In this domain:
- **Tools:** Parse statvfs into percentages, parse apt stdout into counts, compose SOAP bundles, format digest sections.
- **Connectors:** statvfs syscall, subprocess invocations, journal SQLite writes, OpenRouter HTTP POST (the LLM call that shares machine status with Jack/Kask for assessment).

## Document Map

| Task | Document | Summary |
|---|---|---|
| 0 | [`disk-pkg-hygiene/00-semantic-decomposition.md`](disk-pkg-hygiene/00-semantic-decomposition.md) | Six root causes of host entropy; observable signals, thresholds, surfaces |
| 1 | [`disk-pkg-hygiene/01-domain-model.md`](disk-pkg-hygiene/01-domain-model.md) | RDF graph, entity classes, predicates, Mermaid ER diagram |
| 2 | [`disk-pkg-hygiene/02-disk-probes.md`](disk-pkg-hygiene/02-disk-probes.md) | `disk_hygiene` probe family (statvfs, cache sizes, tmp age) — 5-min cadence |
| 3 | [`disk-pkg-hygiene/03-package-probes.md`](disk-pkg-hygiene/03-package-probes.md) | `pkg_ecosystem` probe family (apt, pip, npm, brew, snap, flatpak, cargo) — hourly cadence |
| 4 | [`disk-pkg-hygiene/04-provenance-registry.md`](disk-pkg-hygiene/04-provenance-registry.md) | Tracking curl-installed binaries via operator-maintained TOML registry |
| 5 | [`disk-pkg-hygiene/05-integration.md`](disk-pkg-hygiene/05-integration.md) | Wiring into hexagonal architecture; sequence diagrams; rules integration |
| 6 | [`disk-pkg-hygiene/06-open-questions.md`](disk-pkg-hygiene/06-open-questions.md) | Deferred decisions; ADR requirements; cadence, thresholds, ownership |
| 7 | [`disk-pkg-hygiene/07-dependency-drag-policy.md`](disk-pkg-hygiene/07-dependency-drag-policy.md) | **Policy:** Detecting and managing packages that pin stale dependencies |
| — | [`disk-pkg-hygiene/audit-russell-sentinel.md`](disk-pkg-hygiene/audit-russell-sentinel.md) | Three-layer audit of sentinel crate (completed) |

## Prerequisites

Before implementation begins:

1. **MVP must be closed** (20-day soak, per `MVP_SPEC.md` §6).
2. ~~**ADR for cadence separation** (Task 6, Question 1) must be decided.~~ **Done** — [ADR-0019](../adr/0019-probe-cadence-and-okh.md).
3. **`nix` crate** must be added to `russell-sentinel/Cargo.toml` for `statvfs`.
4. **`Sample` type** should be evaluated for promotion from `russell-sentinel` to `russell-core` (it's a domain type used across crate boundaries via the `ProviderHealth` trait).

## Implementation Order

1. Move `Sample` to `russell-core` (or define `ProviderHealth` to return a core-level type).
2. ~~Refactor `probes.rs` → `probes/mod.rs` + `probes/memory.rs`.~~ **Done** (ADR-0019).
3. Implement `probes/disk.rs` (Task 2) — cheapest, highest value, no new dependencies beyond `nix`.
4. Implement `ProviderHealth` trait in `russell-core/src/provider.rs` (Task 3 port).
5. Implement provider adapters in `probes/packages.rs` (Task 3 adapters).
6. Implement `ProvenanceAdapter` in `probes/provenance.rs` (Task 4).
7. Wire into `collect()` / `collect_extended()` (Task 5 orchestration).
8. Verify: `russell digest` and `russell jack` surface new samples without code changes (they already query all samples generically).

## Relationship to Kask

When Russell operates as part of the Kask platform:

- The **MCP connector** (`russell-mcp`, currently deferred) exposes journal samples as MCP tool responses.
- The **LLM connector** (`russell-meta`) shares machine status with Jack for assessment via SOAP bundles sent to OpenRouter.
- Both are **connectors** in the tool/connector sense: they transfer formed data across boundaries without transformation logic.

The disk/package probes produce the **data** that flows through these connectors. The probes themselves are unaware of Jack, Kask, or any external consumer.

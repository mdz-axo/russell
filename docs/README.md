---
title: "Russell Documentation Portal"
audience: [operators, developers, contributors, architects, agents]
last_updated: 2026-05-09
togaf_phase: "Cross-cutting — Documentation Governance"
version: "1.0.0"
status: "Active"
---

# Russell Documentation Portal

<!-- TOGAF_DOMAIN: Cross-cutting — Documentation Governance -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-05-09 -->

This is the **single navigation entrypoint** for Russell's active
documentation corpus. Russell is small; the corpus should be too.
If you cannot find what you need from this page, it probably does
not belong in the corpus (yet).

## 1. Quick Navigation

| Section | Contents | TOGAF Phase | Audience |
|---|---|---|---|
| [`architecture/`](architecture/) | System architecture & principles | A / Preliminary | All |
| [`architecture/adr/`](adr/) | Architecture Decision Records (13 active, 7 deferred) | H | Developers |
| [`specifications/`](specifications/) | MVP boundary, persistence catalog | Requirements | All |
| [`standards/`](standards/) | Documentation, coding, commits, safety, ADR process | Preliminary | Contributors |
| [`operations/`](operations/) | Reuse manifest, operator runbooks | D / G | Developers, Operators |
| [`status/`](status/) | Where we actually are | G | All |
| [`templates/`](templates/) | Copy-when-authoring scaffolds | — | Contributors |
| [`archive/`](archive/) | Superseded content, with provenance | Repository | Reference |

## 2. Critical Set (Authoritative Documents)

Only these documents are **authoritative**. Supplementary documents
may exist but must not contradict these.

### 2.1 Root level

- [`../AGENTS.md`](../AGENTS.md) — binding vocabulary, authority
  hierarchy, what Russell is.
- [`../README.md`](../README.md) — one-liner + reading order.
- [`../CONTRIBUTING.md`](../CONTRIBUTING.md) — dev environment,
  tests, local build.
- [`../MACHINE_PROFILE.md`](../MACHINE_PROFILE.md) — the patient's
  chart.
- [`../cybernetic-health-harness.md`](../cybernetic-health-harness.md)
  — the design document (aspirational target).

### 2.2 Architecture

- [`architecture/PRINCIPLES_CATALOG.md`](architecture/PRINCIPLES_CATALOG.md)
  — JR-1 through JR-7.
- [`architecture/overview.md`](architecture/overview.md) —
  current crate topology + VSM mapping.
- [`architecture/TOGAF_TRACEABILITY_MATRIX.md`](architecture/TOGAF_TRACEABILITY_MATRIX.md)
  — ADM phase coverage.

### 2.3 Specifications

- [`specifications/MVP_SPEC.md`](specifications/MVP_SPEC.md) —
  the pinned MVP boundary. **Read before adding a feature.**
- [`specifications/PERSISTENCE_CATALOG.md`](specifications/PERSISTENCE_CATALOG.md)
  — every byte Russell writes, named.

### 2.4 Standards

- [`standards/DOCUMENTATION_STANDARDS.md`](standards/DOCUMENTATION_STANDARDS.md)
  — this file's rules-of-the-road.
- [`standards/coding-rust.md`](standards/coding-rust.md) — Rust
  conventions.
- [`standards/commits.md`](standards/commits.md) —
  Conventional Commits dialect.
- [`standards/adr.md`](standards/adr.md) — how to author an ADR.
- [`standards/safety.md`](standards/safety.md) — IDRS contract,
  risk bands, kill switches.

### 2.5 Operations

- [`operations/INSTALL.md`](operations/INSTALL.md) — operator runbook.
- [`operations/REUSE_MANIFEST.md`](operations/REUSE_MANIFEST.md)
  — copy-with-provenance register.

### 2.6 Status

- [`status/CONSOLIDATED-STATUS.md`](status/CONSOLIDATED-STATUS.md)
  — where we are.
- [`status/SOAK_FINDINGS.md`](status/SOAK_FINDINGS.md)
  — observational backlog during Phase 1c.

### 2.7 ADRs (active)

- [`adr/0001-scope-and-charter.md`](adr/0001-scope-and-charter.md)
- [`adr/0002-licensing.md`](adr/0002-licensing.md)
- [`adr/0004-sqlite-journal.md`](adr/0004-sqlite-journal.md)
- [`adr/0006-profile-abstraction.md`](adr/0006-profile-abstraction.md)
- [`adr/0008-llm-triage-never-emits-shell.md`](adr/0008-llm-triage-never-emits-shell.md)
- [`adr/0011-testing-strategy.md`](adr/0011-testing-strategy.md)
- [`adr/0013-rust-workspace-layout.md`](adr/0013-rust-workspace-layout.md)
- [`adr/0015-proprioception-self-health.md`](adr/0015-proprioception-self-health.md)
- [`adr/0016-doctor-and-llm-router.md`](adr/0016-doctor-and-llm-router.md)
- [`adr/0017-reuse-over-dependency.md`](adr/0017-reuse-over-dependency.md)
- [`adr/0018-close-phase-1c.md`](adr/0018-close-phase-1c.md)
- [`adr/0019-probe-cadence-and-okh.md`](adr/0019-probe-cadence-and-okh.md)
- [`adr/0020-threshold-gated-llm-escalation.md`](adr/0020-threshold-gated-llm-escalation.md)
- [`adr/0021-proprioception-phase2-reflex-arcs.md`](adr/0021-proprioception-phase2-reflex-arcs.md)
- [`adr/0022-markdown-memory-layer.md`](adr/0022-markdown-memory-layer.md)

### 2.8 ADRs (deferred — not MVP-load-bearing but retained)

The following ADRs are **Accepted** but their subjects are
explicitly outside the MVP boundary (see
[`specifications/MVP_SPEC.md`](specifications/MVP_SPEC.md) §5).
They live under [`adr/deferred/`](adr/deferred/):

- 0003 MCP transport
- 0005 Privileged operations (PolKit)
- 0007 YAML skill manifests
- 0009 Tokio runtime
- 0010 Observability stack
- 0012 Config formats
- 0014 Skill manifest licensing

## 3. Reading Paths by Audience

### Operators (running Russell on a workstation)

1. [`../README.md`](../README.md) — what Russell is.
2. [`specifications/MVP_SPEC.md`](specifications/MVP_SPEC.md)
   §1–2 — the six verbs.
3. [`specifications/PERSISTENCE_CATALOG.md`](specifications/PERSISTENCE_CATALOG.md)
   §2 — where your data lives.
4. [`standards/safety.md`](standards/safety.md) — what Russell
   will and will not do.

### New contributors

1. [`../AGENTS.md`](../AGENTS.md) — vocabulary and posture.
2. [`../CONTRIBUTING.md`](../CONTRIBUTING.md) — environment.
3. [`architecture/PRINCIPLES_CATALOG.md`](architecture/PRINCIPLES_CATALOG.md)
   — the principles.
4. [`standards/DOCUMENTATION_STANDARDS.md`](standards/DOCUMENTATION_STANDARDS.md)
   — how docs work.
5. [`standards/coding-rust.md`](standards/coding-rust.md) —
   how code works.
6. [`status/CONSOLIDATED-STATUS.md`](status/CONSOLIDATED-STATUS.md)
   — where to pick up.

### AI agents (acting on Russell's behalf or extending it)

1. [`../AGENTS.md`](../AGENTS.md) — the binding vocabulary (read first).
2. [`architecture/PRINCIPLES_CATALOG.md`](architecture/PRINCIPLES_CATALOG.md)
   — JR-1 is non-negotiable.
3. [`specifications/MVP_SPEC.md`](specifications/MVP_SPEC.md)
   — the pinned boundary.
4. [`standards/safety.md`](standards/safety.md) — the IDRS
   contract.

### Architects

1. [`architecture/PRINCIPLES_CATALOG.md`](architecture/PRINCIPLES_CATALOG.md)
2. [`architecture/TOGAF_TRACEABILITY_MATRIX.md`](architecture/TOGAF_TRACEABILITY_MATRIX.md)
3. [`architecture/overview.md`](architecture/overview.md)
4. ADRs in the active set (§2.7 above).

## 4. Current Counts

| Bucket | Count |
|---|---|
| Active authoritative docs | 67 |
| Active ADRs | 16 |
| Deferred ADRs | 8 |
| Archived docs | 3 |
| Templates | 5 |

_Verify with `find docs -type f -name '*.md' -not -path 'docs/archive/*' | wc -l`; current as of `last_updated`._

## 5. Governance

This portal is the authoritative entry point for the corpus.
Adding a new authoritative document requires adding it to §2
(Critical Set) in the same changeset. Adding a non-authoritative
document requires stating so explicitly in its frontmatter and
linking from the relevant authoritative doc.

See [`standards/DOCUMENTATION_STANDARDS.md`](standards/DOCUMENTATION_STANDARDS.md)
for the full procedure.

## 6. Conventions

- **Voice.** Per [`standards/DOCUMENTATION_STANDARDS.md`](standards/DOCUMENTATION_STANDARDS.md)
  §7, pick the register that matches reader-task.
- **Frontmatter.** YAML + HTML metadata block on every authoritative
  document.
- **Diagrams.** Mermaid only, with `DIAGRAM_ALIGNMENT` block.
- **Links.** Relative inside the repo, absolute outside.

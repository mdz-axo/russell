---
title: "ADR-0013: Rust Workspace Layout"
audience: [developers, architects]
last_updated: 2026-04-18
togaf_phase: "H — Change Management"
version: "1.0.0"
status: "Active"
---

<!-- TOGAF_DOMAIN: Change Management -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-04-18 -->

<!--
audience: anyone adding crates / moving code between crates
last-reviewed: 2026-04-17
-->

# ADR-0013: Rust workspace layout — seven crates, DAG rooted at `russell-core`

- **Status:** Accepted
- **Date:** 2026-04-17
- **Deciders:** Project founders
- **Tags:** `workspace`, `structure`, `builds`

## Context

Russell's responsibilities partition cleanly: profile and
journal (shared foundation), telemetry (Sentinel), skill
dispatch, triage (Doctor), self-health (proprioception), MCP
server, CLI. Squashing all of these into a single crate makes
compile times and incremental review painful; a flat
multi-crate layout without a rule leads to edges in every
direction.

## Decision

A cargo workspace with **seven library crates and one binary
crate**:

```
russell/
├── Cargo.toml                  # [workspace]
├── rust-toolchain.toml
└── crates/
    ├── russell-core/           # profile, journal, paths, IDRS primitives, event schema, telemetry init
    ├── russell-sentinel/       # host probes, EWMA baselines, rule evaluation
    ├── russell-skills/         # manifest parser, dispatcher, subprocess runner
    ├── russell-doctor/         # supervisor loop, LLM client, SOAP composer, bootstrap state machine
    ├── russell-proprio/        # meta-Sentinel, meta-Doctor, reflex arcs, autoimmune guard
    ├── russell-mcp/            # MCP stdio server, tool handlers, wire schemas
    └── russell-cli/            # binary crate; wires everything to the `russell` entry point
```

Dependency rules (enforced by review + `cargo deny` graph
rules where feasible):

1. **`russell-core` depends on nothing in this workspace.**
2. Every other library crate may depend on `russell-core`.
3. `russell-sentinel`, `russell-skills` do **not** depend
   on each other.
4. `russell-doctor` may depend on `russell-skills` and
   `russell-core`.
5. `russell-proprio` may depend on `russell-core` and
   `russell-skills` (for meta-skills under `skills/self/`).
6. `russell-mcp` may depend on everything except
   `russell-cli`.
7. `russell-cli` is the **only** crate that wires all the
   others; no crate depends on it.

Artifact:

- The binary is named `russell`, produced by
  `russell-cli`, installed to `~/.local/bin/russell`.
- Helper binaries for privileged operations
  (ADR-0005) live in their own `crates/russell-helper-*`
  directories when they arrive.

## Consequences

### Positive

- Incremental builds are small: changing the CLI does
  not rebuild the Sentinel.
- Dependency direction is visually inspectable; a
  cross-cutting change is obvious.
- Tests ride with the code they exercise.
- New contributors have a predictable "where does this
  live?" answer.

### Negative / accepted costs

- Splitting a monorepo has small overhead (extra
  `Cargo.toml`s, re-exports). Worth it for a project
  with this many distinct responsibilities.
- Moving code between crates requires re-exports during
  the migration window. Acceptable.

### Neutral

- Workspace-wide dependency versions are unified in
  `[workspace.dependencies]` at the root `Cargo.toml`.

## Alternatives considered

### Single crate with modules

Rejected. Compile times and reviewability suffer.

### One crate per feature (dozens of crates)

Rejected. Overhead outweighs benefits at our scale.

### Split by "kernel vs. plugins"

Misleading: skills are subprocess scripts
(ADR-0007), not Rust crates. There is no plugin layer
to split.

## Implementation notes

- `Cargo.toml` at workspace root declares
  `resolver = "3"` (Rust 2024 edition default).
- Internal deps use `path = "../russell-core"` form.
- `workspace.package` declares `license`, `edition`,
  `rust-version`, and `repository`; crates inherit via
  `workspace = true`.

## References

- [`../standards/coding-rust.md`](../standards/coding-rust.md) §2
- [`../architecture/overview.md`](../architecture/overview.md) §2
- Rust Book, "Cargo Workspaces":
  https://doc.rust-lang.org/book/ch14-03-cargo-workspaces.html

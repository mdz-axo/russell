---
title: "ADR-0002: Licensing — Dual MIT/Apache-2.0"
audience: [developers, contributors]
last_updated: 2026-04-18
togaf_phase: "H"
version: "1.0.0"
status: "Active"
---

<!-- TOGAF_DOMAIN: Change Management -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-04-18 -->

<!--
audience: anyone contributing code, docs, or skills to Russell
last-reviewed: 2026-04-17
-->

# ADR-0002: Licensing — dual MIT / Apache-2.0

- **Status:** Accepted
- **Date:** 2026-04-17
- **Deciders:** Project founders
- **Tags:** `licensing`, `legal`, `dependencies`

## Context

Russell is open-source software intended to be embedded in a
single user's workstation but also suitable for sharing skill
manifests through a public registry (design document §13).
Skills are separately-licensable artifacts
([ADR-0014](deferred/0014-skill-manifest-licensing.md)).

The Rust ecosystem's de-facto standard is dual-licensed under
MIT and Apache-2.0, which:

- maximizes compatibility with upstream crates,
- gives downstream adopters a choice that suits their own legal
  posture,
- includes an explicit patent grant via Apache-2.0.

## Decision

Russell is licensed **"MIT OR Apache-2.0"** at the project
level. Both license texts live at the workspace root:

- `LICENSE-MIT`
- `LICENSE-APACHE`

Every new source file's header comment includes:

```
SPDX-License-Identifier: MIT OR Apache-2.0
```

`Cargo.toml` for every workspace crate sets
`license = "MIT OR Apache-2.0"`.

Dependency policy (via `deny.toml`, enforced in CI):

- **Allow-listed licenses:** MIT, Apache-2.0, BSD-2-Clause,
  BSD-3-Clause, ISC, Unicode-DFS-2016, CC0-1.0, Zlib,
  MPL-2.0 (weak-copyleft; limited to modifications).
- **Disallowed:** GPL (any version), LGPL (any version),
  AGPL (any version), proprietary, "see LICENSE" without an
  SPDX identifier.
- **Carve-outs:** A new license outside the allow-list
  requires a superseding or amending ADR.

## Consequences

### Positive

- Compatible with the vast majority of Rust crates.
- Clear contributor expectation: your patch is licensed under
  the same dual terms.
- Avoids copyleft propagation into Russell.

### Negative / accepted costs

- Cannot consume GPL-licensed libraries directly. For a
  single-host maintenance tool this is a mild limitation
  (most Linux utilities are GPL shell commands, which
  Russell invokes as subprocesses — a use that does not
  trigger GPL linkage).
- Subprocess invocation of GPL tools (e.g. `smartctl`,
  `fwupdmgr`) is allowed and not considered a license
  violation; this is the standard interpretation.

### Neutral

- Dual licensing is the Rust ecosystem default; no signal
  value added or subtracted.

## Alternatives considered

### MIT alone

Rejected. No explicit patent grant; the Rust ecosystem's
convention is dual.

### Apache-2.0 alone

Rejected. Some downstream adopters prefer MIT; dual keeps the
choice open.

### GPL-3.0-or-later

Rejected. Would limit dependency choices and ecosystem
compatibility; also pushes viral terms onto integrators.
Neither helps Russell's single-host mission.

### Mozilla Public License 2.0

Rejected for the core; permitted as a dependency license.
MPL's file-level copyleft is reasonable but conflicts with
Rust ecosystem norms for new projects.

## Implementation notes

- CI step: `cargo deny check` against `deny.toml`.
- New crates in the workspace inherit `license` from
  `Cargo.toml.workspace.package`.
- Contributor guide (already present in
  [`CONTRIBUTING.md`](../../CONTRIBUTING.md)) states the
  license implicitly by repository convention. No separate
  CLA is required; inbound = outbound as is customary.

## References

- Rust API Guidelines: https://rust-lang.github.io/api-guidelines/
- SPDX license list: https://spdx.org/licenses/
- `deny.toml` template: https://embarkstudios.github.io/cargo-deny/
- [ADR-0014](deferred/0014-skill-manifest-licensing.md) — licensing of
  third-party skill manifests.

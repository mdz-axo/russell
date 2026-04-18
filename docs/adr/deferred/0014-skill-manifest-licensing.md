---
title: "ADR-0014: Skill Manifest Licensing (Deferred)"
audience: [developers, architects]
last_updated: 2026-04-18
togaf_phase: "H — Change Management"
version: "1.0.0"
status: "Accepted — Deferred"
---

<!-- TOGAF_DOMAIN: Change Management -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Accepted — Deferred -->
<!-- LAST_UPDATED: 2026-04-18 -->

> **Deferred.** This ADR's subject is outside the MVP boundary per
> [`../../specifications/MVP_SPEC.md`](../../specifications/MVP_SPEC.md) §5.
> It remains **Accepted** — when its phase opens, it ships this way.
> See [`README.md`](README.md) for the deferral register.

<!--
audience: skill authors and registry contributors
last-reviewed: 2026-04-17
-->

# ADR-0014: Licensing of third-party skill manifests

- **Status:** Accepted
- **Date:** 2026-04-17
- **Deciders:** Project founders
- **Tags:** `licensing`, `skills`, `registry`

## Context

Russell core is dual-licensed
[ADR-0002](../0002-licensing.md). Skills are **data plus
scripts** (ADR-0007) that may be authored separately and
eventually fetched from a remote registry
([`cybernetic-health-harness.md` §13](../../../cybernetic-health-harness.md)).
Third-party skills may reasonably carry licenses the core
does not.

Two concerns:

1. **Inbound license clarity.** Russell should know at
   load time under what terms it is running a fetched
   skill, and refuse skills whose license is
   unspecified or incompatible with execution.
2. **Outbound compatibility.** Bundling a skill in the
   core release tarball must not violate the skill's
   license or the core's dual license.

## Decision

1. **Every skill directory contains a `LICENSE` file.**
   The `manifest.yaml` declares its license via SPDX:

   ```yaml
   id: gpu-doctor
   license: "MIT OR Apache-2.0"
   ```

   Missing SPDX = skill fails to load.

2. **Allowed SPDX identifiers** for skills loaded at
   runtime:

   - `MIT`, `Apache-2.0`, `MIT OR Apache-2.0`
   - `BSD-2-Clause`, `BSD-3-Clause`
   - `ISC`
   - `CC0-1.0`, `Unicode-DFS-2016`, `Zlib`
   - `MPL-2.0` (with `scripts/` clearly delineating
     any MPL-modified files)
   - `GPL-2.0-only`, `GPL-2.0-or-later`,
     `GPL-3.0-only`, `GPL-3.0-or-later` — **allowed for
     runtime execution** because scripts are executed
     as subprocesses, not linked. Shipping such skills
     in Russell's own tarball is **not** permitted;
     they can only be installed via the registry path
     or by the operator manually.
   - `LGPL-*` — treated like GPL: runtime-allowed,
     not-bundled-by-us.

3. **Disallowed:** proprietary / "all rights reserved" /
   "no redistribution" / `AGPL-*` (the network-copyleft
   triggers of AGPL are a poor match for
   Russell's scope and would quietly relicense
   upstream users' host actions).

4. **Core-shipped skills** (skills authored by
   Russell's own contributors and included in the
   release tarball) MUST be `MIT OR Apache-2.0` to
   match the core. This is the same inbound = outbound
   rule as the Rust codebase.

5. **Registry fetches** verify the manifest's
   `license:` field matches the registry's advertised
   metadata and refuse mismatches as a poka-yoke.

6. **Manifest `references:`** do not incur license
   obligations by being listed.

## Consequences

### Positive

- Operators know what licenses they are running.
- The core remains permissive without blocking
  third-party GPL skills from the registry.
- `cargo deny` equivalent for skills is built into the
  manifest loader.

### Negative / accepted costs

- Some skills the community might build under GPL
  cannot be shipped inside Russell's own release. They
  ride via the registry, which is the correct path.
- More fields to validate; mitigated by schema tests.

### Neutral

- SPDX is the standard; using it adds no novelty.

## Alternatives considered

### Require MIT/Apache for all skills

Rejected. Excludes many useful community playbooks
(especially ones that wrap GPL CLI tools).

### Accept any license at runtime, including proprietary

Rejected. Operators deserve a clear "what am I
running" answer.

### License per-file inside a skill

Accepted as a valid refinement: a skill's `LICENSE`
file may carry SPDX headers per referenced script, and
the manifest `license:` field reflects the dominant
license. Inconsistency is a load-time error.

## Implementation notes

- Manifest loader parses `license:` via the
  `spdx` crate (or equivalent) and evaluates against
  the allow-list above.
- Registry fetcher (Phase 4) signs each manifest and
  records the signing identity alongside the license;
  both are displayed by `list_skills`.
- The `evidence_read` MCP tool surfaces the skill's
  license in the SOAP bundle footer.

## References

- SPDX license list: https://spdx.org/licenses/
- [ADR-0002](../0002-licensing.md)
- [ADR-0007](0007-yaml-manifest-subprocess-skill-model.md)
- [`cybernetic-health-harness.md` §13](../../../cybernetic-health-harness.md)

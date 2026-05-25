---
title: "ADR-0001: Scope and Charter"
audience: [architects, developers, contributors]
last_updated: 2026-04-18
ddmvss_context: "cross-cutting"
ddmvss_artifact: "adr"
version: "1.0.0"
status: "Active"
---



<!--
audience: anyone asking "what is Russell for?"
last-reviewed: 2026-04-17
-->

# ADR-0001: Scope and charter

- **Status:** Accepted
- **Date:** 2026-04-17
- **Deciders:** Project founders
- **Tags:** `scope`, `charter`, `policy`

## Context

[`cybernetic-health-harness.md`](../../cybernetic-health-harness.md)
describes an ambitious apparatus: continuous telemetry, tiered
hygiene, a specialist triage loop, a signed skill registry, chaos
engineering, and migration from a pile of shell scripts under
`~/Clones/scripts/`. It could easily grow into a general-purpose
maintenance framework or a fleet manager. It must not.

[`MACHINE_PROFILE.md`](../../MACHINE_PROFILE.md) pins the patient:
a single Framework 16 / Ryzen AI / Radeon / Ubuntu workstation.
Every design choice — user-scoped systemd, SQLite in
`~/.local/state`, opt-in network — assumes exactly one operator on
exactly one machine.

Without a locked charter, PR reviews will bleed scope.

## Decision

Russell v0.x's charter is:

> **Russell is a single-host, single-operator cybernetic health
> harness for a Linux AI/ML workstation, exposed to local agent
> frontends as an MCP server. Its remit is observation, cadenced
> hygiene, and bounded intervention under the IDRS contract. It
> is not a fleet manager, not a remote-admin tool, not a general
> observability platform, and not an alerting SaaS.**

Concretely:

1. **One machine at a time.** The journal, profile, and evidence
   live on the host; there is no central aggregator, no
   multi-host ID namespace, no cross-machine correlation.
2. **One operator.** Russell assumes the human running it is
   also the Policy layer. No multi-tenant auth, no role model
   beyond "the user who launched systemd --user".
3. **Local first.** Network access is opt-in per subsystem
   (LLM, remote skill fetch, firmware LVFS refresh). Nothing
   phones home.
4. **MCP stdio only in v1.** Agent frontends run on the same
   machine. See ADR-0003.
5. **Host OS is Linux.** Ubuntu 25.10 is the primary target;
   other Linux distros are supported best-effort. macOS and
   Windows are out of scope.
6. **The medical metaphor is load-bearing.** Sentinel, Doctor,
   skill, SOAP, IDRS. See
   [`AGENTS.md`](../../AGENTS.md) §3.
7. **"First, do no harm."** The default posture is
   **observe > recommend > act**.

## Consequences

### Positive

- Reviews have a clear veto reason: "out of charter."
- The attack surface stays small: no network listener, no
  privilege escalation by default, no remote RPC.
- The migration target is known: a single user, one set of
  legacy scripts, one machine profile.

### Negative / accepted costs

- A second operator on the same machine is out of scope.
  When that day comes, we will need an ADR that re-opens the
  charter.
- Multi-machine correlation (e.g. comparing two Framework
  16s' baselines) is out of scope.
- We cannot "just add" a web UI or a remote-admin endpoint;
  that is a charter change.

### Neutral

- The architecture does not preclude a future multi-host
  version; nothing in the data plane is single-host on
  purpose. But growing into that shape is a new ADR, not a
  gradual drift.

## Alternatives considered

### Generalize to "workstation maintenance framework"

Rejected. A framework has users; Russell has one user. The
design document's specificity (Framework 16 / ROCm / Ubuntu) is
the feature, not a limitation.

### Scope to a single subsystem (e.g. "ROCm doctor only")

Rejected. The legacy scripts under `~/Clones/scripts/` cover
Rust toolchain, editor drift, disk hygiene, and more. Scoping
Russell down to one subsystem would leave the rest unmanaged
and eventually recreate the pile-of-scripts problem.

### Build on top of an existing tool (Ansible, chezmoi,
systemd-homed hooks)

Rejected. The canonical design document's cybernetic loop —
continuous Sentinel + triaged Doctor + LLM-consulted skill
dispatch — is not expressible in these tools without extensive
plumbing that would itself become "Russell-shaped."

## Implementation notes

The charter is enforced by review. Introducing a feature that
violates the charter requires an ADR that either

1. revises this charter (supersedes this ADR), or
2. documents a narrow carve-out with a sunset clause.

Examples already in the design document that would trigger a
charter check if attempted casually: remote skill registry
auto-publish, cross-machine baselines, a web dashboard.

## References

- [`cybernetic-health-harness.md` §1](../../cybernetic-health-harness.md)
- [`MACHINE_PROFILE.md`](../../MACHINE_PROFILE.md)
- [`AGENTS.md`](../../AGENTS.md)

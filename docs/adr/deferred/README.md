---
title: "Deferred ADRs"
audience: [developers, architects, contributors]
last_updated: 2026-04-18
togaf_phase: "H — Change Management"
version: "1.0.0"
status: "Active"
---

# Deferred ADRs

<!-- TOGAF_DOMAIN: Change Management -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Active -->

These ADRs are **Accepted** — i.e., when their subject ships,
it ships this way. But their subject is explicitly outside the
MVP boundary per
[`../../specifications/MVP_SPEC.md`](../../specifications/MVP_SPEC.md)
§5.

Being deferred is not a demotion. It is a sequencing fact. Each
ADR here gets lifted back to `docs/adr/` (active) when the
corresponding feature's phase opens.

## Contents

| ADR | Subject | Lifts in phase |
|---|---|---|
| [0003](0003-mcp-transport.md) | MCP stdio transport | Phase 4+ |
| [0005](0005-privileged-operations.md) | PolKit + helper binaries for root-requiring actions | Phase 3 (if apt/fwupd skills) |
| [0007](0007-yaml-manifest-subprocess-skill-model.md) | YAML skill manifests + subprocess dispatch | Phase 3 |
| [0009](0009-tokio-runtime.md) | Tokio runtime posture | Already implicit in code; formal lift when MCP/Doctor async grows |
| [0010](0010-observability-stack.md) | `tracing`-only observability | Partial (core uses `tracing`); formal lift with journald + OTel guidance |
| [0012](0012-config-formats.md) | TOML/YAML/JSON split | Lift with Phase-2 rules engine |
| [0014](0014-skill-manifest-licensing.md) | SPDX discipline on skills | Lift with Phase-3 registry |

## Why keep them

Re-deriving these decisions every time the phase opens is exactly
the cost Russell's ADR discipline is designed to avoid. Freezing
them while deferred is cheap; re-arguing them is not.

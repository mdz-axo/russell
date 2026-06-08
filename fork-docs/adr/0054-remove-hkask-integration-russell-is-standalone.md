---
title: "ADR-0054 — Remove hKask Integration: Russell Is Standalone"
audience: [architects, developers, operators]
last_updated: 2026-06-07
ddmvss_context: "cross-cutting"
ddmvss_artifact: "adr"
version: "1.0.0"
status: "Active"
---

# ADR-0054 — Remove hKask Integration: Russell Is Standalone

**Date:** 2026-06-07
**Status:** Active
**Supersedes:** ADR-0025 (hKask MCP Client Trusted Relationship)
**Updates:** ADR-0027 (ACP Integration), ADR-0042 (Adversarial Review)
**Scope:** Full Russell codebase, documentation, persona prompts, skills

---

## Context

Russell was originally built as a host curator within hKask, a multi-agent cybernetic system. The integration was documented in ADR-0025 (MCP Client Trusted Relationship) and relied on three integration paths:

1. **MCP client** — Russell called hKask's MCP spec server (`spec/graph/query`, `spec/curate/evaluate`) for spec-anchored operations.
2. **CNS feedback** — Russell emitted and consumed CNS spans (`cns.cybernetics.*`) for variety monitoring and algedonic signaling.
3. **Tool registry** — Russell registered as a tool provider in hKask's dual-layer skill registry.

The adversarial review (ADR-0042) found that all three integration paths were structurally broken: wire protocols diverged, authentication mechanisms were incompatible, and payload schemas did not match. The integration was never functional in production.

Russell's core value — single-host health monitoring with a nurse persona — does not depend on any of these integration paths. The Sentinel, Journal, Nurse, Proprioception, and Skill subsystems all operate independently.

## Decision

Remove all hKask integration. Russell is now a standalone single-host harness.

Specifically:

1. **Remove MCP client code** — No external spec server calls. Spec-anchored operations (diagnose skill, TDD skill) reference local spec documents (`docs/specifications/MVP_SPEC.md`, `fork-docs/adr/`, `AGENTS.md`) instead of calling `spec/graph/query`.
2. **Remove CNS references** — No `cns.cybernetics.*` spans, no variety counters, no algedonic signals. Proprioception (8 self-observation points + 2 boolean integrity checks) is Russell's native self-regulation mechanism.
3. **Remove dual-layer skill registry** — Russell has a single-layer skill model. Skills live at `~/.local/share/harness/skills/<id>/` with YAML manifests and bash scripts. No FlowDef/Jinja2 templates, no registry templates.
4. **Remove hKask references from source, docs, persona prompts** — All `hkask`, `hKask`, `kask` references removed from Rust source, Markdown documentation, and Jack's persona prompt.
5. **Remote MCP tools via `ACTION: remote/` syntax** — The only interaction with external systems is through the `ACTION: remote/<tool>` syntax in the chat REPL, which is a local action resolution, not a remote procedure call.

## Consequences

### Positive

- **Simpler architecture** — Russell is now a self-contained single-host system. No external dependencies, no cross-process communication protocols to maintain.
- **Clean build** — Removing dead integration code eliminated compile errors and clippy warnings.
- **Clearer operational model** — The operator interacts with Russell through three surfaces (CLI, API, ACP) that all exercise the same functional core. No hidden coupling to an external system.
- **Better spec alignment** — Specs now reference local documents, making spec-anchored operations deterministic and offline-capable.
- **Skills are portable** — Agent skills (TDD, diagnose, coding-guidelines, etc.) ported from hKask now reference Russell's architecture and documentation, not hKask's.

### Negative

- **No cross-machine coordination** — Russell cannot coordinate with other instances or report to a central system. This was already out of scope (Magna Carta H-1: single-host invariant).
- **No spec graph queries** — The `diagnose` skill can no longer call `spec/graph/query`. It references local spec documents instead. This is less automated but more deterministic.
- **No CNS variety monitoring** — Russell's proprioception subsystem provides self-regulation, but it does not have the variety counters or algedonic signaling that CNS provided. The pragmatic-cybernetics skill provides the analytical framework, but the automated enforcement is absent.
- **Skill ecosystem is local** — No shared skill marketplace or registry. Skills are installed locally via `russell skill install` or manually.

### Neutral

- **`ACTION: remote/` syntax** — This syntax for invoking remote MCP tools is preserved as a local action resolution mechanism. It does not depend on hKask's MCP server.

## Migration

The following changes were made:

| Area | Change |
|------|--------|
| Source code | Removed all `hkask`, `hKask`, `kask` references from Rust source |
| Documentation | Updated all docs to reference Russell's standalone architecture |
| Persona prompt | Removed hKask-specific instructions from Jack's prompt |
| Skills | Ported 10 agent skills from hKask, adapted for Russell's architecture |
| ADR-0025 | Superseded by this ADR |
| ADR-0027 | Updated: ACP integration no longer references hKask MCP paths |
| ADR-0042 | Updated: scope narrowed to standalone Russell |
| Tests | 62 new tests added (28 in russell-protocol, 34 in russell-core journal) |

## References

- ADR-0025 (superseded) — hKask MCP Client Trusted Relationship
- ADR-0027 (updated) — ACP Integration
- ADR-0042 (updated) — Adversarial Multi-Perspective Review & Remediation Plan
- `AGENTS.md` — Russell's operational guide
- `fork-docs/architecture/magna-carta.md` — Russell's charter (H-1: single-host invariant)
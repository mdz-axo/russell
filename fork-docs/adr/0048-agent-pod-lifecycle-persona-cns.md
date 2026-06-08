---
title: "ADR-0048: Agent Pod — Sovereign Entity with Lifecycle, Persona, and CNS Integration"
audience: [architects, developers, agents]
last_updated: 2026-05-24
ddmvss_context: "acp"
ddmvss_artifact: "adr"
version: "1.0.0"
status: "Accepted"
---



# ADR-0048: Agent Pod — Sovereign Entity with Lifecycle, Persona, and CNS Integration

- **Status:** Accepted
- **Date:** 2026-05-24
- **Deciders:** Project operator
- **Tags:** `agent`, `pod`, `lifecycle`, `persona`, `cns`, `phase-4`

## Context

Russell evolved from a collection of independent crates (sentinel, meta,
skills, proprio) coordinated by CLI commands and systemd timers. The
Phase 4 security hardening (ADR-0042) introduced macaroon
authentication, capability tokens, and CNS span emission — all requiring
a unified entity that owns these concerns. Without a pod abstraction:

1. **No identity.** Each crate independently reads `profile.json`; no
   single entity represents "Russell" to the CNS or ACP layer.
2. **No lifecycle.** Starting and stopping sentinel + ACP server is ad-hoc
   shell scripting; no state machine enforces ordering or prevents
   invalid transitions.
3. **No artifact management.** Evidence bundles, semantic triples, and
   episodic memory scatter across the filesystem without a unified
   visibility model.

The ACP server (ADR-0027) requires an authenticated agent identity.
The CNS emission layer (ADR-0044) requires a pod-scoped span namespace.
These demands converge on a single abstraction: the Agent Pod.

## Decision

Introduce `russell-agent`, a crate implementing the **Agent Pod** — a
sovereign entity with a four-state lifecycle, a YAML-parsed persona,
CNS observability, and memory artifact storage.

### Lifecycle state machine

```
Populated → Registered → Activated → Deactivated
```

- **Populated:** Persona loaded from `agent_persona.yaml`; journal,
  rule set, and artifact store initialized. Pod has an identity but
  cannot act.
- **Registered:** ACP server connection validated; capability token
  obtained. Pod can receive requests but has not started observation.
- **Activated:** Sentinel timer and ACP server running. Pod observes,
  reports, and responds to agents.
- **Deactivated:** Sentinel and ACP server stopped. Terminal state;
  pod must be re-populated.

Same-state and backward transitions are rejected. This linear
progression prevents partial activation and ensures orderly shutdown.

### Persona

`AgentPersona` is loaded from `agent_persona.yaml` containing:

- `agent`: name, type (Bot / Replicant), version
- `charter`: description, editor
- `capabilities`: list of capability strings (e.g., `tool:system:probe`)
- `rights`: read/write access lists
- `responsibilities`: behavioral constraints
- `visibility`: default and episodic override (public / private)

Validation requires non-empty `name`, `charter.description`,
`capabilities`, and `responsibilities`.

### CNS observability

The pod emits structured spans via the `CnsPort` hexagonal port:

| Span name | Transition |
|---|---|
| `cns.russell.populated` | Pod created |
| `cns.russell.registered` | ACP token obtained |
| `cns.russell.activated` | Sentinel + ACP running |
| `cns.russell.deactivated` | Shutdown |
| `cns.russell.probe.executed` | Sentinel cycle complete |
| `cns.russell.skill.dispatched` | Skill execution |
| `cns.russell.llm.escalation` | Nurse LLM call |

Spans degrade gracefully: if `REMOTE_CNS_ENDPOINT` is not set, spans
log locally via `tracing::info!`. No hard dependency on an external CNS.

### Artifact storage

`ArtifactStore` manages four artifact directories under a base path:

- `semantic/` — structured triple files
- `episodic/` — session episode files
- `evidence/` — evidence bundles
- `skills/<id>/` — skill-specific artifacts

Visibility annotations (Public / Private / OperatorOnly) control which
artifacts the `export()` method includes.

### CLI verbs

Six new CLI verbs expose pod operations:

- `russell pod-status` — show lifecycle state
- `russell pod-activate` — transition to Activated
- `russell pod-deactivate` — transition to Deactivated
- `russell pod-persona-show` — display current persona
- `russell pod-artifacts-list` — list stored artifacts
- `russell pod-artifacts-export` — export by visibility

## Reference Models

| Concept | Source |
|---|---|
| Pod lifecycle | Kubernetes Pod lifecycle (Pending → Running → Succeeded/Failed). Cleveland, B. et al. (2015). *Kubernetes: Up and Running*. O'Reilly. |
| Agent identity | Beer, S. (1972). *Brain of the Firm*. VSM S1/S2 identity channels. |
| Hexagonal port | Cockburn, A. (2005). *Hexagonal Architecture*. `CnsPort` as adapter interface. |
| NuEvent schema | CNS specification. See [ADR-0044](0044-cns-span-emission.md). |

## Alternatives Considered

1. **No pod — keep crates independent.** Rejected: macaroon auth and
   CNS spans require a unified identity. Without a pod, each crate
   independently discovers the endpoint, authenticates, and emits
   spans — duplicating state and risking inconsistency.
2. **Pod as a trait, not a struct.** Rejected: the pod owns concrete
   resources (journal, sentinel handle, ACP handle, artifact store).
   A trait cannot own these; a struct must.
3. **Five-state lifecycle (adding Error state).** Deferred: the
   current four states cover production needs. An Error state can be
   added when the pod supports self-healing transitions.

## Consequences

- **Positive:** Unified identity for ACP auth and CNS emission.
  Explicit lifecycle prevents partial activation. Artifact visibility
  model supports safe export to agents.
- **Positive:** CLI verbs give the operator direct pod control without
  needing systemd service management for lifecycle transitions.
- **Negative:** `russell-agent` depends on 5 other Russell crates
  (core, sentinel, proprio, acp-server, meta). This is the widest
  dependency surface in the workspace, though it is a leaf node — no
  crate depends on `russell-agent`.
- **Negative:** Sentinel loop is spawned inside the pod's `activate()`
  method. If the pod is dropped without `deactivate()`, the `Drop`
  impl attempts cleanup but cannot guarantee timely termination of
  spawned tasks.

## References

- [ADR-0027: ACP Integration](0027-acp-integration.md)
- [ADR-0042: Adversarial Review Remediation Plan](0042-adversarial-review-remediation-plan.md)
- [ADR-0044: CNS Span Emission](0044-cns-span-emission.md)
- [ADR-0047: Capability Token Rotation](0047-capability-token-rotation.md)
- Beer, S. (1972). *Brain of the Firm*. Wiley.
- Cockburn, A. (2005). *Hexagonal Architecture*.

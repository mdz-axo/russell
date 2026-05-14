---
title: "ADR: Russell Service Principal — Kask-Side Provisioning"
audience: [developers, architects, platform-operators]
last_updated: 2026-05-14
togaf_phase: "Preliminary"
version: "1.0.0"
status: "Proposed"
target_repo: "kask"
---

<!-- TOGAF_DOMAIN: Governance -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Proposed -->
<!-- LAST_UPDATED: 2026-05-14 -->
<!-- NOTE: This ADR should live in the Kask repo alongside Russell's
     ADR-0025. It documents the Kask-side provisioning and policy
     for Russell's service principal. -->

# ADR: Russell Service Principal — Kask-Side Provisioning

- **Status:** Proposed (for Kask repo)
- **Date:** 2026-05-14
- **Deciders:** Platform operator
- **Tags:** `service-principal`, `russell`, `mcp`, `capability-grants`
- **Companion:** [Russell ADR-0025](../0025-kask-mcp-client-trusted-relationship.md)

## Context

Russell is a cybernetic health harness for a single Linux AI/ML
workstation. As of Phase 4, Russell gains an MCP client that
connects to Kask's MCP endpoint on localhost to access Kask-mediated
tool surfaces (Cascade, web research, memory graph, curator findings).

Russell authenticates as a **service principal** — a non-human
consumer with a bounded capability set. This ADR documents:

1. Russell's service principal in Kask's identity system.
2. The default capability grants for Russell.
3. The operational procedure for provisioning Russell.

### Relationship to Kask ADRs

- **Kask ADR-T15** (Admin Role Granularity) — the `principal_capability`
  table and `stack-admin key set --for` mechanism.
- **Kask ADR-T22** (Milton as Platform Curator) — the curator
  taxonomy. Russell is a peer domain curator (host infrastructure).

## Decision

### 1. Russell's service principal identity

Russell is provisioned as a Kask service principal with:

| Field | Value |
|---|---|
| **Principal ID** | `russell` |
| **Principal type** | `service` (non-human) |
| **Display name** | `Russell (Host Curator)` |
| **Authentication** | Bearer token (random 256-bit, hex-encoded) |
| **Token storage (Russell side)** | `KASK_MCP_TOKEN` in `~/.config/harness/russell.env` |
| **Token storage (Kask side)** | `stack-keystore` (via `stack-admin key set`) |

### 2. Default capability grants

Russell's service principal is granted the following capabilities
by default on provisioning:

| Capability | Scope | Rationale |
|---|---|---|
| `mcp:tools/list` | `*` | Russell must discover available tools |
| `mcp:tools/call` | `russell_host_snapshot` | Read-only host telemetry (probe-equivalent) |
| `mcp:tools/call` | `paradigm_shift_query` | Cascade second-opinion analysis |
| `mcp:tools/call` | `memory_graph_query` | Cross-domain correlation |
| `mcp:tools/call` | `web_research_query` | Grounded research |
| `mcp:tools/call` | `curator_findings_query` | Peer curator findings |
| `chat:completions` | `*` | LLM inference via Okapi (existing) |

**Consent model:** Russell's local consent gate (IDRS, JR-2) applies
before any tool call is dispatched to Kask. Kask's `UnifiedPolicyDecision`
is an *additional* layer — Russell cannot bypass Kask's policy, and
Kask cannot override Russell's operator consent model.

**Risk bands:** Tools declare their risk band in `annotations.risk_band`.
Russell's local dispatcher uses this to decide auto-execute vs.
operator consent. The risk band is carried in the MCP tool definition.

### 3. Operational provisioning procedure

```bash
# 1. Provision Russell's service principal and generate a token.
stack-admin key create --for russell --type service --display "Russell (Host Curator)"

# 2. Grant default capabilities.
stack-admin key grant --for russell --capability mcp:tools/list --scope "*"
stack-admin key grant --for russell --capability mcp:tools/call --scope "russell_host_snapshot"
stack-admin key grant --for russell --capability mcp:tools/call --scope "paradigm_shift_query"
stack-admin key grant --for russell --capability mcp:tools/call --scope "memory_graph_query"
stack-admin key grant --for russell --capability mcp:tools/call --scope "web_research_query"
stack-admin key grant --for russell --capability mcp:tools/call --scope "curator_findings_query"

# 3. Obtain the token.
RUSSELL_TOKEN=$(stack-admin key get --for russell)

# 4. Write the token to Russell's environment file.
echo "KASK_MCP_TOKEN=$RUSSELL_TOKEN" >> ~/.config/harness/russell.env

# 5. Verify.
russell mcp-tools
```

### 4. Token lifecycle

- **Creation:** `stack-admin key create --for russell`
- **Rotation:** `stack-admin key rotate --for russell` (old token invalidated)
- **Revocation:** `stack-admin key revoke --for russell` (all capabilities removed)
- **Capability changes:** `stack-admin key grant/revoke --for russell --capability ...`

Russell gracefully degrades when the token is invalid or revoked:
the `kask_mcp_reachable` self-vital triggers `Warn`, and local skills
remain fully operational.

### 5. Security considerations

- **Loopback only.** Russell's MCP client refuses non-loopback
  connections at the transport layer. The bearer token never leaves
  the machine.
- **No token exchange.** Russell uses a fixed bearer token, not a
  short-lived credential. Rotation is a manual operational step.
- **Bounded surface.** Kask's `UnifiedPolicyDecision` evaluates every
  `tools/call` against Russell's capability grants. Russell cannot
  call tools outside his grants.
- **Risk-band gating.** Tools with `risk: none` auto-execute; others
  require operator consent. Unknown risk bands default to `medium`
  (consent required).

## Consequences

### Positive

- Russell gains Kask-mediated capabilities without expanding his
  local attack surface.
- Kask's identity system provides a single place to audit, scope,
  and revoke Russell's access.
- The provisioning procedure is a single `stack-admin` command.
- Russell's graceful degradation ensures no operational dependency
  on Kask availability.

### Negative / accepted costs

- Kask must maintain Russell's service principal. This is trivial
  but adds one more entry to the identity table.
- Token rotation is manual. If Russell's token is compromised, the
  operator must manually rotate and update `russell.env`.
- Russell's capability grants must be kept in sync with the tools
  Kask exposes. A new tool added to Kask without a corresponding
  capability grant will be silently unavailable to Russell.

### Neutral

- Russell remains a peer curator, not a subordinate. His local
  consent model (JR-2) is unchanged.
- Kask's `UnifiedPolicyDecision` is additive — it cannot override
  Russell's operator consent, only further restrict.
- This ADR does not change Russell's ability to call Kask via
  `POST /v1/chat/completions` for LLM inference (existing).

## References

- [Russell ADR-0025](../0025-kask-mcp-client-trusted-relationship.md) — the Russell-side ADR.
- Kask ADR-T15 (Admin Role Granularity) — capability model.
- Kask ADR-T22 (Milton as Platform Curator) — curator taxonomy.
- Russell `MACHINE_PROFILE.md` — the patient.
- Russell `AGENTS.md` §4 — JR-1 through JR-7.

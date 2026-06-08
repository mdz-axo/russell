---
title: "ADR-0025: hKask MCP Client — Trusted Local Relationship [SUPERSEDED]"
 audience: [developers, architects, agents]
last_updated: 2026-06-07
ddmvss_context: "mcp"
ddmvss_artifact: "adr"
version: "2.1.0"
status: "Superseded"
---

> **⚠️ Superseded.** The hKask integration described in this ADR has been removed.
> Russell is now a standalone project. The MCP client for hKask, the trusted
> bilateral relationship, and all hKask-specific authentication and tool
> registry mechanisms are no longer implemented. This ADR is retained for
> historical record only.

# ADR-0025: hKask MCP Client — Trusted Local Relationship

- **Status:** Superseded — hKask integration removed. Russell is now standalone.
- **Date:** 2026-05-14 (original), 2026-05-19 (hKask migration), 2026-06-07 (superseded)
- **Deciders:** Project operator
- **Tags:** `mcp`, `hkask`, `trust`, `tools`, `phase-4`
- **Lifts:** Partial lift of [ADR-0003](0003-mcp-transport.md)
  deferral (client-side only); extends [ADR-0023](0023-lift-adr-0007-phase3-skills.md)
  skill surface to include hKask-mediated tools.

## Context

### Where Russell is today

Russell's Phase 3 skill system is operational. The IDRS-gated
dispatcher can execute local skills (YAML manifest + subprocess)
with operator consent. The LLM selects from known IDs registered
in loaded manifests (JR-3, ADR-0008). The poka-yoke rejects any
ID not in the loaded manifest set.

Russell currently talks to hKask in exactly one way: plain HTTP
`POST /v1/chat/completions` to Okapi for LLM inference. There is
no authentication beyond a placeholder bearer token. Russell has
no MCP client and no awareness of hKask's tool surface.

### Where the system is headed

Russell is a **host curator** — a domain-specific helper in the
hKask ecosystem responsible for local host health. He is not a
pure observer; JR-2's ladder runs through to "act." The goal is
cognitive load shedding and responsibility sharing between the
operator and the agents/curators in the install.

hKask exposes rich MCP tool surfaces: the paradigm-shift Cascade,
web research, memory/knowledge graph, domain curator findings,
and platform operations. These are capabilities Russell could
use to provide better care — deeper analysis, cross-domain
correlation, grounded research — without building those systems
himself.

Meanwhile, Russell has privileged local access (filesystem,
processes, sudo-gated interventions). This privilege means his
MCP client surface must be tightly constrained. Connecting to
arbitrary remote MCP servers would create an unacceptable
escalation vector: an untrusted server could propose tool calls
that interact with Russell's local execution capabilities.

### What this ADR enables

A **preferred/trusted bilateral relationship** between Russell
and the local hKask installation. Russell gains an MCP client
that connects exclusively to hKask-served tool surfaces. hKask
remains the sole trust boundary for tool governance.

## Decision

### 1. Russell gains an MCP client, constrained to hKask

The `russell-mcp` crate is promoted from Phase-0 stub to a
**client-only** implementation. It connects to hKask's MCP
endpoint(s) on localhost and nowhere else.

**Hard constraints:**

- The client connects ONLY to endpoints configured in
  `~/.config/harness/russell.env` under `HKASK_MCP_ENDPOINTS`.
- The default (and initially only supported) value is
  `http://127.0.0.1:<port>/mcp`.
- No general MCP server discovery. No `mcp.json`-style
  arbitrary server registration. No remote hosts.
- The client MUST refuse to connect to any non-loopback
  address. This is enforced at the transport layer, not by
  convention.

### 2. Authentication via hKask service principal

Russell authenticates to hKask as a **service principal** — a
non-human consumer with a bounded capability set. The mechanism:

- A bearer token issued by hKask via
  `stack-admin key set --for russell --scope user`.
- Stored in Russell's environment (`HKASK_MCP_TOKEN` in
  `~/.config/harness/russell.env`) or in the OS keychain
  via hKask's `stack-keystore`.
- hKask's `UnifiedPolicyDecision` evaluates every `tools/call`
  against the capability grants bound to Russell's principal.
- Russell sees only the tools hKask's policy permits. A
  `tools/list` response is already filtered by identity.

### 3. Tool registry is hKask-projected, not self-managed

Russell does not maintain his own remote tool registry. His
tool surface is the **union of**:

1. **Local skills** — YAML manifests loaded from
   `~/.local/share/harness/skills/` (unchanged from Phase 3).
2. **hKask MCP tools** — discovered via `tools/list` from the
   authenticated hKask connection.

The dispatcher's poka-yoke expands accordingly: a valid tool ID
is one that exists in either the local manifest set OR the
cached hKask tool list for the current session.

### 4. hKask is the sole MCP trust boundary

Russell MUST NOT:

- Install, register, or connect to MCP servers outside the
  local hKask installation.
- Accept MCP tool definitions from any source other than an
  authenticated hKask endpoint on loopback.
- Serve as a relay or proxy for external MCP servers.

This is a **structural** constraint, not a policy preference.
Russell's local system access (filesystem, processes, sudo)
makes an unconstrained MCP client a privilege escalation risk.
hKask's policy layer — capability grants, dispatch policy,
constraint evaluation — is the governance mechanism.

If a future need arises for Russell to access tools outside
hKask, it must be mediated: hKask proxies the external tool
surface, applies its policy, and exposes the result to Russell.
Russell never reaches past hKask.

### 5. Graceful degradation when hKask is unavailable

Russell MUST NOT block on hKask availability. When the MCP
endpoint is unreachable:

- Local skills remain fully operational.
- The LLM help channel falls back to offline mode (existing
  behaviour per MVP_SPEC §2.1).
- hKask MCP tools are removed from the available tool set.
- A self-vital (`hkask_mcp_reachable`) tracks connectivity.
  Breach events are journaled.

Russell resumes hKask tool usage automatically when the endpoint
becomes reachable again. No operator intervention required.

### 6. Consent model for hKask MCP tools

hKask MCP tools inherit the same consent model as local skills:

| Tool characteristic | Consent requirement |
|---|---|
| Read-only / observational (risk: none) | Auto-execute (probe-equivalent) |
| Mutates hKask-internal state only (risk: low) | Auto-execute below `max_auto_risk` |
| Mutates host state (risk: medium+) | Operator consent required |
| Unknown risk band | Treated as `high`; consent required |

Risk bands for hKask tools are declared in the MCP tool
definition's metadata (hKask's tool schema supports this via
the `annotations` field). If absent, the tool is treated as
risk: medium (consent required).

### 7. JR-3 extension: the LLM selects from an expanded ID set

JR-3 ("The LLM never emits shell") is unchanged in spirit.
The extension:

- **Before:** The LLM ranks IDs from loaded local manifests.
- **After:** The LLM ranks IDs from loaded local manifests
  AND the hKask MCP tool list.

The LLM still cannot compose commands. It can only emit:
- `ACTION: <local-skill>/<probe-or-intervention>` (existing)
- `ACTION: hkask/<tool-name>` (new)

The dispatcher validates both against their respective
registries. Unknown IDs are rejected (poka-yoke). The hKask
`tools/call` request is constructed by Russell's dispatcher
from the tool's declared `inputSchema` — never from
LLM-generated JSON passed through blindly.

### 8. No remote skills

This ADR does NOT lift the "no remote skill registry" deferral.
Skills (YAML manifest + subprocess bundles) remain local-only.
What changes is that Russell can *call hKask MCP tools* — these
are not skills in Russell's sense (they have no local manifest,
no subprocess, no IDRS contract on Russell's side). They are
service calls to a trusted local system.

The distinction:

| Concept | Location | Governance | IDRS |
|---|---|---|---|
| Local skill | Russell's filesystem | Russell's manifest + dispatcher | Russell enforces |
| hKask MCP tool | hKask's process | hKask's policy layer | hKask enforces |

Russell trusts that hKask enforces its own governance. Russell
does NOT attempt to IDRS-wrap hKask tool calls — that would
violate separation of concerns.

## Consequences

### Positive

- Russell can leverage hKask's Cascade for deeper analysis
  without building his own multi-model orchestration.
- Russell can query cross-domain curator findings (Duncan,
  Trout, etc.) for correlation with host symptoms.
- Russell can use hKask's memory/knowledge graph for
  persistence beyond his local journal.
- The operator gets a more capable host curator without
  Russell's codebase growing to absorb those capabilities.
- The single trust boundary (hKask) means one place to audit,
  one place to revoke, one place to scope.

### Negative / accepted costs

- Russell gains a runtime dependency on hKask for full
  capability. Mitigated by graceful degradation (§5).
- The `russell-mcp` crate adds an MCP client dependency
  (~minimal; Russell can vendor a thin client per JR-6 or
  use `stack-mcp`'s client if it's extractable).
- hKask must provision and maintain Russell's service principal.
  This is an operational step in the hKask install flow.
- Tool availability depends on hKask's policy configuration.
  An overly restrictive policy silently limits Russell's
  effectiveness. Mitigated by the `hkask_mcp_reachable`
  self-vital and tool-count logging.

### Neutral

- ADR-0003 (MCP transport — stdio only) remains valid for
  Russell's **server** surface. This ADR concerns Russell as
  a **client**. They are orthogonal.
- ADR-0008 (LLM never emits shell) is unchanged. The ID set
  grows; the constraint is structural.
- ADR-0023 (Phase 3 skills) is unchanged. Local skills
  continue to work exactly as they do today.

## Implementation Sketch

### Phase 4A — MCP client foundation

1. Promote `russell-mcp` from stub to client implementation.
2. Transport: HTTP REST API to `http://127.0.0.1:18100/api/v1/*`
   (hKask's stack-api provides REST endpoints, not JSON-RPC).
3. Loopback enforcement at connect time (reject non-127.0.0.1/::1).
4. Bearer token auth from `HKASK_MCP_TOKEN`.
5. `tools/list` caching with configurable TTL (default: 5 min).
6. `russell mcp-tools` CLI verb: list available hKask tools.
7. Endpoints:
   - `GET /health` — connectivity check
   - `GET /api/v1/tools` — list all tools from all MCP servers
   - `POST /api/v1/tools/{name}` — invoke a tool

### Phase 4B — Dispatcher integration

1. Extend the poka-yoke to accept `hkask/<tool-name>` IDs.
2. Extend `ACTION:` syntax parsing for `hkask/` prefix.
3. Wire hKask tool calls into the ACP session consent flow.
4. Add `hkask_mcp_reachable` self-vital to proprioception.

### Phase 4C — Doctor integration

1. Include hKask tool list in the SOAP prompt's available
   actions (alongside local skills).
2. Jack can propose `ACTION: hkask/<tool>` in conversation.
3. The Cascade becomes available as a "second opinion" tool
   Jack can invoke when facing complex symptoms.

## What this ADR does NOT authorise

- General remote MCP server connections.
- Russell acting as an MCP server for hKask (that remains
  `arsenal-mcp-russell` in the hKask repo, reading Russell's
  journal directly).
- Auto-dispatch of hKask tools without consent (follows
  existing `max_auto_risk` cap).
- Any change to Russell's local skill model.
- Network listeners on Russell's side.

## Relationship to hKask ADRs

This ADR should be read alongside:

- **hKask ADR-T15** (Admin Role Granularity) — the
  `principal_capability` table that governs what Russell's
  service principal can do.
- **hKask ADR-T22** (Milton as Platform Curator) — establishes
  the curator taxonomy. Russell is a peer domain curator
  (host infrastructure), not a subordinate of Milton.

A companion ADR in the hKask repo should document the
Russell service principal's default capability grants and
the operational procedure for provisioning it.

## References

- [ADR-0003](0003-mcp-transport.md) — MCP transport (stdio, deferred).
- [ADR-0008](0008-llm-triage-never-emits-shell.md) — LLM never emits shell.
- [ADR-0023](0023-lift-adr-0007-phase3-skills.md) — Phase 3 skills.
- [`../../AGENTS.md`](../../AGENTS.md) §6 — IDRS contract.
- hKask `stack-hkask-surface/src/mcp.rs` — the MCP endpoint Russell will connect to.
- hKask `stack-auth` + `stack-control-plane/src/auth.rs` — the policy evaluation path.
- MCP specification: https://modelcontextprotocol.io

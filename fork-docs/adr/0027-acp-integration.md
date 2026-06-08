---
title: "ADR-0027: ACP Integration"
audience: [developers, architects, operators]
last_updated: 2026-05-22
ddmvss_context: "acp"
ddmvss_artifact: "adr"
version: "1.0.0"
status: "Implemented"
---



# ADR-0027: ACP Integration

## Context

Russell operates as a cybernetic health harness for a single Linux AI/ML workstation. Russell provides an ACP (Agent Client Protocol) server that enables external agents to interact with Russell's capabilities.

The design goal is for Russell to expose a subset of its capabilities (public skills, host probes) to external agents via ACP, while retaining autonomy over security-sensitive operations (private skills, proprioception).

**Note:** This ADR was originally written in the context of hKask integration. The hKask integration has been removed; Russell is now standalone. The ACP server and its architecture remain valid for integration with any external agent that implements the ACP protocol.

---

## Decision

### 1. Hybrid Deployment Model

Russell will operate as **two complementary components**:

| Component | Deployment | Purpose |
|-----------|------------|---------|
| **russell-sentinel** | systemd timer (5-min cadence) | Host probes, journal writes, proprioception |
| **russell-acp-server** | systemd service (long-running) | ACP session interface, public skill dispatch |

**Rationale:**
- Sentinel retains efficient probe cadence via systemd
- ACP server provides rich multi-turn session interface for external agents
- Separation allows Russell to function standalone when no external agent is connected

### 2. Skill Visibility Boundary

Russell skills are categorized with **hLexicon taxonomy** (WordAct/FlowDef/KnowAct) and **visibility annotation** (public/private):

| Visibility | Count | Skills | Exposure |
|------------|-------|--------|----------|
| **Public** | 8 | journal-compactor, journal-viewer, package-checker, pragmatic-cybernetics, pragmatic-semantics, scenario-tester, ubuntu-jack, web-search | Exposed via ACP to external agents |
| **Private** | 7 | okapi-watcher, skill-discovery, skill-maintenance, skill-manager, sysadmin | Russell-only, never exposed |

**Rationale:**
- Public skills are read-only or informational — no host mutation risk
- Private skills involve sudo-gated operations, skill registry mutations, or local machine configuration
- Visibility is enforced at the ACP dispatch layer — private skills return `SkillNotExposed` error if called via ACP

### 3. Persistence Independence

Russell's SQLite journal **remains independent** — no external data store dependency:

| Aspect | Russell |
|--------|----------|
| **Storage** | SQLite (`~/.local/state/harness/journal.db`) |
| **Schema** | Health telemetry, IDRS evidence, probe results |
| **Access** | ACP agents query via `russell_journal_query` |

**Rationale:**
- Russell operates standalone when no external agent is connected (graceful degradation, ADR-0025)
- Different data models: Russell needs fast writes + hash-chain integrity; external agents may need different query patterns
- Avoids cross-dependency (JR-6: reuse, don't depend)

### 4. Proprioception Privacy

Russell's proprioception vitals **remain private** — never exposed via ACP:

| Vital | Purpose | Visibility |
|-------|---------|------------|
| `sentinel_last_run_age_s` | Sentinel cadence health | Private |
| `journal_writer_stall_s` | Journal write latency | Private |
| `llm_p95_latency_ms` | Nurse pipeline performance | Private |
| `timer_drift_s` | systemd timer accuracy | Private |
| `help_error_rate_pct` | Skill failure rate | Private |

**Rationale:**
- Russell is the attack surface for the local machine
- Proprioception data reveals Russell's operational state — valuable to attackers
- External agents may have their own health monitoring for platform-level observation
- This is a security boundary, not a feature gap

### 5. Authentication: Macaroon-Based OCAP

Russell ACP server **requires macaroon authentication** for all ACP calls:

**Configuration:** `~/.config/harness/macaroon.yaml`

```yaml
issuer:
  root_key: "<random-32-byte-hex>"
  capabilities:
    - name: russell-acp
      attenuations:
        - skill: web-search
        - skill: journal-viewer
        - rate_limit: 100/minute
      before: 24h
```

**Caveats:**
- Time-based expiration (`before: 24h`)
- Rate limiting (`rpm: 100`)
- Skill-specific attenuation (only public skills)
- Third-party discharge for Okapi access (if needed)

**Rationale:**
- Implements OCAP capability discipline (P4)
- Enables fine-grained attenuation per external agent
- Audit trail via macaroon discharge chain
- Loopback-only is insufficient when ACP server accepts connections

### 6. ACP Protocol Version

The ACP protocol version is **`0.1`** (based on the ACP specification).

**Note:** Russell implements the ACP protocol directly. The protocol version should be documented and version negotiation supported at connect time.

### 7. Crate Topology

Add **`russell-acp-server`** crate; retain existing structure:

```
crates/
  russell-acp-server/    # NEW: ACP session interface
  russell-cli/           # Unchanged: local CLI
  russell-core/          # Unchanged: base types, journal
  russell-mcp/           # Extended: MCP client/server
  russell-meta/          # Unchanged: Jack persona
  russell-proprio/       # Unchanged: self-vitals (private)
  russell-sentinel/      # Unchanged: host probes
  russell-skills/        # Extended: visibility enforcement
  russell-testing/       # Extended: ACP test fixtures
```

**Rationale:**
- Minimal changes to existing crates
- Clear separation: ACP server is a new exposure surface, not a refactor
- Skills crate adds visibility filter without changing IDRS contract

---

## Consequences

### Positive

1. **ACP capability access:**
   - External agents gain 8 Russell public skills (host probes, read-only diagnostics)
   - Russell exposes health data and consent flow via ACP

2. **Security boundary maintained:**
   - Private skills (host mutations) never exposed
   - Proprioception vitals remain Russell-internal
   - Macaroon auth enables fine-grained attenuation

3. **Graceful degradation:**
   - Russell operates standalone when no external agent is connected
   - Sentinel continues 5-min probe cadence
   - Journal writes continue; ACP calls fail gracefully

4. **Semantic interoperability:**
   - Skill categorization enables capability routing
   - Public/private visibility boundaries enable safe exposure

### Negative

1. **Increased complexity:**
   - Two deployment units (timer + service)
   - Macaroon key management
   - ACP session state management

2. **Additional attack surface:**
   - ACP server is a new network-exposed component (localhost only)
   - Requires rate limiting, auth validation, input sanitization

3. **Maintenance burden:**
   - ACP protocol version tracking
   - Testing overhead for ACP integration

### Risks

| Risk | Mitigation |
|------|------------|
| **Macaroon key leakage** | Keys stored in `~/.config/harness/` with 0600 permissions; rotate on 24h cadence |
| **ACP server DoS** | Rate limiting (100/min), connection timeouts, semaphore-bounded concurrency |
| **Private skill exposure** | Visibility filter at dispatch layer; unit tests verify rejection |
| **Proprioception leak** | Never added to ACP capability registry; audit trail logs all exposure attempts |
| **External dependency creep** | ADR enforces persistence independence; Russell works standalone |

---

## Compliance

### JR Principles

| Principle | Compliance |
|-----------|------------|
| **JR-1** (Austere by default) | ✅ Minimal crate changes; no feature bloat |
| **JR-2** (Observe > Recommend > Act) | ✅ Public skills are read-only; mutations require local consent |
| **JR-3** (LLM never emits shell) | ✅ ACP dispatch ranks IDs; shell execution remains IDRS-gated |
| **JR-4** (Nurse present) | ✅ Jack persona projected via ACP sessions |
| **JR-5** (Proprioception) | ✅ 5 vitals retained; remain private |
| **JR-6** (Reuse, don't depend) | ✅ Russell journal independent; no external crate dependencies |
| **JR-7** (Persistence auditable) | ✅ ACP calls logged to journal with correlation IDs |

### ADR-0025 (MCP Client Trusted Relationship)

| Constraint | Compliance |
|------------|------------|
| **Loopback-only MCP** | ✅ Extended to macaroon auth for ACP |
| **No cross-dependency** | ✅ Russell crate graph unchanged |
| **Graceful degradation** | ✅ Sentinel operates without external agents |

---

## Implementation Plan

### Phase 0: Foundation (Week 1)
- [ ] Create this ADR (0026)
- [ ] Audit all 16 skills: add hLexicon + visibility metadata
- [ ] Design ACP server interface (methods, types, errors)

### Phase 1: Build ACP Server (Week 2-3)
- [ ] Create `russell-acp-server` crate
- [ ] Implement session manager + turn records
- [ ] Implement visibility filter (public/private enforcement)
- [ ] Implement Jack persona projection
- [ ] Implement macaroon auth validation

### Phase 2: ACP Client Integration (Week 3-4)
- [ ] Extend `russell-mcp` with ACP client methods
- [ ] Configure macaroon keys + caveats
- [ ] Test ACP communication

### Phase 3: Deployment (Week 4-5)
- [ ] Create systemd units (service + socket)
- [ ] Update sentinel timer (add ACP heartbeat)
- [ ] Build + install binary
- [ ] Test graceful degradation (verify sentinel continues without external agent)

### Phase 4: Security Hardening (Week 5-6)
- [ ] Rate limiting (100 calls/min per agent)
- [ ] Audit trail (ACP calls logged to journal)
- [ ] Input sanitization (prevent prompt injection via ACP)
- [ ] Security penetration test

### Phase 5: Testing + Validation (Week 6-7)
- [ ] Unit tests (visibility filter, auth, dispatch)
- [ ] Integration tests (bidirectional ACP + MCP)
- [ ] Security tests (auth bypass, rate limit, DoS)
- [ ] Standalone tests (Russell functions without any external agent)

### Phase 6: Documentation + Rollout (Week 7-8)
- [ ] Update `docs/README.md`, `overview.md`, `AGENTS.md`
- [ ] Write operator guide (`acp-integration.md`)
- [ ] Rollout checklist completion

---

## References

- [ADR-0025: MCP Client Trusted Relationship](0025-hkask-mcp-client-trusted-relationship.md) — **Superseded** (hKask integration removed)
- [ACP Specification](https://agentclientprotocol.com)
- [Speech Act Theory](https://en.wikipedia.org/wiki/Speech_act)
- [Workflow Patterns (van der Aalst)](https://www.workflowpatterns.com/)
- [Enactive Cognition (Varela)](https://en.wikipedia.org/wiki/Enactivism)

---

## Changelog

| Date | Change |
|------|--------|
| 2026-05-22 | Initial draft |
| 2026-06-07 | Updated: removed hKask-specific references; Russell is now standalone. ACP server architecture remains valid for any external agent integration. |

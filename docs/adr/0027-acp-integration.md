# ADR-0027: hKask ACP Integration

**Date:** 2026-05-22  
**Status:** Implemented  
**Author:** Russell Team  
**Deciders:** Operator  
**Technical Story:** [GitHub Issue #XXX](https://github.com/russell/russell/issues/XXX)

---

## Context

Russell operates as a cybernetic health harness for a single Linux AI/ML workstation. hKask is a multi-agent cognitive platform with its own skill registry, MCP tool ecosystem, and ACP (Agent Client Protocol) server implementation.

The design goal is for Russell to operate as a linked system to hKask, where:
- Russell registers as an ACP agent in hKask
- Russell gains access to hKask's MCP tools (193 tools across 16 servers)
- Russell exposes a subset of its capabilities (public skills, host probes) to hKask agents
- Russell retains autonomy over security-sensitive operations (private skills, proprioception)

This ADR documents the architectural decisions for implementing this integration.

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
- ACP server provides rich multi-turn session interface for hKask agents
- Separation allows Russell to function standalone if hKask is unavailable

### 2. Skill Visibility Boundary

Russell skills are categorized with **hLexicon taxonomy** (WordAct/FlowDef/KnowAct) and **visibility annotation** (public/private):

| Visibility | Count | Skills | Exposure |
|------------|-------|--------|----------|
| **Public** | 8 | journal-compactor, journal-viewer, package-checker, pragmatic-cybernetics, pragmatic-semantics, scenario-tester, ubuntu-jack, web-search | Exposed via ACP to hKask agents |
| **Private** | 8 | okapi-watcher, skill-discovery, skill-maintenance, skill-manager, skill-workshop, sysadmin | Russell-only, never exposed |

**Rationale:**
- Public skills are read-only or informational — no host mutation risk
- Private skills involve sudo-gated operations, skill registry mutations, or local machine configuration
- Visibility is enforced at the ACP dispatch layer — private skills return `SkillNotExposed` error if called via ACP

### 3. Persistence Independence

Russell's SQLite journal **remains independent** from hKask's bitemporal store:

| Aspect | Russell | hKask |
|--------|---------|-------|
| **Storage** | SQLite (`~/.local/state/harness/journal.db`) | Bitemporal store (PostgreSQL/Redis) |
| **Schema** | Health telemetry, IDRS evidence, probe results | Cognitive turns, ensemble sessions, agent state |
| **Access** | hKask queries via MCP tool (`russell_journal_query`) | Russell queries via MCP client |
| **A2A Messages** | Cross-agent delegations mirrored to hKask | Native storage |

**Rationale:**
- Russell operates standalone during hKask outages (graceful degradation, ADR-0025)
- Different data models: Russell needs fast writes + hash-chain integrity; hKask needs bitemporal queries
- Avoids cross-dependency (JR-6: reuse, don't depend)

### 4. Proprioception Privacy

Russell's 5 self-vitals **remain private** — never exposed via ACP:

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
- hKask has its own HealthSentinel for platform-level observation
- This is a security boundary, not a feature gap

### 5. Authentication: Macaroon-Based OCAP

Russell ACP server **requires macaroon authentication** for all ACP calls:

**Configuration:** `~/.config/hkask/macaroon.yaml`

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
- Matches hKask's security architecture (OCAP capability tokens)
- Enables fine-grained attenuation per hKask agent
- Audit trail via macaroon discharge chain
- Loopback-only is insufficient for multi-agent environments

### 6. ACP Protocol Version

hKask uses **`acp-runtime = "0.1"`** (external crate dependency).

**Note:** The exact ACP protocol version implemented by this crate is unspecified. Russell will:
- Use hKask's `acp-runtime` crate for compatibility
- Document protocol version once upstream clarifies
- Design for protocol agility (version negotiation at connect time)

### 7. Crate Topology

Add **`russell-acp-server`** crate; retain existing structure:

```
crates/
  russell-acp-server/    # NEW: ACP session interface
  russell-cli/           # Unchanged: local CLI
  russell-core/          # Unchanged: base types, journal
  russell-mcp/           # Extended: hKask MCP client
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

1. **Bidirectional capability access:**
   - Russell gains 193 hKask MCP tools (web search, research, image generation)
   - hKask gains 8 Russell public skills (host probes, read-only diagnostics)

2. **Security boundary maintained:**
   - Private skills (host mutations) never exposed
   - Proprioception vitals remain Russell-internal
   - Macaroon auth enables fine-grained attenuation

3. **Graceful degradation:**
   - Russell operates standalone during hKask outages
   - Sentinel continues 5-min probe cadence
   - Journal writes continue; ACP calls queue or fail gracefully

4. **Semantic interoperability:**
   - hLexicon categorization enables skill routing
   - Shared vocabulary (WordAct/FlowDef/KnowAct) bridges systems

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
   - hKask API changes may require Russell updates
   - Bidirectional testing overhead

### Risks

| Risk | Mitigation |
|------|------------|
| **Macaroon key leakage** | Keys stored in `~/.config/hkask/` with 0600 permissions; rotate on 24h cadence |
| **ACP server DoS** | Rate limiting (100/min), connection timeouts, semaphore-bounded concurrency |
| **Private skill exposure** | Visibility filter at dispatch layer; unit tests verify rejection |
| **Proprioception leak** | Never added to ACP capability registry; audit trail logs all exposure attempts |
| **hKask dependency creep** | ADR enforces persistence independence; Russell works standalone |

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
| **JR-6** (Reuse, don't depend) | ✅ Russell journal independent; no hKask crate dependencies |
| **JR-7** (Persistence auditable) | ✅ ACP calls logged to journal with correlation IDs |

### ADR-0025 (MCP Client Trusted Relationship)

| Constraint | Compliance |
|------------|------------|
| **Loopback-only MCP** | ✅ Extended to macaroon auth for ACP |
| **No cross-dependency** | ✅ Russell crate graph unchanged |
| **Graceful degradation** | ✅ Sentinel operates during hKask outages |

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
- [ ] Register Russell as hKask ACP agent
- [ ] Configure macaroon keys + caveats
- [ ] Test bidirectional communication

### Phase 3: Deployment (Week 4-5)
- [ ] Create systemd units (service + socket)
- [ ] Update sentinel timer (add ACP heartbeat)
- [ ] Build + install binary
- [ ] Test graceful degradation (kill hKask, verify sentinel continues)

### Phase 4: Security Hardening (Week 5-6)
- [ ] Rate limiting (100 calls/min per agent)
- [ ] Audit trail (ACP calls logged to journal)
- [ ] Input sanitization (prevent prompt injection via ACP)
- [ ] Security penetration test

### Phase 5: Testing + Validation (Week 6-7)
- [ ] Unit tests (visibility filter, auth, dispatch)
- [ ] Integration tests (bidirectional ACP + MCP)
- [ ] Security tests (auth bypass, rate limit, DoS)

### Phase 6: Documentation + Rollout (Week 7-8)
- [ ] Update `docs/README.md`, `overview.md`, `AGENTS.md`
- [ ] Write operator guide (`acp-integration.md`)
- [ ] Rollout checklist completion

---

## References

- [ADR-0025: MCP Client Trusted Relationship](0025-hkask-mcp-client-trusted-relationship.md)
- hKask hLexicon — separate repository
- hKask hLexicon Governance — `hKask/registry/registries/hlexicon-governance.yaml` (term allocation model)
- hKask Terminology Change — `hKask/docs/plans/LEXICON_TERMINOLOGY_CHANGE.md` ("budget" → "allocation" for vocabulary)
- hKask ACP Server — separate repository
- hKask Macaroon Config — separate repository
- [ACP Specification](https://agentclientprotocol.com)
- [A2A Protocol](https://a2a-protocol.org)
- [Speech Act Theory](https://en.wikipedia.org/wiki/Speech_act)
- [Workflow Patterns (van der Aalst)](https://www.workflowpatterns.com/)
- [Enactive Cognition (Varela)](https://en.wikipedia.org/wiki/Enactivism)

---

## Changelog

| Date | Change |
|------|--------|
| 2026-05-22 | Initial draft |

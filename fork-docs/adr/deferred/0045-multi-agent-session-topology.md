---
title: "ADR-0045: Multi-Agent Session Topology"
audience: [developers, architects]
last_updated: 2026-05-24
ddmvss_context: "acp"
ddmvss_artifact: "adr"
version: "1.0.0"
status: "Deferred"
---



# ADR-0045: Multi-Agent Session Topology (Deferred)

## Context

Russell's ACP server currently implements a 1:1 session model: one agent creates one session with Russell. However, an orchestrating agent may coordinate multiple agents consulting Russell simultaneously (e.g., a diagnostic agent, a remediation agent, and a monitoring agent all querying Russell's health data).

The adversarial review (2026-05-23) identified this as underspecified:
- Current session model assumes single-agent topology
- No mechanism for session fan-out (one operator, multiple agents)
- No session delegation or sharing between agents
- No conflict resolution if multiple agents propose interventions

---

## Decision

**Deferred.** Multi-agent session topology is not required for MVP and introduces significant complexity:

### Complexity Factors

1. **Session ownership** — Which agent "owns" the session? Can ownership be transferred?
2. **Turn interleaving** — How to handle concurrent messages from multiple agents?
3. **Consent conflicts** — If two agents propose interventions simultaneously, which gets consent priority?
4. **State consistency** — How to ensure all agents see consistent session state?
5. **Token binding** — Current model binds session to creating token; multi-agent requires token federation

### Current Mitigation

The 1:1 model is sufficient for current use cases:
- Agents are orchestrated sequentially, not concurrently
- Each agent creates its own session with Russell
- Sessions are short-lived (minutes, not hours)
- No observed contention or conflicts

### When to Revisit

Revisit this decision when:
- an orchestrating agent demonstrates concurrent multi-agent orchestration
- Session duration increases significantly (hours/days)
- Observed contention or state conflicts in production
- Operator requests multi-agent collaboration features

---

## Consequences

### Positive (of deferral)

- **Simplicity** — 1:1 model is easy to reason about and test
- **No premature optimization** — Avoids complexity not yet required
- **JR-1 compliance** — Austere by default, add complexity only when needed

### Negative (of deferral)

- **Future rework** — Multi-agent support will require significant refactoring
- **Undocumented limitation** — Agents must coordinate externally to avoid conflicts

### Neutral

- **No breaking changes** — Current model continues to work
- **Migration path** — Can add multi-agent support incrementally (session sharing, then fan-out)

---

## Compliance

| Principle | Compliance |
|---|---|
| **JR-1** (Austere by default) | Defer complexity until required |
| **JR-6** (Reuse, don't depend) | Current model reuses simple session manager |

---

## Future Work (if revisited)

1. **Session sharing** — Allow multiple tokens to access the same session
2. **Session fan-out** — One operator session, multiple agent sub-sessions
3. **Turn locking** — Serialize concurrent messages to prevent interleaving
4. **Consent queue** — Queue simultaneous intervention proposals for sequential consent
5. **Agent roles** — Define agent roles (observer, proposer, executor) with different permissions

---

## References

- [ADR-0027: ACP Integration](../0027-acp-integration.md)
- [ADR-0041: ACP Consent Protocol](../0041-acp-consent-protocol.md)
- Adversarial Review Action Plan (2026-05-23) §Tier 3 recommendations

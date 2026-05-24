---
title: "ADR-0046: ACP Protocol Versioning"
audience: [developers, architects]
last_updated: 2026-05-24
togaf_phase: "Requirements Management"
version: "1.0.0"
status: "Deferred"
---

<!-- TOGAF_DOMAIN: Requirements Management — Protocol Compatibility -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Deferred -->
<!-- LAST_UPDATED: 2026-05-24 -->

---
title: "ADR-0046: ACP Protocol Versioning"
audience: [developers, architects]
last_updated: 2026-05-24
togaf_phase: "Requirements Management"
version: "1.0.0"
status: "Deferred"
---

<!-- TOGAF_DOMAIN: Requirements Management — Protocol Compatibility -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Deferred -->
<!-- LAST_UPDATED: 2026-05-24 -->

# ADR-0046: ACP Protocol Versioning (Deferred)

## Decision

**Deferred.** Protocol versioning is not required for MVP because:

### Current Mitigations

1. **Single operator** — Russell and hKask are deployed by the same operator, enabling coordinated updates
2. **JSON-RPC 2.0** — Stable, versioned protocol underneath ACP methods
3. **Method-level errors** — Unknown methods return `InvalidRequest`, allowing graceful handling
4. **Capabilities query** — `acp/capabilities` advertises supported skills and probes

### When to Revisit

Revisit this decision when:
- hKask and Russell are deployed by different operators
- Protocol breaking changes are planned
- Multiple hKask versions must coexist
- Observed compatibility issues in production

---

## Proposed Design (if implemented)

### Version Handshake

```json
// Client sends version in first request
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "acp/version",
  "params": {
    "client_version": "1.0.0",
    "supported_methods": ["acp/session.create", "acp/session.message", ...]
  }
}

// Server responds with negotiated version
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "server_version": "1.0.0",
    "negotiated_version": "1.0.0",
    "supported_methods": ["acp/session.create", ...]
  }
}
```

### Version Compatibility Matrix

| Client | Server | Behavior |
|--------|--------|----------|
| 1.0.0 | 1.0.0 | Full compatibility |
| 1.0.0 | 1.1.0 | Server supports superset, client uses subset |
| 1.1.0 | 1.0.0 | Client requests unsupported methods, receives errors |
| 2.0.0 | 1.0.0 | Breaking change, client must downgrade or server must upgrade |

### Semantic Versioning

- **Major** — Breaking changes (method removal, format changes)
- **Minor** — Additive changes (new methods, optional fields)
- **Patch** — Bug fixes, no API changes

---

## Consequences

### Positive (of deferral)

- **Simplicity** — No version negotiation overhead
- **JR-1 compliance** — Austere by default
- **No premature optimization** — Single-operator deployment makes versioning unnecessary

### Negative (of deferral)

- **Coordinated deployment required** — Russell and hKask must be updated together
- **No graceful degradation** — Protocol mismatches cause hard failures
- **Future rework** — Versioning will require significant refactoring

### Neutral

- **No breaking changes** — Current protocol continues to work
- **Migration path** — Can add versioning incrementally (handshake, then negotiation)

---

## Compliance

| Principle | Compliance |
|---|---|
| **JR-1** (Austere by default) | Defer complexity until required |
| **JR-6** (Reuse, don't depend) | Reuse JSON-RPC 2.0 versioning |

---

## Future Work (if implemented)

1. **Version handshake** — `acp/version` method for negotiation
2. **Capability advertisement** — Extend `acp/capabilities` to include protocol version
3. **Deprecation warnings** — Log warnings when deprecated methods are called
4. **Version-specific handlers** — Route requests to version-appropriate handlers
5. **Protocol documentation** — OpenAPI-style spec for each version

---

## References

- [ADR-0027: hKask ACP Integration](../0027-acp-integration.md)
- [JSON-RPC 2.0 Specification](https://www.jsonrpc.org/specification)
- Adversarial Review Action Plan (2026-05-23) §Tier 3 recommendations

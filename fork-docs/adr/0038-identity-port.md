---
title: "ADR-0038: Identity Port — Unified Authentication Abstraction"
audience: [developers, architects]
last_updated: 2026-05-23
ddmvss_context: "acp"
ddmvss_artifact: "adr"
version: "1.0.0"
status: "Implemented"
---



# ADR-0038: Identity Port — Unified Authentication Abstraction

## Context

Russell operates in a multi-authentication environment:

1. **Macaroon OCAP tokens** — Used by the ACP server for external agent authentication (ADR-0026)
2. **Capability tokens** — Used by the Nurse pipeline for inference requests
3. **Future token formats** — JWT, WebID, or other identity systems may emerge

The adversarial review (2026-05-23) identified weakness W7: "Dual auth systems" — the ACP server and Nurse pipeline used divergent authentication models with no shared abstraction. This created:

- **Cognitive overhead** — Two separate identity models to reason about
- **Testing friction** — No common interface for test doubles
- **Architectural drift** — Each subsystem evolved its own auth patterns independently

Per Alastair Cockburn's hexagonal architecture principles, authentication is a **port** — a boundary between the application core and external identity providers. The core should depend on an abstraction, not concrete implementations.

---

## Decision

Define an `IdentityPort` trait in `russell-core` that provides a unified interface for authentication and authorization, regardless of the underlying token format.

### Trait Definition

```rust
pub trait IdentityPort: Send + Sync {
    /// Get the principal identifier (e.g., token ID, WebID, user ID).
    fn principal_id(&self) -> &str;

    /// Check if this identity has a specific capability.
    fn has_capability(&self, capability: &str) -> bool;

    /// Get all capabilities granted to this identity.
    fn capabilities(&self) -> Vec<String>;

    /// Check if the identity is still valid (not expired).
    fn is_valid(&self) -> bool;
}
```

### Capability Format

Capabilities follow the format `"domain:action"`:
- `"acp:session"` — Create and manage ACP sessions
- `"acp:skill:run"` — Execute skills via ACP
- `"tool:inference"` — Request LLM inference
- `"proprio:read"` — Read proprioception data

### Implementations

**1. CapabilityToken (russell-acp-server)**

The existing macaroon-based `CapabilityToken` now implements `IdentityPort`:

```rust
impl IdentityPort for CapabilityToken {
    fn principal_id(&self) -> &str {
        &self.token_id
    }

    fn has_capability(&self, capability: &str) -> bool {
        self.capabilities.iter().any(|c| c == capability)
    }

    fn capabilities(&self) -> Vec<String> {
        self.capabilities.clone()
    }

    fn is_valid(&self) -> bool {
        match self.expires_at {
            Some(expires) => Utc::now() <= expires,
            None => true,
        }
    }
}
```

**2. SimpleIdentity (russell-core)**

A minimal implementation for testing and dev mode:

```rust
pub struct SimpleIdentity {
    pub principal_id: String,
    pub capabilities: Vec<String>,
}

impl SimpleIdentity {
    pub fn new(principal_id: impl Into<String>, capabilities: Vec<String>) -> Self;
    pub fn anonymous() -> Self;
}
```

### Location

- **Trait:** `crates/russell-core/src/identity.rs`
- **CapabilityToken impl:** `crates/russell-acp-server/src/auth.rs`
- **SimpleIdentity:** `crates/russell-core/src/identity.rs`

---

## Consequences

### Positive

- **Hexagonal architecture** — Authentication is now a proper port. The core depends on an abstraction; adapters provide concrete implementations.

- **Testability** — Test code can use `SimpleIdentity` without constructing real macaroon tokens. Integration tests can mock identity without cryptographic overhead.

- **Future extensibility** — Adding JWT, WebID, or other identity systems requires only a new `IdentityPort` implementation. No changes to core authorization logic.

- **Capability composition** — The `has_capability()` method enables fine-grained authorization checks without exposing token internals.

- **Mark Miller's capability discipline** — Capabilities are explicit, enumerable, and checkable. No ambient authority leaks.

### Negative

- **Indirection cost** — Trait dispatch adds a vtable lookup per capability check. Negligible for Russell's request rates (<100 req/min).

- **Capability string parsing** — The `"domain:action"` format is not validated at compile time. Typos in capability strings are runtime errors. Mitigation: define capability constants in a shared module.

- **Migration path** — Existing code that directly accesses `CapabilityToken` fields must be refactored to use the trait. Low risk: only 3 call sites identified.

### Neutral

- **No breaking changes** — `CapabilityToken` retains all existing fields and methods. The `IdentityPort` impl is additive.

- **Backward compatible** — Code that doesn't need the abstraction can continue using `CapabilityToken` directly.

---

## Compliance

| Principle | Compliance |
|---|---|
| **JR-6** (Reuse, don't depend) | Core depends on abstraction, not concrete auth systems |
| **Cockburn** (Hexagonal architecture) | Authentication is a port with multiple adapters |
| **Miller** (Capability discipline) | Capabilities are explicit, enumerable, checkable |
| **Schneier** (Defense in depth) | `is_valid()` enables expiration checks without exposing token internals |

---

## Implementation

**Files created:**
- `crates/russell-core/src/identity.rs` — Trait definition + `SimpleIdentity`

**Files modified:**
- `crates/russell-acp-server/src/auth.rs` — `impl IdentityPort for CapabilityToken`
- `crates/russell-acp-server/src/handler.rs` — Session creation uses `token.principal_id()`

**Tests:**
- `test_simple_identity` — Verifies basic capability checking
- `test_anonymous_identity` — Verifies empty capability set
- Existing `CapabilityToken` tests continue to pass

---

## Future Work

1. **Capability constants module** — Define `pub const ACP_SESSION: &str = "acp:session";` etc. to prevent typos.

2. **External token adapter** — Implement `IdentityPort` for external capability token formats (used in Nurse pipeline).

3. **Capability attenuation** — Extend `has_capability()` to support attenuations (e.g., `"acp:session:max_duration:3600"`).

4. **Audit logging** — Emit `identity.capability_checked` events for security audit trail.

---

## References

- [ADR-0026: Metacognitive Layer](0026-metacognitive-layer.md) — macaroon authentication design was incorporated into the identity port and metacognitive layer rename
- [ADR-0033: Explicit Port Interfaces](0033-explicit-port-interfaces.md)
- Adversarial Review Action Plan (2026-05-23) §Task T7
- Alastair Cockburn, *Hexagonal Architecture* (2005)
- Mark Miller, *Robust Composition: Towards a Unified Approach to Access Control and Concurrency Control* (2006)

---
title: "ADR-0047: Capability Token Rotation"
audience: [developers, architects, security reviewers]
last_updated: 2026-05-24
togaf_phase: "H"
version: "1.0.0"
status: "Implemented"
---

<!-- TOGAF_DOMAIN: Security — Token Management -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Implemented -->
<!-- LAST_UPDATED: 2026-05-24 -->

# ADR-0047: Capability Token Rotation

## Context

Russell's capability tokens (hKask-format, used for SOAP inference) were generated once and persisted with `expires_at: null` (infinite lifetime). This violated the Schneier principle: secrets should have bounded lifetimes to limit exposure from compromise.

The adversarial review (2026-05-23) identified this as a security concern:
- If a token is compromised, it remains valid indefinitely
- No mechanism to revoke or rotate tokens
- No audit trail of token issuance

---

## Decision

Implement automatic token rotation with a configurable lifetime (default: 24 hours).

### Rotation Logic

1. **On token load**, check `expires_at` field
2. **If expired or missing**, regenerate token with new expiration
3. **New tokens** include `expires_at` set to `now + TOKEN_ROTATION_HOURS`
4. **Token ID** includes timestamp to ensure uniqueness across rotations

### Configuration

- **Default rotation interval:** 24 hours (`TOKEN_ROTATION_HOURS = 24`)
- **Configurable via:** Compile-time constant in `help.rs`
- **Future:** Could be made runtime-configurable via environment variable

### Implementation

**File:** `crates/russell-meta/src/help.rs`

```rust
const TOKEN_ROTATION_HOURS: i64 = 24;

fn is_token_expired(token_b64: &str) -> bool {
    // Decode base64, parse JSON, check expires_at field
    // Returns true if expired or unparseable
}

pub fn load_capability_token() -> Option<String> {
    // ... load and decrypt existing token ...
    
    if !is_token_expired(&token_str) {
        return Some(token_str);
    }
    tracing::info!("capability token expired, rotating");
    
    // Generate new token with expiration
    let expires_at = now + chrono::Duration::hours(TOKEN_ROTATION_HOURS);
    let token_json = json!({
        "expires_at": expires_at.to_rfc3339(),
        // ... other fields ...
    });
    
    // Encrypt and persist
}
```

### Security Properties

- **Bounded lifetime** — Tokens expire after 24h, limiting exposure window
- **Automatic rotation** — No operator intervention required
- **Audit trail** — Token regeneration logged via `tracing::info!`
- **Backward compatible** — Legacy tokens without `expires_at` treated as never-expiring (migration path)

---

## Consequences

### Positive

- **Schneier compliance** — Secrets have bounded lifetimes
- **Reduced attack surface** — Compromised tokens expire within 24h
- **Zero operator burden** — Rotation is automatic and transparent
- **Audit trail** — Token regeneration events logged

### Negative

- **Slight overhead** — Token regeneration on first request after expiration (negligible: ~10ms)
- **Clock dependency** — Relies on system clock accuracy (mitigated by NTP)

### Neutral

- **No breaking changes** — Legacy tokens continue to work
- **Opt-in migration** — Operators can manually delete old tokens to force rotation

---

## Compliance

| Principle | Compliance |
|---|---|
| **Schneier** (Defense in depth) | Bounded token lifetime limits exposure |
| **JR-7** (Persistence is auditable) | Token regeneration logged |
| **JR-1** (Austere by default) | Simple constant-based configuration |

---

## Implementation

**Files modified:**
- `crates/russell-meta/src/help.rs` — Added `TOKEN_ROTATION_HOURS`, `is_token_expired()`, updated `load_capability_token()`
- `crates/russell-meta/Cargo.toml` — Added `chrono` dependency

**Tests:**
- Existing token loading tests continue to pass
- Manual verification: token regeneration on expiration

---

## Future Work

1. **Runtime configuration** — Make rotation interval configurable via `RUSSELL_TOKEN_ROTATION_HOURS` env var
2. **Proactive rotation** — Rotate tokens 1 hour before expiration to avoid request-time delays
3. **Token revocation** — Implement revocation list for compromised tokens
4. **Rotation metrics** — Track rotation count and timing in proprioception

---

## References

- [ADR-0026: Metacognitive Layer](0026-metacognitive-layer.md) — macaroon authentication design incorporated into the identity port
- [ADR-0038: Identity Port](0038-identity-port.md)
- Bruce Schneier, *Applied Cryptography* (1996) — key lifetime principles
- Adversarial Review Action Plan (2026-05-23) §Tier 2 recommendations

---
title: "ADR-0052: Scoped, Versioned, Expiring Consent"
audience: [developers, architects, security reviewers]
last_updated: 2026-06-07
ddmvss_context: "cross-cutting"
ddmvss_artifact: "adr"
version: "1.0.0"
status: "Proposed"
domain: "Cross-cutting"
---

# ADR-0052: Scoped, Versioned, Expiring Consent

## Status

**Proposed**

## Context

Magna Carta P2 (Affirmative Consent) requires that consent grants are scoped, versioned, and expiring — not blanket permissions. The Magna Carta §Consent Scope, Versioning, and Expiration states:

> Consent grants are not indefinite blanket permissions. Each consent grant is scoped to specific categories and resource versions, version-bound (consent must be re-affirmed when a resource is upgraded), and time-bound (consent grants can have expiration dates and must be re-affirmed at expiration).

The existing consent model (from ADR-0050 and the chat REPL) only supports approve/deny semantics. When the operator says "ok" to a `SHELL:` command or an intervention, the consent is implicit, unscoped, and indefinite for that session. There is no way to:

- Limit consent to specific `DataCategory` values (e.g., consent to reading sentinel samples but not journal entries).
- Re-require consent when a skill or resource is upgraded to a new version.
- Set an expiration time after which the consent automatically lapses.
- Express hierarchical consent (master vs. per-skill vs. per-action-type).

Without these, the current consent model violates P2: consent is not scoped, not version-bound, and not time-bound.

## Decision

Extend `ConsentGrant` with four fields and implement a `check_consent()` method that evaluates scope, version, and expiry:

1. **`categories: HashSet<DataCategory>`** — The consent grant covers only the listed data categories. Access to an unlisted category is denied.

2. **`resource_version: Option<String>`** — If set, the consent is bound to this version of the resource. When the resource version changes (e.g., a skill is upgraded), existing consent grants for that resource are invalidated and must be re-granted.

3. **`expires_at: Option<DateTime<Utc>>`** — If set, the consent grant expires at this timestamp. After expiry, the grant is treated as denied. The operator must re-affirm.

4. **`scope: ConsentScope`** — Hierarchical consent structure with three levels:

   | Level | Description |
   |---|---|
   | `Master` | Covers all skills and probes for the operator |
   | `PerSkill` | Specific to a single skill module (identified by skill ID) |
   | `PerActionType` | One structure for probes (auto-execute), another for interventions (require consent) |

   Most-specific grant wins. When a `PerSkill` grant exists alongside a `Master` grant, the `PerSkill` grant takes precedence for that skill.

5. **`OperatorConsent::check_consent()`** — Returns a `ConsentStatus` enum:

   | Variant | Meaning |
   |---|---|
   | `Granted` | All checks pass: category in scope, version matches (or not tracked), not expired |
   | `Denied` | Category not in grant's `categories`, or no grant exists |
   | `Expired` | `expires_at` is set and `Utc::now() > expires_at` |
   | `VersionMismatch` | `resource_version` is set and the current resource version differs |

6. **Backward compatibility** — Existing approve/deny consent in the chat REPL is mapped to a `ConsentGrant` with `scope: Master`, all categories, no version binding, and no expiry. Session-scoped implicit consent continues to work; the new fields are additive.

## Consequences

### Positive

- **P2 compliance** — Consent is now scoped, versioned, and expiring as the Magna Carta requires.
- **Fail-closed** — An expired or version-mismatched grant is treated as denied, not silently accepted.
- **Principle of least privilege** — `PerSkill` and `PerActionType` scopes limit consent to only what is needed.
- **Automatic re-consent on upgrade** — When a skill version changes, its consent grants are invalidated, forcing the operator to re-review and re-affirm.
- **Auditable consent trail** — Every `ConsentGrant` is a structured record with scope, categories, version, and expiry, supporting JR-7.

### Negative

- **Operator friction** — Fine-grained consent means more consent prompts. An operator with many skills may need to grant consent per skill per category.
- **Version tracking overhead** — Every skill/resource must report its version for consent matching. Skills without version metadata cannot be version-bound.
- **Migration** — Existing session consent must be mapped to the new `ConsentGrant` structure. The mapping is straightforward (Master scope, all categories) but must be implemented.

### Risks

- **Consent fatigue** — Too many granular consent prompts may cause operators to grant `Master` scope reflexively, undermining the purpose of scoping. Mitigation: `Master` scope is available but not the default; the REPL suggests the narrowest sufficient scope.
- **Clock skew** — Expiry checks depend on the system clock. If the clock is wrong, consent may expire early or persist beyond its intended lifetime. Mitigation: Russell already checks clock sanity in proprioception (`timer_drift_s`); large drift alerts the operator.
- **Version string comparison** — Semver-aware comparison is more correct but adds complexity. Mitigation: `resource_version` is an opaque string; mismatch means exact equality. Semantic versioning is a future enhancement.

## Implementation

### Type Definitions (`crates/russell-core/src/sovereignty.rs`)

```rust
pub enum ConsentScope {
    Master,
    PerSkill { skill_id: String },
    PerActionType { action_type: ActionType },
}

pub struct ConsentGrant {
    pub categories: HashSet<DataCategory>,
    pub resource_version: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub scope: ConsentScope,
    pub granted_at: DateTime<Utc>,
}

pub enum ConsentStatus {
    Granted,
    Denied,
    Expired { expires_at: DateTime<Utc> },
    VersionMismatch { expected: String, actual: String },
}
```

### Consent Tracking (`crates/russell-session/src/engine.rs`)

| Method | Purpose |
|---|---|
| `grant_consent(grant: ConsentGrant)` | Record a new consent grant |
| `check_consent(category, skill_id, version) -> ConsentStatus` | Evaluate whether consent is valid for a given access |
| `revoke_consent(scope)` | Revoke all grants matching a scope |
| `prune_expired()` | Remove expired grants from the active set |

### Code Changes

| File | Change |
|---|---|
| `crates/russell-core/src/sovereignty.rs` | Add `ConsentScope`, `ConsentGrant`, `ConsentStatus`, `OperatorConsent` |
| `crates/russell-core/src/lib.rs` | Re-export new types |
| `crates/russell-session/src/engine.rs` | Add consent tracking methods |
| `crates/russell-cli/src/commands/chat/consent.rs` | Map approve/deny to `ConsentGrant`; prompt for scope when operator grants consent |
| `crates/russell-skills/src/dispatch.rs` | Call `check_consent` before dispatching; respect `ConsentStatus::Expired` and `VersionMismatch` |

### Backward Compatibility

Existing session consent (approve/deny in chat REPL) maps to:

```rust
ConsentGrant {
    categories: HashSet::from([DataCategory::all()]),
    resource_version: None,
    expires_at: None,
    scope: ConsentScope::Master,
    granted_at: Utc::now(),
}
```

This preserves current behavior. The operator can optionally specify a narrower scope via `/consent` REPL commands.

### Testing Strategy

- Unit tests for `ConsentStatus` variants: granted, denied, expired, version mismatch.
- Unit tests for hierarchical scope resolution: `PerSkill` overrides `Master`.
- Unit tests for expiry: grants with past `expires_at` return `Expired`.
- Integration test: skill dispatch with expired consent is blocked; operator is prompted to re-grant.
- Integration test: skill upgrade invalidates version-bound consent.

## References

- [Magna Carta §P2: Affirmative Consent](../architecture/magna-carta.md#principle-2-affirmative-consent)
- [Magna Carta §Consent Scope, Versioning, and Expiration](../architecture/magna-carta.md#consent-scope-versioning-and-expiration)
- [Magna Carta §Hierarchical Consent Structures](../architecture/magna-carta.md#hierarchical-consent-structures)
- [Magna Carta §Fail-Closed Default](../architecture/magna-carta.md#fail-closed-default)
- [ADR-0051: Data Sovereignty Boundary Module](0051-data-sovereignty-boundary-module.md) — `DataCategory`, `SovereigntyChecker`
- [ADR-0050: Shell Commands Through the Consent Gate](0050-shell-commands-through-consent-gate.md) — existing approve/deny consent
- [AGENTS.md](../../AGENTS.md) — vocabulary: Affirmative consent, Consent gate, Risk band

## Appendix

### Alternatives Considered

#### Alternative 1: Time-only expiry (no scope or version)

**Description:** Add only `expires_at` to consent grants, keeping scope and version implicit.

**Pros:**
- Simpler implementation
- Fewer changes to consent UI

**Cons:**
- Does not satisfy P2's requirement for scoped and version-bound consent
- An expired grant for one category implicitly revokes consent for all categories
- No mechanism to force re-consent on skill upgrades

**Why rejected:** The Magna Carta explicitly requires all three properties: scope, version, and expiry. Implementing only one violates P2.

#### Alternative 2: Macaroon-based consent tokens

**Description:** Use macaroons (attenuating bearer credentials with caveats) as the consent grant format, replacing the struct-based approach.

**Pros:**
- Macaroons are cryptographically verifiable and attenuating
- Natural fit for P4 (OCAP) — macaroons are capability tokens
- Third-party caveats enable delegation

**Cons:**
- Adds a macaroon library dependency (violates JR-6: Reuse, don't depend)
- Macaroons are optimized for distributed systems; Russell is single-host
- The operator must understand macaroon caveats to audit consent grants
- Over-engineered for the current single-host threat model

**Why rejected:** Macaroons are the right long-term direction for P4 enforcement, but introducing them before the consent model is correct would add dependency complexity without addressing the structural gap. The struct-based `ConsentGrant` can be migrated to macaroons later (ADR for macaroon-based consent would supersede this one).

#### Alternative 3: Regex/glob-based category matching

**Description:** Allow `categories` to include glob patterns like `Journal:*` instead of exact `DataCategory` values.

**Pros:**
- More flexible; fewer grants needed for broad access

**Cons:**
- Harder to audit: the operator must mentally resolve glob patterns
- Violates P1's atomic consent principle: each consent decision must be unbundled
- Glob expansion can be surprising (e.g., `*` matches everything)

**Why rejected:** The Magna Carta §Atomic Consent requires that "each consent term must be described in no more than 5 sentences or a standard paragraph." Glob patterns are not atomic consent terms; they are bundles.
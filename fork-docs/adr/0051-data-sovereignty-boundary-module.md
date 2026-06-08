---
title: "ADR-0051: Data Sovereignty Boundary Module"
audience: [developers, architects, security reviewers]
last_updated: 2026-06-07
ddmvss_context: "cross-cutting"
ddmvss_artifact: "adr"
version: "1.0.0"
status: "Proposed"
domain: "Cross-cutting"
---

# ADR-0051: Data Sovereignty Boundary Module

## Status

**Proposed**

## Context

Magna Carta P1 (Operator Sovereignty) and P4 (Clear Boundaries) require that every data access path in Russell is gated by both `require_capability` (OCAP) and `require_sovereignty` (data classification). The Magna Carta specifies a `DataSovereigntyBoundary` struct with `sovereign_data`, `shared_data`, and `public_data` category sets, plus a `requires_affirmative_consent` flag that defaults to `true`.

Currently, Russell has no runtime enforcement of data sovereignty boundaries. The adversarial review (ADR-0031) introduced capability attenuation for environment variables, but there is no per-category data classification gate. A skill or ACP handler can access any data — journal entries, proprioceptive readings, consent records — without checking whether the operator has consented to that category of access.

The Magna Carta P4 dual enforcement gate requires that **no code path can bypass either gate**. Without a `SovereigntyChecker`, the `require_sovereignty` half of the dual gate is unimplemented, meaning P1 sovereignty is enforced only by convention, not by code.

## Decision

Implement a sovereignty module with four core types:

1. **`DataCategory` enum** — Classifies every data resource in Russell into one of three tiers:
   - `Sovereign` — Operator controls; access requires affirmative consent. Default members: journal entries, SOAP bundles, proprioceptive readings, consent records, operator profile.
   - `Shared` — Explicit consent required for external sharing. Default member: sentinel samples (via ACP).
   - `Public` — No sovereignty claim. Default members: hlexicon terms, skill manifests (if published).

2. **`DataSovereigntyBoundary`** — Holds the three `HashSet<DataCategory>` sets and the `requires_affirmative_consent` flag. `russell_default()` sets the flag to `true` (fail-closed).

3. **`SovereigntyChecker`** — Provides `require_sovereignty(category: DataCategory)` as the runtime gate. Every function that reads or writes sovereign data must call this gate. If the category is sovereign and no affirmative consent exists, the call returns a `SovereigntyDenied` error.

4. **`ConsentGate` trait** — Abstracts consent resolution. `DenyAllConsent` is the default implementation — it denies everything until explicitly granted. If the consent port is misconfigured or missing, the system denies all access. Sovereignty fails closed.

The dual gate (OCAP + sovereignty) ensures no code path can bypass either. Skill dispatch checks `require_capability` then `require_sovereignty`. ACP handlers do the same. The order is intentional: capability attenuation is cheaper and rejects unauthorized callers before sovereignty consent is even considered.

## Consequences

### Positive

- **P1 enforcement** — Operator sovereignty is enforced at runtime, not just by convention.
- **P4 compliance** — The `require_sovereignty` half of the dual enforcement gate is implemented, completing the Magna Carta's structural guarantee.
- **Fail-closed default** — `DenyAllConsent` ensures that misconfiguration or missing consent state cannot silently open access to sovereign data.
- **Auditable** — Every sovereignty check produces a structured event, supporting JR-7 (Persistence is auditable).
- **Composable** — `ConsentGate` is a trait; future implementations can integrate operator prompts, macaroons, or ACP consent flows without changing the checker.

### Negative

- **Migration burden** — Every existing function that accesses sovereign data (journal writer, proprioception collector, consent recorder) must be updated to call `require_sovereignty`. This is a broad but mechanical change.
- **Runtime overhead** — Every data access now includes a category lookup and consent check. The overhead is O(1) per access (hash set lookup), but it is nonzero.
- **False negatives** — A developer who forgets to gate a new sovereign data path creates a silent P1 violation. The Magna Carta Verifier skill should catch these, but the verifier itself must be maintained.

### Risks

- **Incomplete gating** — A code path that accesses sovereign data without calling `require_sovereignty` is a P1 violation. Mitigation: the Magna Carta Verifier skill runs a structural audit asserting that all sovereign-data-accessing methods include a `require_sovereignty` call.
- **Over-classification** — Marking too many categories as sovereign degrades usability by requiring consent for routine reads. Mitigation: the default configuration follows the Magna Carta's three-tier list; operators can reclassify via `Profile`.

## Implementation

### Code Changes

| File | Change |
|---|---|
| `crates/russell-core/src/sovereignty.rs` (new) | `DataCategory` enum, `DataSovereigntyBoundary` struct, `SovereigntyChecker`, `ConsentGate` trait, `DenyAllConsent` |
| `crates/russell-core/src/lib.rs` | Re-export `sovereignty` module |
| `crates/russell-skills/src/dispatch.rs` | Call `SovereigntyChecker::require_sovereignty` before dispatching skills that access sovereign data |
| `crates/russell-acp-server/src/handler.rs` | Call `SovereigntyChecker::require_sovereignty` in ACP request handlers that serve sovereign data |
| `crates/russell-journal/src/writer.rs` | Gate journal writes with `require_sovereignty(Journal)` |

### Sovereign Data Categories (Default)

| Category | DataCategory | Description |
|---|---|---|
| Journal entries | `Sovereign` | SQLite journal rows, hash chain |
| SOAP bundles | `Sovereign` | Evidence folders |
| Proprioceptive readings | `Sovereign` | Self-observation vitals |
| Consent records | `Sovereign` | Grants, denials, expiry history |
| Operator profile | `Sovereign` | Preferences, persona settings |
| Sentinel samples | `Shared` | Telemetry shared via ACP with consent |
| hlexicon terms | `Public` | No sovereignty claim |
| Skill manifests | `Public` | If published |

### Testing Strategy

- Unit tests for each `DataCategory` classification.
- Unit tests for `SovereigntyChecker` with `DenyAllConsent`: every sovereign access is denied.
- Integration test: skill dispatch with sovereign data access is blocked without consent.
- Integration test: ACP handler serving sentinel samples with shared consent succeeds.
- Property test: no code path can reach sovereign data without passing through `require_sovereignty`.

## References

- [Magna Carta §P1: Operator Sovereignty](../architecture/magna-carta.md#principle-1-operator-sovereignty)
- [Magna Carta §P4: Clear Boundaries (OCAP)](../architecture/magna-carta.md#principle-4-clear-boundaries-ocap)
- [ADR-0031: Capability Attenuation for Skills](0031-capability-attenuation.md) — OCAP half of the dual gate
- [ADR-0052: Scoped, Versioned, Expiring Consent](0052-scoped-versioned-expiring-consent.md) — consent grant structure
- [AGENTS.md](../../AGENTS.md) — vocabulary: Data Sovereignty Boundary, OCAP, SovereigntyChecker

## Appendix

### Alternatives Considered

#### Alternative 1: Convention-only sovereignty

**Description:** Rely on developer discipline and code review to ensure sovereign data is gated, without a runtime checker.

**Pros:**
- Zero runtime overhead
- No migration burden for existing code

**Cons:**
- Silent P1 violations when developers forget to gate
- No fail-closed guarantee; a missing gate is invisible until audited

**Why rejected:** The Magna Carta requires that P1 be structurally enforced, not merely conventional. P4's dual gate explicitly states "there is no bypass."

#### Alternative 2: Middleware-layer sovereignty

**Description:** Insert a middleware layer in the ACP server and skill dispatcher that intercepts all data accesses and checks sovereignty.

**Pros:**
- Single interception point; no need to annotate individual functions
- Automatic coverage of new code paths

**Cons:**
- Middleware can be bypassed by direct crate access
- Does not cover internal function calls within a crate
- Adds a coupling layer between the data layer and business logic

**Why rejected:** The Magna Carta's structural audit (verifier) requires that each sovereign-data-accessing method explicitly include a `require_sovereignty` call. A middleware layer makes the audit surface implicit rather than explicit.
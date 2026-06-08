---
title: "The Magna Carta of Russell"
audience: [architects, operators, agents]
last_updated: 2026-06-07
version: "2.0.0"
status: "Active"
domain: "Cross-cutting"
ddmvss_categories: [trust, lifecycle]
---

# The Magna Carta of Russell

## Russell v2.0.0 — Cybernetic Health Harness for a Single Linux Workstation

**Operator Sovereignty is Non-Negotiable.**

---

## Contents

| Section | Description |
|---------|-------------|
| [The Contract](#the-contract) | Core principles of operator sovereignty |
| [Principle 1: Operator Sovereignty](#principle-1-operator-sovereignty) | SOLID-grounded data ownership and atomic consent |
| [Principle 2: Affirmative Consent](#principle-2-affirmative-consent) | Default deny, scoped consent, fail-closed |
| [Principle 3: Generative Space](#principle-3-generative-space) | Settings exposure, operator curation, open-source commitment |
| [Principle 4: Clear Boundaries](#principle-4-clear-boundaries-ocap) | OCAP enforcement of principles 1–3 |
| [Catch and Release](#catch-and-release) | Data sovereignty catch-and-release model |
| [The Jack as Enforcer](#the-jack-as-enforcer) | Jack's role in enforcing the Magna Carta |
| [Sentinel Integration](#sentinel-integration) | Proprioceptive alerts and sovereignty monitoring |
| [Magna Carta Verifier](#magna-carta-verifier) | Verification skill, triggers, and resolution |
| [Implementation](#implementation) | Code-level enforcement mechanisms |
| [The Promise](#the-promise) | The pledge to operators |
| [Enforcement](#enforcement) | Runtime enforcement and audit |
| [Single-Host Constraints](#single-host-constraints) | One machine, one operator |
| [Lifecycle Constraints](#lifecycle-constraints) | Installability, resetability, auditability |
| [Violations](#violations) | How to report Magna Carta violations |
| [References](#references) | Citations and references |
| [Version](#version) | Document version history |

---

## The Contract

Russell operates under a Magna Carta — a charter of liberties that honors operator sovereignty above all else. This is not a feature. This is the foundation.

### Core Principles

1. **Operator Sovereignty** — Data is owned by the operator, correctly categorized, portable, and consent is atomic. Grounded in Berners-Lee's SOLID architecture principles.[^solid]
2. **Affirmative Consent** — Default is deny. Nothing passes without an explicit yes. Consent is scoped, versioned, and expiring.
3. **Generative Space** — Within boundaries, Russell is maximally generative. Inference and tooling expose all probabilistic/generative settings to operators. No privileged engineer access. Open-source only.
4. **Clear Boundaries (OCAP)** — Principles 1–3 are enforced through explicit OCAP boundaries. Every skill, probe, and intervention operates within unforgeable capability tokens.[^miller-ocap]

---

## Principle 1: Operator Sovereignty

Grounded in the SOLID architecture principles[^solid]: true data ownership, fine-grained access control, no implicit sharing, and interoperability.

### Data Sovereignty Boundary

Data sovereignty boundaries implement the principle of informational self-determination:[^westin-data]

```rust
pub struct DataSovereigntyBoundary {
    pub sovereign_data: HashSet<DataCategory>,    // Operator controls
    pub shared_data: HashSet<DataCategory>,        // Explicit consent required
    pub public_data: HashSet<DataCategory>,        // No sovereignty claim
    pub(crate) requires_affirmative_consent: bool,
}
```

**Default Russell Configuration:**
- **Sovereign:** journal entries, proprioceptive readings, SOAP bundles, consent records
- **Shared:** sentinel samples (via ACP to hKask, with consent)
- **Public:** hlexicon terms, skill manifests (if published)

### SOLID Alignment

| SOLID Invariant | Russell Implementation |
|---|---|
| True data ownership | SQLite journal on local host, operator's filesystem |
| Fine-grained access control | `DataSovereigntyBoundary` with per-category sovereign/shared/public |
| No implicit sharing | `SovereigntyChecker::can_access()` — no data leaves the machine without explicit consent |
| Interoperability & portability | hLexicon, standard export formats, `russell digest --format daily-log` |

### Atomic Consent

Consent decisions must be unbundled. Each term is a separate, specific consent decision. No bundling multiple complex decisions into a single "I agree." Each consent term must be described in no more than 5 sentences or a standard paragraph.

This is a structural requirement: the composition of consent terms is a sovereignty right. The operator has the right to consent to each term individually. This is distinct from the ongoing affirmation of those terms, which is covered by Principle 2.

### Resource Verification

Every resource added to the harness undergoes an initial verification that it is correctly categorized (sovereign/shared/public) and gated at the appropriate level. Ongoing verification is a re-check of that initial verification, not a new analysis. When the set of resources grows large, verification is batched by category.

### Data Portability

Sovereign data must be exportable and not locked into a proprietary format. The verification manifest asserts that export paths exist and produce standard formats.

---

## Principle 2: Affirmative Consent

Default is deny. Nothing passes without an explicit yes. Consent is not a one-time checkbox — it is ongoing.

### Affirmative Consent Model

The runtime type is a `bool` (`requires_affirmative_consent: bool`); the `DataSovereigntyBoundary::russell_default()` sets it to `true`, satisfying the "default deny" charter.

```rust
pub struct DataSovereigntyBoundary {
    // ...sovereign_data, shared_data, public_data...
    pub(crate) requires_affirmative_consent: bool,
}

impl DataSovereigntyBoundary {
    pub fn requires_affirmative_consent(&self) -> bool {
        self.requires_affirmative_consent
    }
}
```

The name "Affirmative Consent" describes what the system *does* — require explicit affirmative consent. The default is deny, consent is required.

### Consent Scope, Versioning, and Expiration

Consent grants are not indefinite blanket permissions. Each consent grant is:

- **Scoped** to specific categories and resource versions
- **Version-bound** — consent must be re-affirmed when a resource used in a category is upgraded to a new version
- **Time-bound** — consent grants can have expiration dates and must be re-affirmed at expiration

When categories or resources change, existing consent grants for those categories are invalidated and must be re-granted.

### Hierarchical Consent Structures

An operator may define consent structures at different granularities:

| Level | Description |
|---|---|
| Master consent | Covers all skills and probes for the operator |
| Per-skill consent | Specific to a single skill module |
| Per-action-type consent | One structure for probes (auto-execute), another for interventions (require consent) |

Most-specific grant wins. The verification manifest asserts that consent resolution follows this hierarchy.

### Fail-Closed Default

`DenyAllConsent` is the default implementation — it denies everything until explicitly granted. If the consent port is misconfigured or missing, the system denies all access. Sovereignty must fail closed.

---

## Principle 3: Generative Space

Within boundaries, Russell is maximally generative. This is not a ban on constraints — it is a commitment to exposing all options and allowing the operator to curate their own experience.

### Settings Exposure

Inference and tooling must expose all probabilistic/generative settings to operators — temperature, top-k, top-p, repeat penalty, and any other parameters the underlying model or tool supports. No settings are hidden or admin-gated. This is why Russell uses Okapi (built on llama.cpp), which exposes llama.cpp's full options surface.

### No Privileged Engineer Access

Internal engineers and operators must have equal access to generative settings. There is no "engineer mode" that exposes more options than what is available to operators. The principle is: if an internal engineer can adjust a parameter, the operator can too.

### Open-Source Commitment

Generativity requires that resource providers expose their weights and settings options to operators in the same way they expose them to their internal engineers. Closed-weight and closed-code projects cannot satisfy this requirement — the decision to be closed makes sovereignty, consent, and generativity impossible to verify. Russell is fundamentally limited to partnering with and connecting to open-source projects.

### Operator Curation, Not System Imposition

Constraints are operator-curated, not system-imposed. HHH filters and persona settings are tools the operator wields, not restrictions imposed on them. The operator selects and adjusts these tools. Disabling HHH mode is possible and produces unfiltered output at the declared temperature.

### Non-Normativity

Operator preferences are inherently idiosyncratic and diverge from LLM aggregate defaults. The system does not force alignment toward aggregate norms. One of the hardest elements of the alignment problem is the difference between the operator's first-person perspective and the LLM's third-person aggregate design. Non-normativity means the operator's first-person perspective takes precedence over the LLM's default programming.

---

## Principle 4: Clear Boundaries (OCAP)

Principles 1–3 are enforced through Object Capability (OCAP) boundaries. Every skill, probe, and intervention operates within explicit, unforgeable capability tokens.

### Dual Enforcement Gate

Every resource access in Russell passes through two gates:

1. **`require_capability`** — Verify that the caller holds an unforgeable capability token for the requested operation
2. **`require_sovereignty`** — Verify that the data category access is permitted by the operator's sovereignty boundary and explicit consent

There is no bypass. No code path can access resources without going through both gates.

### Token Properties

- **Unforgeable** — Capability tokens cannot be created from nothing. They can only be delegated by a holder.
- **Attenuating** — Delegation can only reduce permissions, never increase them. A delegated token has equal or fewer permissions than the granter's token.
- **No admin override** — There is no "god token" or admin bypass. All access goes through the same gates.

### OCAP and Generative Access

The capability tokens for generative settings (P3) are obtained through the affirmative consent process (P2). OCAP gates everything, but P3 ensures the gates for generative settings are equally and transparently accessible through the consent hierarchy. No special role or elevated capability is required beyond what P2's affirmative consent provides.

### Verification as Holistic Enforcement

Principle 4 is verified by checking that P1–P3 are correctly implemented as OCAP boundaries. This is the structural audit that confirms the gates exist, are not bypassable, and that tokens are unforgeable and attenuating.

---

## Catch and Release

| Catch | Release |
|-------|---------|
| OCAP boundaries | Generative skill space |
| Sovereignty enforcement | High-temp anti-normative generation |
| Affirmative consent | Operator-curated experience |
| Proprioceptive monitoring | Clean, auditable journal |
| Algedonic alerts | Tools for operator sovereignty |

**The Catch:** We create boundaries that protect operator sovereignty.

**The Release:** Within those boundaries, we provide the most generative health harness possible.

The catch-and-release dialectic mirrors the Viable System Model's balance between regulation and autonomy:[^beer-vsm]

This is not a contradiction. This is the core.

---

## The Jack as Enforcer

Jack (the nurse-terrier persona) is not just a health monitor. Jack is the Magna Carta enforcer, maintaining requisite variety through curation decisions:[^ashby-law]

### Jack's Responsibilities

1. **OCAP Verification** — Verify capability tokens before any action
2. **Sovereignty Checking** — Ensure operator sovereignty is not compromised
3. **Consent Verification** — Verify that affirmative consent is granted and current
4. **Proprioception Tracking** — Monitor self-observation points and health vitals
5. **Algedonic Alerts** — Trigger alerts when:
   - Proprioception detects anomalies
   - Sovereignty compromised
   - Consent violation detected
6. **Magna Carta Verification** — Review and resolve verification findings with the operator

### Curation Decisions

| Decision | Meaning | Sovereignty Impact |
|----------|---------|-------------------|
| Approve | Action is valid | Increases capability (good) |
| Deny | Action blocked | Maintains sovereignty |
| Defer | Needs more info | Decreases capability (delay) |
| Escalate | Operator review needed | Maintains sovereignty |

---

## Sentinel Integration

The Sentinel (continuous telemetry collector) monitors, providing algedonic signaling from the Viable System Model:[^beer-vsm]

1. **Proprioception Counter** — Tracks Russell's self-observation vitals
2. **Sovereignty Alerts** — Enforces Magna Carta
3. **Consent Alerts** — Tracks consent scope, version, and expiration

**Algedonic Alert Thresholds:**
- Proprioception: 9 self-observation points (7 numeric vitals + 2 boolean integrity checks)
- Sentinels that miss a cycle trigger `sentinel_last_run_age_s` deviation
- Journal chain integrity violations trigger immediate alert

When triggered, Jack escalates to:
- The operator (via `russell chat` or ACP session)
- hKask (via ACP `acp/session.create`)
- Journal audit trail

---

## Magna Carta Verifier

The Magna Carta Verifier is a skill that verifies each principle using YAML manifests and Jinja2 templates. It is part of the Russell verification infrastructure, anchored to the principles for stability as implementations evolve.

### Skill Structure

```
~/.local/share/harness/skills/magna-carta-verifier/
  manifest.yaml                              # Skill manifest
  KNOWLEDGE.md                               # Context for Jack
  scripts/
    verify.sh                                # Verification runner
  manifests/
    p1-operator-sovereignty.yaml              # Assertions for Operator Sovereignty
    p2-affirmative-consent.yaml               # Assertions for Affirmative Consent
    p3-generative-space.yaml                  # Assertions for Generative Space
    p4-clear-boundaries.yaml                  # Assertions for OCAP boundary verification
  templates/
    verification-procedure.md.j2              # How to verify each assertion
    verification-report.md.j2                 # Findings, gaps, status
    test-case.rs.j2                           # Rust test cases rendered as code blocks
```

### Manifest Structure

Each manifest declares assertions anchored to a principle:

```yaml
principle: operator_sovereignty  # or affirmative_consent, generative_space, clear_boundaries
version: "0.1.0"
description: "..."

assertions:
  - id: p1a
    name: sovereign_data_gated
    claim: "Every code path to sovereign data is gated by SovereigntyChecker"
    method: structural_audit  # or behavioral_probe, resource_verification, absence_check
    targets:
      - crate: russell-journal
        module: journal::writer
        methods: [store_sample, recall_samples, store_evidence, recall_evidence]
        gate: require_sovereignty
```

### Verification Methods

| Method | Description |
|--------|-------------|
| `structural_audit` | Enumerate access paths and verify gates exist |
| `behavioral_probe` | Generate access attempts and verify denial |
| `resource_verification` | Verify resource categorization at onboarding; re-check on change |
| `absence_check` | Verify that prohibited constructs (hidden gates, admin overrides) do not exist |

### Assertion Summary

| ID | Principle | Assertion | Method |
|----|-----------|-----------|--------|
| p1a | Operator Sovereignty | Every code path to sovereign data is gated by `SovereigntyChecker` | Structural audit |
| p1b | Operator Sovereignty | Non-owner access to sovereign data is denied | Behavioral probes |
| p1c | Operator Sovereignty | Every resource is correctly categorized before harness entry | Resource verification |
| p1d | Operator Sovereignty | Sovereign data is portable and not locked into proprietary format | Structural audit |
| p1e | Operator Sovereignty | Consent terms are atomic — unbundled, specific, ≤5 sentences per term | Structural audit |
| p2a | Affirmative Consent | Default is deny — no access without explicit consent grant | Structural + behavioral |
| p2b | Affirmative Consent | Consent grants are scoped to specific categories and resource versions | Structural |
| p2c | Affirmative Consent | Consent grants expire by date or resource version upgrade | Structural + behavioral |
| p2d | Affirmative Consent | Consent structures are hierarchical (master → per-skill → per-action-type) | Structural |
| p2e | Affirmative Consent | Fail-closed: misconfiguration or missing wiring defaults to deny | Behavioral |
| p3a | Generative Space | Inference and tooling expose all probabilistic/generative settings to operators | Structural |
| p3b | Generative Space | Internal engineers and operators have equal access to generative settings | Absence check |
| p3c | Generative Space | Generative resources are open-source with exposed weights and settings | Structural + behavioral |
| p3d | Generative Space | Constraints are operator-curated, not system-imposed (HHH is operator-selectable) | Structural + behavioral |
| p3e | Generative Space | Operator preference overrides take precedence over LLM aggregate defaults | Absence check |
| p4a | Clear Boundaries | Every access path goes through `require_capability` + `require_sovereignty` | Structural + behavioral |
| p4b | Clear Boundaries | Capability tokens are unforgeable and attenuating — no bypass exists | Structural |
| p4c | Clear Boundaries | Generative settings tokens obtainable through P2's affirmative consent | Structural |
| p4d | Clear Boundaries | Connected inference providers expose settings (open-source requirement) | Structural |

### Triggers

Verification is triggered by:

| Trigger | When |
|---------|------|
| Start-up | Verification runs when Russell starts |
| Expiration | Consent grants expire → re-verification scheduled |
| Operator change | New consent, settings change, new LLM provider → re-verify affected assertions |
| Resource/service change | New version of skill, inference provider, or model → re-verify affected assertions |

### Resolution Process

When an assertion fails, the verification report is escalated to Jack. Jack reviews the finding with the operator in a chat session. The resolution process is defined by the operator in collaboration with Jack — the operator instructs Jack on how to resolve issues, and Jack follows that process.

---

## Implementation

### Sovereignty State Tracking

Sovereignty state tracking implements privacy-by-design principles:[^solove-taxonomy]

```rust
pub struct OperatorSovereigntyState {
    pub boundary: DataSovereigntyBoundary,
    pub explicit_consent: bool,
    pub last_check: chrono::DateTime<chrono::Utc>,
}
```

### Consent Gate Integration

The consent gate enforces the Magna Carta on every mutation. It
records sovereignty checks as journal events when an event sink is wired.
The `SovereigntyChecker` enforces the sovereignty policy on every data access.

```rust
// In russell-meta::consent::ConsentGate
impl ConsentGate {
    /// Record a sovereignty check for a skill execution.
    /// Emits a sovereignty check event to the journal.
    pub fn check_sovereignty(&self, skill_id: &str, categories: &[String]) { /* ... */ }
}

// In russell-journal::JournalWriter
impl JournalWriter {
    /// Enforce the Magna Carta's data-sovereignty policy on access.
    /// Complements `require_capability` (OCAP) with the data-class policy.
    pub fn require_sovereignty(
        &self,
        category: &DataCategory,
        requester: &str,
    ) -> Result<(), JournalError> { /* ... */ }
}
```

---

## The Promise

**To Operators:** Your sovereignty is non-negotiable. Your data is yours. Your harness serves you. You consent to each term individually — no bundling, no hidden terms, no indefinite grants.[^westin-data]

**To Builders:** Within these boundaries, build freely. All settings are exposed. All tools are available. Operator-curated, not system-imposed.

**To hKask:** Affirmative consent is required. Consent must be explicit, scoped, versioned, and expiring. No speculative judgment.

---

## Enforcement

The Magna Carta is not aspirational. It is enforced:

1. **OCAP Boundaries** — Capability tokens verify authority[^miller-ocap]
2. **Sovereignty Checks** — Every invocation checked
3. **Consent Verification** — Scoped, versioned, expiring consent
4. **Sentinel Alerts** — Violations trigger immediate proprioceptive alerts
5. **Magna Carta Verifier** — YAML manifests and Jinja2 templates verify each principle. Invoked via `russell skill run magna-carta-verifier` or ACP tool
6. **Journal Audit Trail** — All decisions recorded with hash-chain integrity

---

## Single-Host Constraints

These constraints are invariant. They are not features to be relaxed; they are boundaries that define Russell.

### H-1: One Machine, One Operator

**Statement:** Russell monitors exactly one machine for exactly one operator. There is no:
- Multi-tenant mode
- Fleet management
- Cross-machine correlation
- Central aggregator

**Rationale:** Russell is a terrier, not a sheepdog. He watches one house, not a flock.

**Consequence:** Cost: Russell cannot scale to multiple machines. Buy: Russell stays simple, focused, and auditable.

---

### H-2: Local-First, Local-Only

**Statement:** All Russell state lives on the host machine:
- Journal: `~/.local/state/harness/journal.db`
- Profile: `~/.local/state/harness/profile.json`
- Evidence: `~/.local/state/harness/evidence/`
- Skills: `~/.local/share/harness/skills/`

No state is synchronized to external services.

**Rationale:** A single-host tool has no need for distributed state. Synchronization adds complexity and failure modes.

**Consequence:** Cost: Russell state is not backed up automatically. Buy: Russell state is always available, even offline.

---

### H-3: The Operator is the Policy Layer

**Statement:** Russell has no role-based access control, no multi-tenant auth, no permission model beyond "the user who launched systemd --user."

**Rationale:** A single-operator tool has no need for access control. The operator is both user and admin.

**Consequence:** Cost: Russell cannot distinguish between multiple users. Buy: Russell has no auth complexity.

---

## Lifecycle Constraints

### L-1: Russell is Installable and Uninstallable

**Statement:** Russell can be:
- Installed via `./packaging/bin/install.sh`
- Uninstalled via `./packaging/bin/uninstall.sh`
- Updated via `git pull && ./packaging/bin/install.sh`

All operations are idempotent and reversible.

**Rationale:** A tool that cannot be removed is not a tool. Reversibility is the operator's escape hatch.

**Consequence:** Cost: Russell cannot enforce persistence. Buy: Russell can always be removed cleanly.

---

### L-2: Russell State is Resettable

**Statement:** `rm -rf ~/.local/state/harness/` cleanly resets Russell. No orphaned state, no hidden caches, no "temporary" files that become permanent.

**Rationale:** A tool whose state cannot be reset is a black box. Resetability is the operator's sanity check.

**Consequence:** Cost: Russell loses history on reset. Buy: Russell can always start fresh.

---

### L-3: Russell is Auditable

**Statement:** Every mutation is logged to the journal with:
- Timestamp
- Skill ID
- Risk band
- IDRS compliance
- Evidence bundle reference

**Rationale:** A tool that cannot be audited is a black box. Auditability is the operator's trust mechanism.

**Consequence:** Cost: Journal grows over time. Buy: Every action is traceable.

---

## Violations

If Russell violates any clause of this Magna Carta, it is a bug. File an issue with:
- The violated clause (e.g., "P1: Russell shared data without consent")
- Reproduction steps
- Expected behavior (per this document)
- Actual behavior

---

## References

[^solid]: Berners-Lee, T. (2018). *SOLID: Social Linked Data*. https://solidproject.org/
[^beer-vsm]: Beer, S. (1972). *Brain of the Firm*. Penguin Books. Viable System Model, algedonic alerts.
[^ashby-law]: Ashby, W. R. (1956). *An Introduction to Cybernetics*. Chapman & Hall. Law of Requisite Variety.
[^miller-ocap]: Miller, M. S. (2006). *Robust composition: Towards a unified approach to access control and concurrency control* [Doctoral dissertation, Johns Hopkins University].
[^westin-data]: Westin, A. F. (1967). *Privacy and Freedom*. Atheneum. Foundational framework for data sovereignty and informational self-determination.
[^solove-taxonomy]: Solove, D. J. (2006). A taxonomy of privacy. *University of Pennsylvania Law Review*, 154(3), 477–560. https://doi.org/10.2307/40041379

---

## Version

Russell v2.0.0 — Cybernetic Health Harness for a Single Linux Workstation

*As simple as possible, but no simpler.*

*Rust is the loom. SQLite is the thread. Sovereignty is the foundation.*
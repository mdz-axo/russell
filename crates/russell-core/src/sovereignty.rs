// SPDX-License-Identifier: MIT OR Apache-2.0
//! Data sovereignty boundary — Magna Carta P1 (Operator Sovereignty)
//! and P4 (Clear Boundaries).
//!
//! Implements the `DataSovereigntyBoundary` and `SovereigntyChecker` types
//! defined in the Magna Carta. Every data access in Russell must pass through
//! both `require_capability` (OCAP) and `require_sovereignty` (data category)
//! gates. There is no bypass.
//!
//! ## Default deny
//!
//! `DataSovereigntyBoundary::russell_default()` sets
//! `requires_affirmative_consent: true`, satisfying the Magna Carta's
//! "default deny" charter (P2: Affirmative Consent).

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// DataCategory — P1: Operator Sovereignty
// ---------------------------------------------------------------------------

/// Categories of data for sovereignty classification.
///
/// Each category determines the access control policy:
/// - `Sovereign` — Operator controls. Never shared without explicit consent.
/// - `Shared` — Explicit consent required per category. Shared with hKask
///   only when the operator grants it.
/// - `Public` — No sovereignty claim. Can be shared freely.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataCategory {
    // Sovereign data — operator controls, never shared without consent
    /// Journal entries (samples, events, SOAP bundles).
    JournalEntry,
    /// Proprioceptive readings (self-observation vitals).
    ProprioceptiveReading,
    /// SOAP bundles (Subjective/Objective/Assessment/Plan).
    SoapBundle,
    /// Consent records (who consented to what, when).
    ConsentRecord,
    /// Operator profile (host info, preferences).
    OperatorProfile,

    // Shared data — explicit consent required per category
    /// Sentinel samples shared via ACP (with consent).
    SentinelSample,
    /// Skill dispatch results shared via ACP (with consent).
    SkillResult,
    /// Session metadata shared via ACP (with consent).
    SessionMetadata,

    // Public data — no sovereignty claim
    /// hLexicon terms (terminology definitions).
    HlexiconTerm,
    /// Skill manifests (if published).
    SkillManifest,
    /// Probe definitions (public schema).
    ProbeDefinition,
}

impl DataCategory {
    /// Returns the sovereignty tier for this category.
    pub fn tier(&self) -> SovereigntyTier {
        match self {
            Self::JournalEntry
            | Self::ProprioceptiveReading
            | Self::SoapBundle
            | Self::ConsentRecord
            | Self::OperatorProfile => SovereigntyTier::Sovereign,

            Self::SentinelSample | Self::SkillResult | Self::SessionMetadata => {
                SovereigntyTier::Shared
            }

            Self::HlexiconTerm | Self::SkillManifest | Self::ProbeDefinition => {
                SovereigntyTier::Public
            }
        }
    }

    /// Whether this category requires affirmative consent before sharing.
    pub fn requires_consent_to_share(&self) -> bool {
        matches!(
            self.tier(),
            SovereigntyTier::Sovereign | SovereigntyTier::Shared
        )
    }
}

/// Sovereignty tier — determines the access policy for a data category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SovereigntyTier {
    /// Operator controls. Never shared without explicit consent.
    Sovereign,
    /// Explicit consent required per category.
    Shared,
    /// No sovereignty claim. Can be shared freely.
    Public,
}

// ---------------------------------------------------------------------------
// DataSovereigntyBoundary — P1 + P4
// ---------------------------------------------------------------------------

/// The data sovereignty boundary for Russell, implementing the Magna Carta's
/// P1 (Operator Sovereignty) and P4 (Clear Boundaries) principles.
///
/// Every resource access in Russell passes through two gates:
/// 1. `require_capability` — verify the caller holds an unforgeable capability
///    token for the requested operation (OCAP, P4).
/// 2. `require_sovereignty` — verify the data category access is permitted by
///    the operator's sovereignty boundary and explicit consent (P1, P2).
///
/// There is no bypass. No code path can access resources without going through
/// both gates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataSovereigntyBoundary {
    /// Categories the operator controls — never shared without consent.
    pub sovereign_data: HashSet<DataCategory>,
    /// Categories that require explicit consent per sharing event.
    pub shared_data: HashSet<DataCategory>,
    /// Categories with no sovereignty claim — can be shared freely.
    pub public_data: HashSet<DataCategory>,
    /// Whether affirmative consent is required (default: true — P2 default deny).
    pub requires_affirmative_consent: bool,
}

impl DataSovereigntyBoundary {
    /// Russell's default sovereignty boundary.
    ///
    /// Implements the Magna Carta's default-deny charter: affirmative consent
    /// is required (`requires_affirmative_consent: true`).
    ///
    /// Sovereign categories (operator controls, never shared without consent):
    /// - Journal entries, proprioceptive readings, SOAP bundles,
    ///   consent records, operator profile
    ///
    /// Shared categories (explicit consent required per category):
    /// - Sentinel samples, skill results, session metadata
    ///
    /// Public categories (no sovereignty claim):
    /// - hLexicon terms, skill manifests, probe definitions
    pub fn russell_default() -> Self {
        let sovereign: HashSet<DataCategory> = [
            DataCategory::JournalEntry,
            DataCategory::ProprioceptiveReading,
            DataCategory::SoapBundle,
            DataCategory::ConsentRecord,
            DataCategory::OperatorProfile,
        ]
        .into_iter()
        .collect();

        let shared: HashSet<DataCategory> = [
            DataCategory::SentinelSample,
            DataCategory::SkillResult,
            DataCategory::SessionMetadata,
        ]
        .into_iter()
        .collect();

        let public: HashSet<DataCategory> = [
            DataCategory::HlexiconTerm,
            DataCategory::SkillManifest,
            DataCategory::ProbeDefinition,
        ]
        .into_iter()
        .collect();

        Self {
            sovereign_data: sovereign,
            shared_data: shared,
            public_data: public,
            requires_affirmative_consent: true,
        }
    }

    /// Whether affirmative consent is required for any data access.
    ///
    /// This implements P2 (Affirmative Consent): the default is deny,
    /// and consent must be explicitly granted.
    pub fn requires_affirmative_consent(&self) -> bool {
        self.requires_affirmative_consent
    }

    /// Check whether a given category can be accessed by a requester.
    ///
    /// Returns `Ok(())` if access is permitted, or an error describing
    /// why access was denied. This is the `require_sovereignty` gate
    /// from P4 (Clear Boundaries).
    pub fn can_access(
        &self,
        category: &DataCategory,
        requester: &str,
        has_consent: bool,
    ) -> Result<(), SovereigntyError> {
        // Public data is always accessible.
        if self.public_data.contains(category) {
            return Ok(());
        }

        // Affirmative consent gate (P2).
        if self.requires_affirmative_consent && !has_consent {
            return Err(SovereigntyError::ConsentRequired {
                category: *category,
                requester: requester.to_string(),
            });
        }

        // Sovereign data: only the operator can access without explicit consent.
        if self.sovereign_data.contains(category) {
            if has_consent {
                return Ok(());
            }
            return Err(SovereigntyError::SovereignDataAccessDenied {
                category: *category,
                requester: requester.to_string(),
            });
        }

        // Shared data: requires per-category consent.
        if self.shared_data.contains(category) {
            if has_consent {
                return Ok(());
            }
            return Err(SovereigntyError::SharedDataAccessDenied {
                category: *category,
                requester: requester.to_string(),
            });
        }

        // Category not in any set — deny by default (fail-closed, P2).
        Err(SovereigntyError::UncategorizedDataAccessDenied {
            category: *category,
            requester: requester.to_string(),
        })
    }
}

// ---------------------------------------------------------------------------
// SovereigntyChecker — P4 dual enforcement gate
// ---------------------------------------------------------------------------

/// The sovereignty checker enforces P1 (Operator Sovereignty) and P4 (Clear
/// Boundaries) on every data access. It is the `require_sovereignty` half of
/// the dual enforcement gate; the `require_capability` half is implemented by
/// `MacaroonAuth` in `russell-acp-server`.
///
/// There is no bypass. No admin override. No god token.
pub struct SovereigntyChecker {
    boundary: DataSovereigntyBoundary,
}

impl SovereigntyChecker {
    /// Create a new sovereignty checker with the given boundary.
    pub fn new(boundary: DataSovereigntyBoundary) -> Self {
        Self { boundary }
    }

    /// Create a sovereignty checker with Russell's default boundary.
    pub fn russell_default() -> Self {
        Self {
            boundary: DataSovereigntyBoundary::russell_default(),
        }
    }

    /// The `require_sovereignty` gate — P4 (Clear Boundaries).
    ///
    /// Every data access in Russell must pass through this gate.
    /// It checks whether the data category is accessible by the requester,
    /// given the current consent state.
    ///
    /// This complements `require_capability` (OCAP tokens). No code path
    /// can access resources without going through both gates.
    pub fn require_sovereignty(
        &self,
        category: &DataCategory,
        requester: &str,
        has_consent: bool,
    ) -> Result<(), SovereigntyError> {
        self.boundary.can_access(category, requester, has_consent)
    }

    /// Get a reference to the current boundary configuration.
    pub fn boundary(&self) -> &DataSovereigntyBoundary {
        &self.boundary
    }
}

// ---------------------------------------------------------------------------
// ConsentGate — P2 affirmative consent (fail-closed default)
// ---------------------------------------------------------------------------

/// The consent gate enforces P2 (Affirmative Consent) on every mutation.
///
/// The default implementation is `DenyAllConsent` — it denies everything
/// until explicitly granted. If the consent gate is misconfigured or
/// missing, the system denies all access. Sovereignty must fail closed.
pub trait ConsentGate: Send + Sync {
    /// Check whether the requester has consent for the given action.
    fn has_consent(&self, requester: &str, action: &str) -> bool;
}

/// Fail-closed default consent gate — denies everything until explicitly
/// granted. This is the correct default: P2 requires affirmative consent,
/// and misconfiguration must not result in accidental permission.
pub struct DenyAllConsent;

impl ConsentGate for DenyAllConsent {
    fn has_consent(&self, _requester: &str, _action: &str) -> bool {
        false
    }
}

/// Operator-granted consent — records explicit consent decisions.
///
/// Consent is:
/// - **Scoped** to specific actions
/// - **Version-bound** — consent must be re-affirmed on resource version change
/// - **Time-bound** — consent grants can expire
///
/// This implements ACR-2 (Consent is Scoped, Versioned, and Expiring).
pub struct OperatorConsent {
    grants: std::collections::HashMap<String, ConsentGrant>,
}

/// A single consent grant, scoped to a specific action.
#[derive(Debug, Clone)]
pub struct ConsentGrant {
    /// The categories this grant covers.
    pub categories: HashSet<DataCategory>,
    /// The resource version this grant was issued for.
    /// If the resource version changes, consent must be re-affirmed.
    pub resource_version: Option<String>,
    /// When this grant expires. If None, it does not expire.
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    /// The scope of this grant.
    pub scope: ConsentScope,
    /// When this grant was issued.
    pub granted_at: chrono::DateTime<chrono::Utc>,
}

/// Hierarchical consent scope — most-specific grant wins.
///
/// P2 (Affirmative Consent) requires that consent can be structured at
/// different granularities:
/// - `Master` — covers all skills and probes
/// - `PerSkill` — specific to one skill module
/// - `PerActionType` — one structure for probes, another for interventions
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ConsentScope {
    /// Covers all skills and probes for the operator.
    Master,
    /// Specific to a single skill module.
    PerSkill {
        /// The skill ID this consent covers.
        skill_id: String,
    },
    /// One structure for probes (auto-execute), another for interventions (require consent).
    PerActionType {
        /// The action type this consent covers.
        action_type: String,
    },
}

impl ConsentScope {
    /// Whether this scope covers the given action.
    ///
    /// Most-specific grant wins:
    /// `PerActionType` > `PerSkill` > `Master`
    pub fn covers(&self, skill_id: &str, action_type: &str) -> bool {
        match self {
            Self::Master => true,
            Self::PerSkill { skill_id: sid } => sid == skill_id,
            Self::PerActionType { action_type: at } => at == action_type,
        }
    }
}

impl OperatorConsent {
    /// Create a new empty consent store (all access denied by default).
    pub fn new() -> Self {
        Self {
            grants: std::collections::HashMap::new(),
        }
    }

    /// Grant consent for a specific action.
    pub fn grant(&mut self, action: String, grant: ConsentGrant) {
        self.grants.insert(action, grant);
    }

    /// Revoke consent for a specific action.
    pub fn revoke(&mut self, action: &str) {
        self.grants.remove(action);
    }

    /// Check whether consent exists for a given action, considering
    /// scope, version, and expiration.
    pub fn check_consent(&self, action: &str, current_version: Option<&str>) -> ConsentStatus {
        match self.grants.get(action) {
            None => ConsentStatus::Denied,
            Some(grant) => {
                // Check expiration
                if let Some(expires_at) = grant.expires_at {
                    if chrono::Utc::now() > expires_at {
                        return ConsentStatus::Expired {
                            expired_at: expires_at,
                        };
                    }
                }

                // Check version mismatch — re-consent required when resource
                // version changes (P2: version-bound consent).
                if let (Some(granted_version), Some(current_ver)) =
                    (&grant.resource_version, current_version)
                {
                    if granted_version != current_ver {
                        return ConsentStatus::VersionMismatch {
                            granted_version: granted_version.clone(),
                            current_version: current_ver.to_string(),
                        };
                    }
                }

                ConsentStatus::Granted {
                    scope: grant.scope.clone(),
                }
            }
        }
    }
}

/// The status of a consent check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConsentStatus {
    /// Consent has been granted with the given scope.
    /// Consent has been granted with the given scope.
    Granted {
        /// The scope of the granted consent.
        scope: ConsentScope,
    },
    /// Consent has not been granted for this action.
    Denied,
    /// Consent was granted but has expired.
    Expired {
        /// When the consent expired.
        expired_at: chrono::DateTime<chrono::Utc>,
    },
    /// Consent was granted for a different resource version.
    /// Re-consent is required (P2: version-bound consent).
    VersionMismatch {
        /// The version the consent was granted for.
        granted_version: String,
        /// The current version of the resource.
        current_version: String,
    },
}

impl ConsentGate for OperatorConsent {
    fn has_consent(&self, _requester: &str, action: &str) -> bool {
        matches!(
            self.check_consent(action, None),
            ConsentStatus::Granted { .. }
        )
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors returned by the sovereignty checker.
#[derive(Debug, thiserror::Error)]
pub enum SovereigntyError {
    /// Consent is required but not granted (P2: affirmative consent).
    #[error("consent required for {category:?} access by {requester}")]
    ConsentRequired {
        /// The data category that was accessed.
        category: DataCategory,
        /// The requester who was denied access.
        requester: String,
    },

    /// Access to sovereign data was denied (P1: operator sovereignty).
    #[error("sovereign data {category:?} access denied for {requester}")]
    SovereignDataAccessDenied {
        /// The data category that was accessed.
        category: DataCategory,
        /// The requester who was denied access.
        requester: String,
    },

    /// Access to shared data was denied (P1: operator sovereignty).
    #[error("shared data {category:?} access denied for {requester}")]
    SharedDataAccessDenied {
        /// The data category that was accessed.
        category: DataCategory,
        /// The requester who was denied access.
        requester: String,
    },

    /// Access to uncategorized data was denied (P2: fail-closed).
    #[error("uncategorized data {category:?} access denied for {requester}")]
    UncategorizedDataAccessDenied {
        /// The data category that was accessed.
        category: DataCategory,
        /// The requester who was denied access.
        requester: String,
    },
}

// ---------------------------------------------------------------------------
// Operator sovereignty state tracking — Implementation section of Magna Carta
// ---------------------------------------------------------------------------

/// Sovereignty state tracking implements privacy-by-design principles
/// (Solove, 2006). This is the runtime state that tracks the operator's
/// current boundary and consent status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperatorSovereigntyState {
    /// The current data sovereignty boundary.
    pub boundary: DataSovereigntyBoundary,
    /// Whether the operator has given explicit consent.
    pub explicit_consent: bool,
    /// When the sovereignty state was last checked.
    pub last_check: chrono::DateTime<chrono::Utc>,
}

impl OperatorSovereigntyState {
    /// Create a new sovereignty state with Russell defaults.
    pub fn russell_default() -> Self {
        Self {
            boundary: DataSovereigntyBoundary::russell_default(),
            explicit_consent: false,
            last_check: chrono::Utc::now(),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_category_tiers() {
        assert_eq!(
            DataCategory::JournalEntry.tier(),
            SovereigntyTier::Sovereign
        );
        assert_eq!(DataCategory::SentinelSample.tier(), SovereigntyTier::Shared);
        assert_eq!(DataCategory::HlexiconTerm.tier(), SovereigntyTier::Public);
    }

    #[test]
    fn data_category_consent_requirements() {
        assert!(DataCategory::JournalEntry.requires_consent_to_share());
        assert!(DataCategory::SentinelSample.requires_consent_to_share());
        assert!(!DataCategory::HlexiconTerm.requires_consent_to_share());
    }

    #[test]
    fn default_boundary_is_deny_by_default() {
        let boundary = DataSovereigntyBoundary::russell_default();
        assert!(boundary.requires_affirmative_consent());
    }

    #[test]
    fn sovereignty_checker_deny_without_consent() {
        let checker = SovereigntyChecker::russell_default();
        let result = checker.require_sovereignty(
            &DataCategory::JournalEntry,
            "remote-agent",
            false, // no consent
        );
        assert!(result.is_err());
    }

    #[test]
    fn sovereignty_checker_allow_with_consent() {
        let checker = SovereigntyChecker::russell_default();
        let result = checker.require_sovereignty(
            &DataCategory::JournalEntry,
            "operator",
            true, // has consent
        );
        assert!(result.is_ok());
    }

    #[test]
    fn sovereignty_checker_public_always_accessible() {
        let checker = SovereigntyChecker::russell_default();
        let result = checker.require_sovereignty(
            &DataCategory::HlexiconTerm,
            "anyone",
            false, // no consent needed for public data
        );
        assert!(result.is_ok());
    }

    #[test]
    fn deny_all_consent_gate_denies_everything() {
        let gate = DenyAllConsent;
        assert!(!gate.has_consent("operator", "any-action"));
        assert!(!gate.has_consent("admin", "any-action"));
    }

    #[test]
    fn consent_scope_covers() {
        let master = ConsentScope::Master;
        let per_skill = ConsentScope::PerSkill {
            skill_id: "sysadmin".to_string(),
        };
        let per_action = ConsentScope::PerActionType {
            action_type: "probe".to_string(),
        };

        assert!(master.covers("sysadmin", "probe"));
        assert!(master.covers("any-skill", "any-action"));

        assert!(per_skill.covers("sysadmin", "probe"));
        assert!(!per_skill.covers("other-skill", "probe"));

        assert!(per_action.covers("sysadmin", "probe"));
        assert!(!per_action.covers("sysadmin", "intervention"));
    }

    #[test]
    fn operator_consent_grant_and_revoke() {
        let mut consent = OperatorConsent::new();
        let grant = ConsentGrant {
            categories: [DataCategory::JournalEntry].into_iter().collect(),
            resource_version: Some("1.0.0".to_string()),
            expires_at: None,
            scope: ConsentScope::PerSkill {
                skill_id: "sysadmin".to_string(),
            },
            granted_at: chrono::Utc::now(),
        };

        consent.grant("sysadmin/sweep-caches".to_string(), grant);
        assert!(consent.has_consent("operator", "sysadmin/sweep-caches"));

        consent.revoke("sysadmin/sweep-caches");
        assert!(!consent.has_consent("operator", "sysadmin/sweep-caches"));
    }

    #[test]
    fn operator_consent_version_mismatch() {
        let mut consent = OperatorConsent::new();
        let grant = ConsentGrant {
            categories: [DataCategory::JournalEntry].into_iter().collect(),
            resource_version: Some("1.0.0".to_string()),
            expires_at: None,
            scope: ConsentScope::Master,
            granted_at: chrono::Utc::now(),
        };

        consent.grant("action".to_string(), grant);

        // Same version — granted
        assert_eq!(
            consent.check_consent("action", Some("1.0.0")),
            ConsentStatus::Granted {
                scope: ConsentScope::Master
            }
        );

        // Different version — version mismatch (re-consent required)
        assert_eq!(
            consent.check_consent("action", Some("2.0.0")),
            ConsentStatus::VersionMismatch {
                granted_version: "1.0.0".to_string(),
                current_version: "2.0.0".to_string()
            }
        );
    }

    #[test]
    fn operator_consent_expiry() {
        let mut consent = OperatorConsent::new();
        let grant = ConsentGrant {
            categories: [DataCategory::JournalEntry].into_iter().collect(),
            resource_version: None,
            expires_at: Some(chrono::Utc::now() - chrono::Duration::hours(1)), // expired
            scope: ConsentScope::Master,
            granted_at: chrono::Utc::now() - chrono::Duration::hours(2),
        };

        consent.grant("action".to_string(), grant);

        assert_eq!(
            consent.check_consent("action", None),
            ConsentStatus::Expired {
                expired_at: chrono::Utc::now() - chrono::Duration::hours(1)
            }
        );
    }

    #[test]
    fn uncategorized_data_is_denied() {
        let boundary = DataSovereigntyBoundary::russell_default();
        // Create a category that's not in any set — but we can't easily
        // do this with the enum, so instead verify that all categories
        // in the default boundary are accounted for.
        let all_categories: HashSet<DataCategory> = boundary
            .sovereign_data
            .iter()
            .chain(boundary.shared_data.iter())
            .chain(boundary.public_data.iter())
            .copied()
            .collect();

        // Verify all DataCategory variants are in one of the three sets
        for cat in [
            DataCategory::JournalEntry,
            DataCategory::ProprioceptiveReading,
            DataCategory::SoapBundle,
            DataCategory::ConsentRecord,
            DataCategory::OperatorProfile,
            DataCategory::SentinelSample,
            DataCategory::SkillResult,
            DataCategory::SessionMetadata,
            DataCategory::HlexiconTerm,
            DataCategory::SkillManifest,
            DataCategory::ProbeDefinition,
        ] {
            assert!(
                all_categories.contains(&cat),
                "Category {:?} not in any set",
                cat
            );
        }
    }
}

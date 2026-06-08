// SPDX-License-Identifier: MIT OR Apache-2.0
//! Skill metadata types for ACP exposure.

use serde::{Deserialize, Serialize};

/// Visibility annotation (from skill manifests).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    /// Exposed via ACP to agents.
    Public,
    /// Russell-only (never exposed).
    Private,
}

/// hLexicon domain (from skill manifests).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LexiconDomain {
    /// Language for prompting/LLM interactions (speech act theory).
    WordAct,
    /// Language for process/skill composition (workflow patterns).
    FlowDef,
    /// Language for cognition and metacognition (enactive cognition).
    KnowAct,
}

/// hLexicon categorization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LexiconCategorization {
    /// Primary domain.
    pub primary: LexiconDomain,
    /// Specific terms (3-7 from hLexicon).
    pub terms: Vec<String>,
}

/// Safety information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyInfo {
    /// Maximum auto-execution risk level.
    pub max_auto_risk: String,
    /// Interventions requiring explicit human consent.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub require_human_for: Vec<String>,
}

/// Probe information (public metadata).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeInfo {
    /// Probe ID.
    pub id: String,
    /// Human-readable description.
    pub description: String,
    /// Execution timeout.
    pub timeout: String,
}

/// Intervention information (public metadata).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterventionInfo {
    /// Intervention ID.
    pub id: String,
    /// Human-readable description.
    pub description: String,
    /// Risk level.
    pub risk: String,
    /// Requires sudo.
    pub needs_sudo: bool,
    /// Rollback information.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rollback: Option<String>,
}

/// Public skill metadata (exposed via ACP).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillInfo {
    /// Skill ID.
    pub id: String,
    /// Version.
    pub version: String,
    /// Human-readable description.
    pub description: String,
    /// Visibility (always `Public` for ACP-exposed skills).
    pub visibility: Visibility,
    /// hLexicon categorization.
    pub lexicon: LexiconCategorization,
    /// Symptoms this skill addresses.
    pub symptoms: Vec<String>,
    /// Probe metadata.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub probes: Vec<ProbeInfo>,
    /// Intervention metadata.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub interventions: Vec<InterventionInfo>,
    /// Safety constraints.
    pub safety: SafetyInfo,
}

/// Capabilities response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilitiesResponse {
    /// Public skills.
    pub skills: Vec<SkillInfo>,
    /// Host probes.
    pub probes: Vec<ProbeInfo>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // REQ: Visibility enum serializes as lowercase.
    #[test]
    fn visibility_lowercase_round_trip() {
        for (vis, expected) in [
            (Visibility::Public, "public"),
            (Visibility::Private, "private"),
        ] {
            let json = serde_json::to_string(&vis).unwrap();
            assert_eq!(json.trim_matches('"'), expected);
            let back: Visibility = serde_json::from_str(&json).unwrap();
            assert_eq!(back, vis);
        }
    }

    // REQ: LexiconDomain serializes as lowercase (note: "lowercase" not "snake_case").
    #[test]
    fn lexicon_domain_round_trip() {
        let domains = vec![
            (LexiconDomain::WordAct, "wordact"),
            (LexiconDomain::FlowDef, "flowdef"),
            (LexiconDomain::KnowAct, "knowact"),
        ];
        for (domain, expected) in domains {
            let json = serde_json::to_string(&domain).unwrap();
            assert_eq!(json.trim_matches('"'), expected);
            let back: LexiconDomain = serde_json::from_str(&json).unwrap();
            assert_eq!(back, domain);
        }
    }

    // REQ: SkillInfo must round-trip through serialization.
    #[test]
    fn skill_info_round_trip() {
        let skill = SkillInfo {
            id: "disk-check".to_string(),
            version: "1.0.0".to_string(),
            description: "Check disk health".to_string(),
            visibility: Visibility::Public,
            lexicon: LexiconCategorization {
                primary: LexiconDomain::WordAct,
                terms: vec!["assess".to_string(), "probe".to_string()],
            },
            symptoms: vec!["disk_io_pressure".to_string()],
            probes: vec![ProbeInfo {
                id: "disk-usage".to_string(),
                description: "Root filesystem %".to_string(),
                timeout: "5s".to_string(),
            }],
            interventions: vec![],
            safety: SafetyInfo {
                max_auto_risk: "none".to_string(),
                require_human_for: vec![],
            },
        };
        let json = serde_json::to_string(&skill).unwrap();
        let back: SkillInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, "disk-check");
        assert_eq!(back.version, "1.0.0");
        assert_eq!(back.visibility, Visibility::Public);
        assert_eq!(back.lexicon.primary, LexiconDomain::WordAct);
        assert_eq!(back.probes.len(), 1);
        assert!(back.interventions.is_empty());
    }

    // REQ: InterventionInfo with optional rollback must round-trip.
    #[test]
    fn intervention_info_round_trip() {
        let iv = InterventionInfo {
            id: "clean-journal".to_string(),
            description: "Compact journal".to_string(),
            risk: "low".to_string(),
            needs_sudo: false,
            rollback: Some("cp journal.db journal.db.bak".to_string()),
        };
        let json = serde_json::to_string(&iv).unwrap();
        let back: InterventionInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, "clean-journal");
        assert!(!back.needs_sudo);
        assert_eq!(back.rollback.unwrap(), "cp journal.db journal.db.bak");
    }

    // REQ: CapabilitiesResponse must round-trip.
    #[test]
    fn capabilities_response_round_trip() {
        let resp = CapabilitiesResponse {
            skills: vec![],
            probes: vec![ProbeInfo {
                id: "cpu-temp".to_string(),
                description: "CPU temperature".to_string(),
                timeout: "3s".to_string(),
            }],
        };
        let json = serde_json::to_string(&resp).unwrap();
        let back: CapabilitiesResponse = serde_json::from_str(&json).unwrap();
        assert!(back.skills.is_empty());
        assert_eq!(back.probes.len(), 1);
        assert_eq!(back.probes[0].id, "cpu-temp");
    }

    // REQ: SkillInfo skips empty probes/interventions from wire.
    #[test]
    fn skill_info_skips_empty_collections() {
        let skill = SkillInfo {
            id: "minimal".to_string(),
            version: "0.1.0".to_string(),
            description: "Minimal skill".to_string(),
            visibility: Visibility::Public,
            lexicon: LexiconCategorization {
                primary: LexiconDomain::FlowDef,
                terms: vec!["compose".to_string()],
            },
            symptoms: vec![],
            probes: vec![],
            interventions: vec![],
            safety: SafetyInfo {
                max_auto_risk: "none".to_string(),
                require_human_for: vec![],
            },
        };
        let json = serde_json::to_string(&skill).unwrap();
        // Empty Vecs with skip_serializing_if should be omitted
        assert!(!json.contains("probes"));
        assert!(!json.contains("interventions"));
        // But must still deserialize correctly
        let back: SkillInfo = serde_json::from_str(&json).unwrap();
        assert!(back.probes.is_empty());
        assert!(back.interventions.is_empty());
    }

    // REQ: Missing required field on SkillInfo causes deserialization error.
    #[test]
    fn skill_info_missing_id_fails() {
        let json = r#"{"version":"1","description":"x","visibility":"public","lexicon":{"primary":"word_act","terms":[]},"symptoms":[],"safety":{"max_auto_risk":"none"}}"#;
        let result = serde_json::from_str::<SkillInfo>(json);
        assert!(result.is_err());
    }
}

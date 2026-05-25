// SPDX-License-Identifier: MIT OR Apache-2.0
//! Skill metadata types for ACP exposure.

use serde::{Deserialize, Serialize};

/// Visibility annotation (from skill manifests).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    /// Exposed via ACP to hKask agents.
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

// SPDX-License-Identifier: MIT OR Apache-2.0
//! ACP-specific type definitions.
//!
//! Session, consent, and turn types live in `russell-session` and are
//! re-exported from this crate for backward compatibility. This module
//! contains only ACP-transport-specific types (JSON-RPC, skill metadata,
//! notifications).

use serde::{Deserialize, Serialize};

use russell_core::risk::RiskBand;

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
    pub max_auto_risk: RiskBand,
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
    pub risk: RiskBand,
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

/// Proprioception notification — pushed to hKask agents when Russell detects
/// degradation in its own health (T2-2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProprioNotification {
    /// Notification ID (UUID v4).
    pub id: String,
    /// Vital that breached threshold (e.g., "hkask_mcp_reachable_ms").
    pub vital: String,
    /// Severity ("warn", "alert", "critical").
    pub severity: String,
    /// Current value of the vital.
    pub value: serde_json::Value,
    /// Threshold that was breached.
    pub threshold: serde_json::Value,
    /// Human-readable summary.
    pub summary: String,
    /// Timestamp (ISO 8601).
    pub timestamp: String,
}

/// Notifications list response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationsResponse {
    /// Recent proprioception notifications.
    pub notifications: Vec<ProprioNotification>,
    /// Total count of notifications in the time window.
    pub total: usize,
}

/// Capabilities response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilitiesResponse {
    /// Public skills.
    pub skills: Vec<SkillInfo>,
    /// Host probes.
    pub probes: Vec<ProbeInfo>,
}

/// JSON-RPC request envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    /// JSON-RPC version ("2.0").
    pub jsonrpc: String,
    /// Request ID.
    pub id: serde_json::Value,
    /// Method name.
    pub method: String,
    /// Parameters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
    /// Authentication (macaroon token).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<AuthInfo>,
    /// ACP protocol version (for hKask interop).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acp_version: Option<String>,
}

impl JsonRpcRequest {
    /// ACP protocol version constant.
    pub const ACP_VERSION: &'static str = "0.1.0";
}

/// Authentication info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthInfo {
    /// Auth type ("macaroon").
    pub auth_type: String,
    /// Token (base64-encoded macaroon).
    pub token: String,
}

/// JSON-RPC response envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    /// JSON-RPC version ("2.0").
    pub jsonrpc: String,
    /// Request ID (echoed).
    pub id: serde_json::Value,
    /// Result (if success).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// Error (if failure).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// JSON-RPC error.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    /// Error code.
    pub code: i32,
    /// Error message.
    pub message: String,
    /// Additional data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl JsonRpcError {
    /// Create a new JSON-RPC error.
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    /// Create a new JSON-RPC error with data.
    pub fn with_data(code: i32, message: impl Into<String>, data: serde_json::Value) -> Self {
        Self {
            code,
            message: message.into(),
            data: Some(data),
        }
    }
}

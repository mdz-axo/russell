// SPDX-License-Identifier: MIT OR Apache-2.0
//! ACP type definitions.

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

/// Risk level (from skill manifests).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
    /// No risk (read-only probes).
    None,
    /// Low risk (reversible, no data loss).
    Low,
    /// Medium risk (requires operator consent).
    Medium,
    /// High risk (potentially destructive).
    High,
    /// Critical risk (system-affecting).
    Critical,
}

/// Safety information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyInfo {
    /// Maximum auto-execution risk level.
    pub max_auto_risk: RiskLevel,
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
    pub risk: RiskLevel,
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

/// Session creation request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSessionRequest {
    /// Optional persona name (default: "jack").
    #[serde(default = "default_persona")]
    pub persona: String,
}

fn default_persona() -> String {
    "jack".to_string()
}

/// Session creation response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSessionResponse {
    /// Session ID (UUID v4).
    pub session_id: String,
    /// Creation timestamp (ISO 8601).
    pub created_at: String,
    /// Persona name.
    pub persona: String,
}

/// Session message request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMessageRequest {
    /// Session ID.
    pub session_id: String,
    /// Message content.
    pub message: String,
    /// Correlation ID for tracing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
}

/// Session message response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMessageResponse {
    /// Session ID.
    pub session_id: String,
    /// Jack's response.
    pub response: String,
    /// Turn history.
    pub turns: Vec<TurnInfo>,
    /// Session state.
    pub state: String,
    /// Pending action (if consent required).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_action: Option<PendingAction>,
}

/// Turn information (summary for response).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnInfo {
    /// Turn ID.
    pub id: String,
    /// Role (user/assistant/tool).
    pub role: String,
    /// Content.
    pub content: String,
    /// Timestamp (ISO 8601).
    pub timestamp: String,
    /// Tool calls (if any).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCallSummary>,
}

/// Tool call summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallSummary {
    /// Skill ID.
    pub skill_id: String,
    /// Probe ID (if probe).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub probe_id: Option<String>,
    /// Intervention ID (if intervention).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intervention_id: Option<String>,
    /// Arguments.
    pub args: serde_json::Value,
    /// Result.
    pub result: String,
}

/// Pending action (consent required).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingAction {
    /// Action type (intervention/probe).
    pub action_type: String,
    /// Skill ID.
    pub skill_id: String,
    /// Intervention ID.
    pub intervention_id: String,
    /// Risk level.
    pub risk: RiskLevel,
    /// Requires operator consent.
    pub requires_consent: bool,
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

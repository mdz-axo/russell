// SPDX-License-Identifier: MIT OR Apache-2.0
//! Session request/response types — surface-agnostic.
//!
//! These types define the wire format for session operations. Each surface
//! (CLI, API, ACP) adapts these into its own transport format.

use serde::{Deserialize, Serialize};

use crate::session::{ConsentDecision, PendingAction};

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

/// Consent request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsentRequest {
    /// Session ID.
    pub session_id: String,
    /// Action ID (from PendingAction).
    pub action_id: String,
    /// Consent decision (approve/deny).
    pub decision: ConsentDecision,
    /// Optional reason for decision.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Consent response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsentResponse {
    /// Session ID.
    pub session_id: String,
    /// Action ID.
    pub action_id: String,
    /// Decision recorded.
    pub decision: ConsentDecision,
    /// Execution result (if approved and executed).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    /// Error (if approved but execution failed).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

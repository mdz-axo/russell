// SPDX-License-Identifier: MIT OR Apache-2.0
//! MCP protocol types — minimal subset for client operations.
//!
//! We vendor only what Russell needs (JR-6: reuse, don't depend).
//! This is NOT a full MCP SDK — just enough to speak the protocol
//! for `initialize`, `tools/list`, `tools/call`, and `ping`.

use serde::{Deserialize, Serialize};

// ── JSON-RPC 2.0 wire types ────────────────────────────────────────

/// JSON-RPC 2.0 request.
#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcRequest {
    /// Always "2.0".
    pub jsonrpc: &'static str,
    /// Request ID (monotonically increasing integer).
    pub id: u64,
    /// Method name.
    pub method: String,
    /// Parameters (may be empty object).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

impl JsonRpcRequest {
    /// Construct a new request with the given method and params.
    pub fn new(id: u64, method: impl Into<String>, params: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            method: method.into(),
            params,
        }
    }
}

/// JSON-RPC 2.0 notification (no id, no response expected).
#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcNotification {
    /// Always "2.0".
    pub jsonrpc: &'static str,
    /// Method name.
    pub method: String,
    /// Parameters.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

/// JSON-RPC 2.0 response (success or error).
#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcResponse {
    /// Response ID (matches request).
    pub id: Option<u64>,
    /// Successful result.
    pub result: Option<serde_json::Value>,
    /// Error result.
    pub error: Option<JsonRpcErrorObject>,
}

/// JSON-RPC 2.0 error object.
#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcErrorObject {
    /// Error code.
    pub code: i64,
    /// Error message.
    pub message: String,
    /// Additional error data.
    pub data: Option<serde_json::Value>,
}

// ── MCP protocol types ─────────────────────────────────────────────

/// MCP protocol version we speak.
pub const PROTOCOL_VERSION: &str = "2024-11-05";

/// Server info from the `initialize` response.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfo {
    /// Server name.
    pub name: Option<String>,
    /// Server version.
    pub version: Option<String>,
}

/// Capabilities from the `initialize` response.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerCapabilities {
    /// Tools capability (presence means tools are supported).
    pub tools: Option<ToolsCapability>,
}

/// Tools capability metadata.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolsCapability {
    /// Whether the server sends `notifications/tools/list_changed`.
    pub list_changed: Option<bool>,
}

/// Initialize response result.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    /// Negotiated protocol version.
    pub protocol_version: Option<String>,
    /// Server info.
    pub server_info: Option<ServerInfo>,
    /// Server capabilities.
    pub capabilities: Option<ServerCapabilities>,
}

/// A single MCP tool definition from `tools/list`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpToolDefinition {
    /// Tool name (the callable ID).
    pub name: String,
    /// Human-readable description.
    #[serde(default)]
    pub description: Option<String>,
    /// JSON Schema for the tool's input parameters.
    #[serde(default)]
    pub input_schema: Option<serde_json::Value>,
    /// Optional annotations (risk band, metadata).
    #[serde(default)]
    pub annotations: Option<serde_json::Value>,
}

/// Response from `tools/list`.
#[derive(Debug, Clone, Deserialize)]
pub struct ToolsListResult {
    /// List of available tools.
    pub tools: Vec<McpToolDefinition>,
}

/// Parameters for `tools/call`.
#[derive(Debug, Clone, Serialize)]
pub struct ToolCallParams {
    /// Tool name to invoke.
    pub name: String,
    /// Arguments matching the tool's input schema.
    #[serde(default)]
    pub arguments: Option<serde_json::Value>,
}

/// Content item in a tool call result.
#[derive(Debug, Clone, Deserialize)]
pub struct ToolContent {
    /// Content type ("text", "image", "resource").
    #[serde(rename = "type")]
    pub content_type: String,
    /// Text content (if type == "text").
    #[serde(default)]
    pub text: Option<String>,
    /// Additional data.
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Result of a `tools/call` invocation.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallResult {
    /// Content items returned by the tool.
    #[serde(default)]
    pub content: Vec<ToolContent>,
    /// Whether the tool execution encountered an error.
    #[serde(default)]
    pub is_error: bool,
}

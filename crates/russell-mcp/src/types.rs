// SPDX-License-Identifier: MIT OR Apache-2.0
//! Kask REST API types — minimal subset for client operations.
//!
//! We vendor only what Russell needs (JR-6: reuse, don't depend).
//! This is NOT a full Kask SDK — just enough to call the REST API
//! for `tools/list` and `tools/call`.

use serde::{Deserialize, Serialize};

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
    /// Server name that provides this tool (Kask REST API extension).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server: Option<String>,
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

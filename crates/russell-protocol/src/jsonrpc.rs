// SPDX-License-Identifier: MIT OR Apache-2.0
//! JSON-RPC 2.0 request and response envelopes.

use serde::{Deserialize, Serialize};

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
    /// Create a new JSON-RPC request.
    pub fn new(id: serde_json::Value, method: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            method: method.into(),
            params: None,
            auth: None,
            acp_version: None,
        }
    }
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

impl JsonRpcResponse {
    /// Create a success response.
    pub fn ok(id: serde_json::Value, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Create an error response.
    pub fn error(id: serde_json::Value, error: JsonRpcError) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(error),
        }
    }
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

    /// Standard JSON-RPC parse error (-32700).
    pub fn parse_error() -> Self {
        Self::new(-32700, "Parse error")
    }

    /// Standard JSON-RPC invalid request (-32600).
    pub fn invalid_request() -> Self {
        Self::new(-32600, "Invalid Request")
    }

    /// Standard JSON-RPC method not found (-32601).
    pub fn method_not_found() -> Self {
        Self::new(-32601, "Method not found")
    }

    /// Standard JSON-RPC invalid params (-32602).
    pub fn invalid_params() -> Self {
        Self::new(-32602, "Invalid params")
    }

    /// Standard JSON-RPC internal error (-32603).
    pub fn internal_error() -> Self {
        Self::new(-32603, "Internal error")
    }

    /// Authentication required (-32001).
    pub fn auth_required() -> Self {
        Self::new(-32001, "Authentication required")
    }

    /// Token expired (-32002).
    pub fn token_expired() -> Self {
        Self::new(-32002, "Token expired")
    }

    /// Rate limit exceeded (-32003).
    pub fn rate_limited(retry_after_secs: u64) -> Self {
        Self::with_data(
            -32003,
            "Rate limit exceeded",
            serde_json::json!({ "retry_after_secs": retry_after_secs }),
        )
    }

    /// Consent required (-32004).
    pub fn consent_required() -> Self {
        Self::new(-32004, "Consent required")
    }
}

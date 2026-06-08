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
    /// ACP protocol version (for ACP interop).
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

#[cfg(test)]
mod tests {
    use super::*;

    // REQ: JSON-RPC request must round-trip through serialization.
    #[test]
    fn request_round_trip() {
        let req = JsonRpcRequest::new(serde_json::json!(1), "acp/capabilities");
        let json = serde_json::to_string(&req).unwrap();
        let back: JsonRpcRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.jsonrpc, "2.0");
        assert_eq!(back.id, serde_json::json!(1));
        assert_eq!(back.method, "acp/capabilities");
        assert!(back.params.is_none());
        assert!(back.auth.is_none());
        assert!(back.acp_version.is_none());
    }

    // REQ: JSON-RPC request with all optional fields must round-trip.
    #[test]
    fn request_round_trip_with_optionals() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: serde_json::json!("uuid-42"),
            method: "acp/session.create".to_string(),
            params: Some(serde_json::json!({"mode": "chat"})),
            auth: Some(AuthInfo {
                auth_type: "macaroon".to_string(),
                token: "dG9rZW4=".to_string(),
            }),
            acp_version: Some("0.1.0".to_string()),
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: JsonRpcRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.jsonrpc, "2.0");
        assert_eq!(back.id, serde_json::json!("uuid-42"));
        assert_eq!(back.method, "acp/session.create");
        assert_eq!(back.params, Some(serde_json::json!({"mode": "chat"})));
        let auth = back.auth.unwrap();
        assert_eq!(auth.auth_type, "macaroon");
        assert_eq!(auth.token, "dG9rZW4=");
        assert_eq!(back.acp_version.unwrap(), "0.1.0");
    }

    // REQ: JSON-RPC request omits None optionals from wire format.
    #[test]
    fn request_skips_none_optionals() {
        let req = JsonRpcRequest::new(serde_json::json!(1), "ping");
        let json = serde_json::to_string(&req).unwrap();
        assert!(!json.contains("params"));
        assert!(!json.contains("auth"));
        assert!(!json.contains("acp_version"));
    }

    // REQ: JSON-RPC success response must round-trip.
    #[test]
    fn response_ok_round_trip() {
        let resp = JsonRpcResponse::ok(serde_json::json!(2), serde_json::json!({"status": "ok"}));
        let json = serde_json::to_string(&resp).unwrap();
        let back: JsonRpcResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(back.jsonrpc, "2.0");
        assert_eq!(back.id, serde_json::json!(2));
        assert!(back.result.is_some());
        assert!(back.error.is_none());
    }

    // REQ: JSON-RPC error response must round-trip.
    #[test]
    fn response_error_round_trip() {
        let err = JsonRpcError::method_not_found();
        let resp = JsonRpcResponse::error(serde_json::json!(3), err);
        let json = serde_json::to_string(&resp).unwrap();
        let back: JsonRpcResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(back.jsonrpc, "2.0");
        assert_eq!(back.id, serde_json::json!(3));
        assert!(back.result.is_none());
        let e = back.error.unwrap();
        assert_eq!(e.code, -32601);
        assert_eq!(e.message, "Method not found");
        assert!(e.data.is_none());
    }

    // REQ: JSON-RPC error with data must round-trip.
    #[test]
    fn error_with_data_round_trip() {
        let err = JsonRpcError::rate_limited(30);
        let json = serde_json::to_string(&err).unwrap();
        let back: JsonRpcError = serde_json::from_str(&json).unwrap();
        assert_eq!(back.code, -32003);
        assert_eq!(back.message, "Rate limit exceeded");
        let data = back.data.unwrap();
        assert_eq!(data["retry_after_secs"], 30);
    }

    // REQ: Standard error codes have correct values.
    #[test]
    fn standard_error_codes() {
        assert_eq!(JsonRpcError::parse_error().code, -32700);
        assert_eq!(JsonRpcError::invalid_request().code, -32600);
        assert_eq!(JsonRpcError::method_not_found().code, -32601);
        assert_eq!(JsonRpcError::invalid_params().code, -32602);
        assert_eq!(JsonRpcError::internal_error().code, -32603);
        assert_eq!(JsonRpcError::auth_required().code, -32001);
        assert_eq!(JsonRpcError::token_expired().code, -32002);
        assert_eq!(JsonRpcError::consent_required().code, -32004);
    }

    // REQ: Missing required fields cause deserialization errors.
    #[test]
    fn request_missing_method_fails() {
        let json = r#"{"jsonrpc":"2.0","id":1}"#;
        let result = serde_json::from_str::<JsonRpcRequest>(json);
        assert!(result.is_err());
    }

    // REQ: Missing required 'id' field on request causes deserialization error.
    #[test]
    fn request_missing_id_fails() {
        let json = r#"{"jsonrpc":"2.0","method":"ping"}"#;
        let result = serde_json::from_str::<JsonRpcRequest>(json);
        assert!(result.is_err());
    }
}

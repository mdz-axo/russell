// SPDX-License-Identifier: MIT OR Apache-2.0
//! MCP JSON-RPC client over HTTP POST.
//!
//! Connects to Kask's MCP endpoint on localhost. Enforces the
//! loopback constraint (ADR-0025 §4) at connect time.

use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::json;
use tracing::{debug, warn};

use crate::config::KaskMcpConfig;
use crate::error::{McpError, Result};
use crate::types::{
    InitializeResult, JsonRpcNotification, JsonRpcRequest, JsonRpcResponse, McpToolDefinition,
    PROTOCOL_VERSION, ToolCallParams, ToolCallResult, ToolsListResult,
};

/// Validate that a URL points to a loopback address.
///
/// Russell MUST NOT connect to non-loopback MCP servers (ADR-0025 §4).
/// This function is the structural enforcement — not convention, not
/// configuration, but code that refuses.
pub fn validate_endpoint(url: &str) -> Result<()> {
    // Parse the URL to extract the host.
    let parsed = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))
        .unwrap_or(url);

    // Extract host portion (before first `/` path separator).
    let authority = parsed.split('/').next().unwrap_or(parsed);

    // Handle bracketed IPv6: [::1]:port
    let host = if authority.starts_with('[') {
        // IPv6 bracketed form: extract between [ and ]
        authority
            .strip_prefix('[')
            .and_then(|s| s.split(']').next())
            .unwrap_or(authority)
    } else {
        // IPv4 or hostname: take before last colon (port separator).
        // But only split on last colon if what follows looks like a port.
        match authority.rsplit_once(':') {
            Some((host, maybe_port)) if maybe_port.chars().all(|c| c.is_ascii_digit()) => host,
            _ => authority,
        }
    };

    match host {
        "127.0.0.1" | "localhost" | "::1" => Ok(()),
        other => {
            // Try parsing as an IP to check loopback.
            if let Ok(ip) = other.parse::<std::net::Ipv4Addr>()
                && ip.is_loopback()
            {
                return Ok(());
            }
            if let Ok(ip) = other.parse::<std::net::Ipv6Addr>()
                && ip.is_loopback()
            {
                return Ok(());
            }
            Err(McpError::NonLoopbackRefused {
                url: url.to_owned(),
            })
        }
    }
}

/// MCP client for the trusted Kask relationship.
///
/// Speaks MCP JSON-RPC 2.0 over HTTP POST. Authenticated via bearer
/// token. Connects only to loopback addresses.
pub struct KaskMcpClient {
    config: KaskMcpConfig,
    http: reqwest::Client,
    request_id: AtomicU64,
    /// Server name from the last successful `initialize`.
    server_name: Option<String>,
    /// Whether the client has completed the `initialize` handshake.
    initialized: bool,
}

impl KaskMcpClient {
    /// Construct a new client from configuration.
    ///
    /// Does NOT connect or initialize — call [`connect`](Self::connect)
    /// to perform the handshake.
    ///
    /// # Errors
    /// Returns [`McpError::NonLoopbackRefused`] if the endpoint is not loopback.
    /// Returns [`McpError::Config`] if the HTTP client cannot be built.
    pub fn new(config: KaskMcpConfig) -> Result<Self> {
        config.validate()?;

        let http = reqwest::Client::builder()
            .timeout(config.timeout)
            .user_agent(format!("russell-mcp/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| McpError::Config(format!("failed to build HTTP client: {e}")))?;

        Ok(Self {
            config,
            http,
            request_id: AtomicU64::new(1),
            server_name: None,
            initialized: false,
        })
    }

    /// Perform the MCP `initialize` handshake.
    ///
    /// Sends `initialize` with Russell's client info and receives the
    /// server's capabilities. Follows up with `notifications/initialized`.
    ///
    /// # Errors
    /// Transport, protocol, or authentication errors.
    pub async fn connect(&mut self) -> Result<InitializeResult> {
        let params = json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": {
                "name": "russell-mcp",
                "version": env!("CARGO_PKG_VERSION"),
            }
        });

        let response = self.request("initialize", Some(params)).await?;

        let init_result: InitializeResult = serde_json::from_value(response)
            .map_err(|e| McpError::InvalidResponse(format!("initialize response: {e}")))?;

        self.server_name = init_result
            .server_info
            .as_ref()
            .and_then(|s| s.name.clone());

        // Send initialized notification.
        self.notify("notifications/initialized", None).await?;

        self.initialized = true;

        debug!(
            server = ?self.server_name,
            protocol = ?init_result.protocol_version,
            "MCP handshake complete"
        );

        Ok(init_result)
    }

    /// Discover available tools from the server.
    ///
    /// # Errors
    /// Transport, protocol, or authentication errors.
    pub async fn list_tools(&self) -> Result<Vec<McpToolDefinition>> {
        let response = self.request("tools/list", Some(json!({}))).await?;

        let result: ToolsListResult = serde_json::from_value(response)
            .map_err(|e| McpError::InvalidResponse(format!("tools/list response: {e}")))?;

        debug!(count = result.tools.len(), "tools/list complete");
        Ok(result.tools)
    }

    /// Invoke a tool by name with the given arguments.
    ///
    /// # Errors
    /// Transport, protocol, or authentication errors.
    pub async fn call_tool(
        &self,
        name: impl Into<String>,
        arguments: Option<serde_json::Value>,
    ) -> Result<ToolCallResult> {
        let params = ToolCallParams {
            name: name.into(),
            arguments,
        };

        let response = self
            .request(
                "tools/call",
                Some(serde_json::to_value(&params).map_err(|e| {
                    McpError::InvalidResponse(format!("failed to serialize tool call: {e}"))
                })?),
            )
            .await?;

        let result: ToolCallResult = serde_json::from_value(response)
            .map_err(|e| McpError::InvalidResponse(format!("tools/call response: {e}")))?;

        Ok(result)
    }

    /// Ping the server (keepalive / health check).
    ///
    /// Returns `Ok(())` if the server responds to ping.
    pub async fn ping(&self) -> Result<()> {
        let _response = self.request("ping", None).await?;
        Ok(())
    }

    /// Whether the client has completed initialization.
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Server name from the last successful handshake.
    pub fn server_name(&self) -> Option<&str> {
        self.server_name.as_deref()
    }

    /// The configured endpoint URL.
    pub fn endpoint(&self) -> &str {
        &self.config.endpoint
    }

    // ── Private helpers ────────────────────────────────────────────

    /// Send a JSON-RPC request and return the result value.
    async fn request(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        let id = self.request_id.fetch_add(1, Ordering::Relaxed);

        let rpc_request = JsonRpcRequest::new(id, method, params);

        let mut req = self
            .http
            .post(&self.config.endpoint)
            .header("Content-Type", "application/json");

        // Attach bearer token if configured.
        if let Some(ref token) = self.config.token {
            req = req.bearer_auth(token);
        }

        let resp = req
            .json(&rpc_request)
            .send()
            .await
            .map_err(|e| map_reqwest_error(&e))?;

        let status = resp.status();

        // Handle HTTP-level errors before parsing JSON-RPC.
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(McpError::Unauthenticated);
        }
        if status == reqwest::StatusCode::FORBIDDEN {
            return Err(McpError::Forbidden);
        }
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            let body_truncated = if body.len() > 500 {
                format!("{}...", &body[..500])
            } else {
                body
            };
            return Err(McpError::HttpStatus {
                status: status.as_u16(),
                body: body_truncated,
            });
        }

        // Parse JSON-RPC response.
        let body = resp.text().await.map_err(|e| McpError::Transport {
            message: format!("failed to read response body: {e}"),
            is_connect: false,
            is_timeout: false,
        })?;

        let rpc_response: JsonRpcResponse = serde_json::from_str(&body)
            .map_err(|e| McpError::InvalidResponse(format!("JSON-RPC parse: {e}: {body}")))?;

        // Check for JSON-RPC error.
        if let Some(err) = rpc_response.error {
            return Err(McpError::Protocol {
                code: err.code,
                message: err.message,
            });
        }

        // Return the result (default to empty object if absent).
        Ok(rpc_response.result.unwrap_or(json!({})))
    }

    /// Send a JSON-RPC notification (no response expected).
    async fn notify(&self, method: &str, params: Option<serde_json::Value>) -> Result<()> {
        let notification = JsonRpcNotification {
            jsonrpc: "2.0",
            method: method.to_owned(),
            params,
        };

        let mut req = self
            .http
            .post(&self.config.endpoint)
            .header("Content-Type", "application/json");

        if let Some(ref token) = self.config.token {
            req = req.bearer_auth(token);
        }

        // Fire and forget — notifications don't expect a response.
        let result = req.json(&notification).send().await;
        if let Err(e) = result {
            warn!(method, error = %e, "notification send failed (non-fatal)");
        }

        Ok(())
    }
}

/// Map a reqwest error to our McpError taxonomy.
fn map_reqwest_error(e: &reqwest::Error) -> McpError {
    McpError::Transport {
        message: e.to_string(),
        is_connect: e.is_connect(),
        is_timeout: e.is_timeout(),
    }
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_ipv4_accepted() {
        assert!(validate_endpoint("http://127.0.0.1:9500/mcp").is_ok());
    }

    #[test]
    fn loopback_ipv6_accepted() {
        assert!(validate_endpoint("http://[::1]:9500/mcp").is_ok());
    }

    #[test]
    fn localhost_accepted() {
        assert!(validate_endpoint("http://localhost:9500/mcp").is_ok());
    }

    #[test]
    fn remote_ipv4_rejected() {
        assert!(validate_endpoint("http://192.168.1.100:9500/mcp").is_err());
        assert!(validate_endpoint("http://10.0.0.1:9500/mcp").is_err());
        assert!(validate_endpoint("http://8.8.8.8:9500/mcp").is_err());
    }

    #[test]
    fn remote_hostname_rejected() {
        assert!(validate_endpoint("http://kask.example.com:9500/mcp").is_err());
    }

    #[test]
    fn https_loopback_accepted() {
        assert!(validate_endpoint("https://127.0.0.1:9500/mcp").is_ok());
    }

    #[test]
    fn client_construction_validates_endpoint() {
        let cfg = KaskMcpConfig {
            endpoint: "http://192.168.1.1:9500/mcp".into(),
            token: None,
            tool_ttl: std::time::Duration::from_secs(300),
            timeout: std::time::Duration::from_secs(30),
        };
        assert!(KaskMcpClient::new(cfg).is_err());
    }

    #[test]
    fn client_construction_succeeds_for_loopback() {
        let cfg = KaskMcpConfig {
            endpoint: "http://127.0.0.1:9500/mcp".into(),
            token: Some("test-token".into()),
            tool_ttl: std::time::Duration::from_secs(300),
            timeout: std::time::Duration::from_secs(30),
        };
        let client = KaskMcpClient::new(cfg).unwrap();
        assert!(!client.is_initialized());
        assert_eq!(client.endpoint(), "http://127.0.0.1:9500/mcp");
    }
}

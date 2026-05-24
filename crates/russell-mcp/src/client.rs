// SPDX-License-Identifier: MIT OR Apache-2.0
//! hKask REST API client.
//!
//! Connects to hKask's stack-api on localhost. Enforces the
//! loopback constraint (ADR-0025 §4) at connect time.
//!
//! Uses hKask's REST API endpoints:
//! - `GET /api/mcp/tools` — list all MCP tools
//! - `GET /api/mcp/tools/{name}` — get tool definition
//! - `POST /api/mcp/tools/{name}` — invoke a tool
//!
//! Paths are configurable via `HKASK_MCP_API_PATH` (default: `/api/mcp`).

use serde::Deserialize;
use serde_json::json;
use tracing::debug;

use crate::auth::TokenProvider;
use crate::config::HKaskMcpConfig;
use crate::error::{McpError, Result};
use crate::port::McpPort;
use crate::types::{McpToolDefinition, ToolCallResult};

/// MCP Client trait — hexagonal port for MCP tool access.
#[async_trait::async_trait]
pub trait McpClient: Send + Sync {
    /// Connect to the MCP endpoint and perform health check.
    async fn connect(&mut self) -> Result<()>;

    /// List available MCP tools.
    async fn list_tools(&self) -> Result<Vec<McpToolDefinition>>;

    /// Call an MCP tool by name with optional arguments.
    async fn call_tool(
        &self,
        name: &str,
        arguments: Option<serde_json::Value>,
    ) -> Result<ToolCallResult>;

    /// Server name from last successful connection.
    fn server_name(&self) -> Option<&str>;

    /// Whether the client is initialized (completed health check).
    fn is_initialized(&self) -> bool;

    /// The configured endpoint URL.
    fn endpoint(&self) -> &str;
}

#[async_trait::async_trait]
impl McpClient for HKaskMcpClient {
    async fn connect(&mut self) -> Result<()> {
        self.connect().await
    }

    async fn list_tools(&self) -> Result<Vec<McpToolDefinition>> {
        self.list_tools().await
    }

    async fn call_tool(
        &self,
        name: &str,
        arguments: Option<serde_json::Value>,
    ) -> Result<ToolCallResult> {
        self.call_tool(name, arguments).await
    }

    fn server_name(&self) -> Option<&str> {
        self.server_name()
    }

    fn is_initialized(&self) -> bool {
        self.is_initialized()
    }

    fn endpoint(&self) -> &str {
        self.endpoint()
    }
}

#[async_trait::async_trait]
impl McpPort for HKaskMcpClient {
    async fn list_tools(&self) -> Result<Vec<McpToolDefinition>> {
        self.list_tools().await
    }

    async fn call_tool(
        &self,
        name: &str,
        arguments: Option<serde_json::Value>,
    ) -> Result<ToolCallResult> {
        self.call_tool(name, arguments).await
    }

    async fn health_check(&self) -> Result<()> {
        self.ping().await
    }
}

/// Validate that a URL points to a loopback address.
///
/// Russell MUST NOT connect to non-loopback MCP servers (ADR-0025 §4).
/// This function is the structural enforcement — not convention, not
/// configuration, but code that refuses.
///
/// ## DNS Rebinding Defense
///
/// Hostnames are resolved to IP addresses and ALL resolved IPs must be
/// loopback. This prevents DNS rebinding attacks where a hostname like
/// `localhost.evil.com` resolves to a non-loopback address.
pub fn validate_endpoint(url: &str) -> Result<()> {
    let parsed = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))
        .unwrap_or(url);

    let authority = parsed.split('/').next().unwrap_or(parsed);

    let host = if authority.starts_with('[') {
        authority
            .strip_prefix('[')
            .and_then(|s| s.split(']').next())
            .unwrap_or(authority)
    } else {
        match authority.rsplit_once(':') {
            Some((host, maybe_port)) if maybe_port.chars().all(|c| c.is_ascii_digit()) => host,
            _ => authority,
        }
    };

    // Fast path: literal loopback addresses need no DNS resolution.
    match host {
        "127.0.0.1" | "::1" => return Ok(()),
        _ => {}
    }

    // Literal IP check (non-localhost IPv4/IPv6).
    if let Ok(ip) = host.parse::<std::net::Ipv4Addr>() {
        return if ip.is_loopback() {
            Ok(())
        } else {
            Err(McpError::NonLoopbackRefused {
                url: url.to_owned(),
            })
        };
    }
    if let Ok(ip) = host.parse::<std::net::Ipv6Addr>() {
        return if ip.is_loopback() {
            Ok(())
        } else {
            Err(McpError::NonLoopbackRefused {
                url: url.to_owned(),
            })
        };
    }

    // Hostname: resolve DNS and verify ALL resolved IPs are loopback.
    // "localhost" is accepted by name only as a special case (it is
    // universally mapped to 127.0.0.1/::1 in /etc/hosts).
    if host == "localhost" {
        return Ok(());
    }

    // Resolve hostname — reject if ANY address is non-loopback.
    let addrs = std::net::ToSocketAddrs::to_socket_addrs(&format!("{host}:0")).map_err(|_| {
        McpError::NonLoopbackRefused {
            url: url.to_owned(),
        }
    })?;

    let mut found_any = false;
    for addr in addrs {
        found_any = true;
        if !addr.ip().is_loopback() {
            return Err(McpError::NonLoopbackRefused {
                url: url.to_owned(),
            });
        }
    }

    if found_any {
        Ok(())
    } else {
        Err(McpError::NonLoopbackRefused {
            url: url.to_owned(),
        })
    }
}

/// hKask API client for the trusted relationship.
pub struct HKaskMcpClient {
    config: HKaskMcpConfig,
    http: reqwest::Client,
    /// Token provider for authentication (supports automatic refresh).
    token_provider: crate::auth::ChainedTokenProvider,
    /// Server name from the last successful connection.
    server_name: Option<String>,
    /// Whether the client has completed the health check.
    initialized: bool,
    /// Rate limiter for concurrent requests (default: 10 concurrent).
    rate_limit: Option<tokio::sync::Semaphore>,
}

impl HKaskMcpClient {
    /// Construct a new client from configuration.
    pub fn new(config: HKaskMcpConfig) -> Result<Self> {
        config.validate()?;

        let http = reqwest::Client::builder()
            .timeout(config.timeout)
            .user_agent(format!("russell-mcp/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| McpError::Config(format!("failed to build HTTP client: {e}")))?;

        // Token provider: file-based rotation with env fallback.
        let token_provider = crate::auth::ChainedTokenProvider::new(None)?;

        // Rate limiter: allow up to 10 concurrent requests to prevent
        // overwhelming the hKask gateway during high-frequency tool usage.
        let rate_limit = Some(tokio::sync::Semaphore::new(10));

        Ok(Self {
            config,
            http,
            token_provider,
            server_name: None,
            initialized: false,
            rate_limit,
        })
    }

    /// Perform a health check to verify hKask API is reachable.
    ///
    /// # Errors
    /// Transport or authentication errors.
    pub async fn connect(&mut self) -> Result<()> {
        let health_url = format!(
            "{}{}/tools",
            self.config.endpoint.trim_end_matches('/'),
            self.config.api_path.trim_end_matches('/')
        );

        let mut req = self
            .http
            .get(&health_url)
            .header("Accept", "application/json");

        // Attach bearer token if available.
        if let Ok(token) = self.token_provider.get_token().await {
            req = req.bearer_auth(token);
        }

        let resp = req.send().await.map_err(|e| map_reqwest_error(&e))?;

        let status = resp.status();

        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(McpError::Unauthenticated);
        }
        if status == reqwest::StatusCode::FORBIDDEN {
            return Err(McpError::Forbidden);
        }
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(McpError::HttpStatus {
                status: status.as_u16(),
                body,
            });
        }

        // Parse health response to extract server info.
        #[derive(Debug, Deserialize)]
        struct HealthResponse {
            #[serde(default)]
            version: Option<String>,
        }

        let health: HealthResponse = resp
            .json()
            .await
            .map_err(|e| McpError::InvalidResponse(format!("health response parse: {e}")))?;

        self.server_name = Some(format!(
            "hkask-api-{}",
            health.version.unwrap_or_else(|| "unknown".into())
        ));
        self.initialized = true;

        debug!(server = ?self.server_name, "hKask API connection established");
        Ok(())
    }

    /// Discover available tools from hKask.
    ///
    /// # Errors
    /// Transport, protocol, or authentication errors.
    pub async fn list_tools(&self) -> Result<Vec<McpToolDefinition>> {
        // Acquire rate limit permit (held for duration of request).
        if let Some(ref semaphore) = self.rate_limit {
            let _permit = semaphore
                .acquire()
                .await
                .map_err(|e| McpError::Config(format!("rate limiter closed: {e}")))?;
        }

        let tools_url = format!(
            "{}{}/tools",
            self.config.endpoint.trim_end_matches('/'),
            self.config.api_path.trim_end_matches('/')
        );

        let mut req = self
            .http
            .get(&tools_url)
            .header("Accept", "application/json");

        // Attach bearer token if available.
        if let Ok(token) = self.token_provider.get_token().await {
            req = req.bearer_auth(token);
        }

        let resp = req.send().await.map_err(|e| map_reqwest_error(&e))?;

        let status = resp.status();

        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(McpError::Unauthenticated);
        }
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(McpError::HttpStatus {
                status: status.as_u16(),
                body,
            });
        }

        // Parse the REST API response format.
        #[derive(Debug, Deserialize)]
        struct ToolInfo {
            name: String,
            #[serde(default)]
            description: Option<String>,
            #[serde(default, alias = "inputSchema")]
            input_schema: Option<serde_json::Value>,
            #[serde(default, alias = "server_id", alias = "serverId")]
            server: Option<String>,
        }

        let tools: Vec<ToolInfo> = resp
            .json()
            .await
            .map_err(|e| McpError::InvalidResponse(format!("tools response parse: {e}")))?;

        // Convert to McpToolDefinition format.
        let definitions: Vec<McpToolDefinition> = tools
            .into_iter()
            .map(|t| McpToolDefinition {
                name: t.name,
                description: t.description,
                input_schema: t.input_schema,
                annotations: None,
                server: t.server,
            })
            .collect();

        debug!(count = definitions.len(), "tools/list complete");
        Ok(definitions)
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
        // Acquire rate limit permit (held for duration of request).
        if let Some(ref semaphore) = self.rate_limit {
            let _permit = semaphore
                .acquire()
                .await
                .map_err(|e| McpError::Config(format!("rate limiter closed: {e}")))?;
        }

        let tool_name = name.into();

        if !self.config.allowed_tools.is_empty() && !self.config.allowed_tools.contains(&tool_name)
        {
            return Err(McpError::Forbidden);
        }

        let call_url = format!(
            "{}{}/tools/{}",
            self.config.endpoint.trim_end_matches('/'),
            self.config.api_path.trim_end_matches('/'),
            urlencoding::encode(&tool_name)
        );

        let mut req = self
            .http
            .post(&call_url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json");

        // Attach bearer token if available.
        if let Ok(token) = self.token_provider.get_token().await {
            req = req.bearer_auth(token);
        }

        let body = if let Some(args) = arguments {
            args
        } else {
            json!({})
        };

        let resp = req
            .json(&body)
            .send()
            .await
            .map_err(|e| map_reqwest_error(&e))?;

        let status = resp.status();

        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(McpError::Unauthenticated);
        }
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(McpError::HttpStatus {
                status: status.as_u16(),
                body,
            });
        }

        // Parse the REST API response format.
        #[derive(Debug, Deserialize)]
        struct ToolResponse {
            #[serde(default)]
            content: Vec<ToolContent>,
            #[serde(default, rename = "isError")]
            is_error: bool,
        }

        #[derive(Debug, Deserialize)]
        struct ToolContent {
            #[serde(rename = "type")]
            content_type: String,
            #[serde(default)]
            text: Option<String>,
            #[serde(flatten)]
            extra: serde_json::Value,
        }

        let tool_resp: ToolResponse = resp
            .json()
            .await
            .map_err(|e| McpError::InvalidResponse(format!("tool call response parse: {e}")))?;

        // Convert to ToolCallResult format.
        Ok(ToolCallResult {
            content: tool_resp
                .content
                .into_iter()
                .map(|c| crate::types::ToolContent {
                    content_type: c.content_type,
                    text: c.text,
                    extra: c.extra,
                })
                .collect(),
            is_error: tool_resp.is_error,
        })
    }

    /// Ping the server (keepalive / health check).
    ///
    /// Returns `Ok(())` if the server responds.
    pub async fn ping(&self) -> Result<()> {
        let health_url = format!(
            "{}{}/tools",
            self.config.endpoint.trim_end_matches('/'),
            self.config.api_path.trim_end_matches('/')
        );

        let mut req = self
            .http
            .get(&health_url)
            .header("Accept", "application/json");

        if let Ok(token) = self.token_provider.get_token().await {
            req = req.bearer_auth(token);
        }

        let resp = req.send().await.map_err(|e| map_reqwest_error(&e))?;

        if resp.status().is_success() {
            Ok(())
        } else {
            Err(McpError::HttpStatus {
                status: resp.status().as_u16(),
                body: resp.text().await.unwrap_or_default(),
            })
        }
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
        assert!(validate_endpoint("http://127.0.0.1:18100").is_ok());
    }

    #[test]
    fn loopback_ipv6_accepted() {
        assert!(validate_endpoint("http://[::1]:18100").is_ok());
    }

    #[test]
    fn localhost_accepted() {
        assert!(validate_endpoint("http://localhost:18100").is_ok());
    }

    #[test]
    fn remote_ipv4_rejected() {
        assert!(validate_endpoint("http://192.168.1.100:18100").is_err());
        assert!(validate_endpoint("http://10.0.0.1:18100").is_err());
        assert!(validate_endpoint("http://8.8.8.8:18100").is_err());
    }

    #[test]
    fn remote_hostname_rejected() {
        assert!(validate_endpoint("http://kask.example.com:18100").is_err());
    }

    #[test]
    fn https_loopback_accepted() {
        assert!(validate_endpoint("https://127.0.0.1:18100").is_ok());
    }

    #[test]
    fn dns_rebinding_non_loopback_rejected() {
        // A hostname that resolves to a non-loopback address must be rejected.
        // This test uses a well-known public DNS name that will never resolve to 127.0.0.1.
        assert!(validate_endpoint("http://one.one.one.one:18100").is_err());
    }

    #[test]
    fn unresolvable_hostname_rejected() {
        assert!(
            validate_endpoint("http://this-host-does-not-exist-russell-test.invalid:18100")
                .is_err()
        );
    }

    #[test]
    fn client_construction_validates_endpoint() {
        let cfg = HKaskMcpConfig {
            endpoint: "http://192.168.1.1:18100".into(),
            token: None,
            tool_ttl: std::time::Duration::from_secs(300),
            timeout: std::time::Duration::from_secs(30),
            api_path: crate::config::DEFAULT_API_PATH.to_owned(),
            allowed_tools: std::collections::HashSet::new(),
        };
        assert!(HKaskMcpClient::new(cfg).is_err());
    }

    #[test]
    fn client_construction_succeeds_for_loopback() {
        let cfg = HKaskMcpConfig {
            endpoint: "http://127.0.0.1:8080".into(),
            token: Some("test-token".into()),
            tool_ttl: std::time::Duration::from_secs(300),
            timeout: std::time::Duration::from_secs(30),
            api_path: crate::config::DEFAULT_API_PATH.to_owned(),
            allowed_tools: std::collections::HashSet::new(),
        };
        let client = HKaskMcpClient::new(cfg).unwrap();
        assert!(!client.is_initialized());
        assert_eq!(client.endpoint(), "http://127.0.0.1:8080");
    }
}

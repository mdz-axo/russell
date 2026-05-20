// SPDX-License-Identifier: MIT OR Apache-2.0
//! Configuration for the hKask MCP client.
//!
//! All values are read from environment variables (loaded via
//! `russell-core::env` from `russell.env`). Existing env vars
//! always win over file values.

use std::time::Duration;

use crate::error::Result;

/// MCP Config trait — hexagonal port for MCP configuration.
///
/// This trait defines the interface for MCP configurations, allowing Russell
/// to work with any MCP implementation without coupling to specific configs.
pub trait McpConfig {
    /// Get the endpoint URL.
    fn endpoint(&self) -> &str;

    /// Get the bearer token (if configured).
    fn token(&self) -> Option<&str>;

    /// Validate the configuration (e.g., loopback check).
    fn validate(&self) -> Result<()>;

    /// Whether a token is configured (can authenticate).
    fn has_token(&self) -> bool;
}

impl McpConfig for HKaskMcpConfig {
    fn endpoint(&self) -> &str {
        &self.endpoint
    }

    fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }

    fn validate(&self) -> Result<()> {
        self.validate()
    }

    fn has_token(&self) -> bool {
        self.has_token()
    }
}

/// Default hKask API endpoint (stack-api default bind).
pub const DEFAULT_ENDPOINT: &str = "http://127.0.0.1:18100";

/// Default tools/list cache TTL in seconds.
pub const DEFAULT_TOOL_TTL_SECS: u64 = 300;

/// Default HTTP timeout for MCP requests.
pub const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Environment variable: hKask MCP endpoint URL.
pub const ENV_HKASK_MCP_ENDPOINT: &str = "HKASK_MCP_ENDPOINT";

/// Environment variable: Bearer token for hKask authentication.
pub const ENV_HKASK_MCP_TOKEN: &str = "HKASK_MCP_TOKEN";

/// Environment variable: Tool list cache TTL in seconds.
pub const ENV_HKASK_MCP_TOOL_TTL_SECS: &str = "HKASK_MCP_TOOL_TTL_SECS";

/// Environment variable: HTTP timeout in seconds.
pub const ENV_HKASK_MCP_TIMEOUT_SECS: &str = "HKASK_MCP_TIMEOUT_SECS";

/// Configuration for the hKask MCP client.
///
/// Constructed from environment variables. The only required value
/// for a functional connection is `HKASK_MCP_TOKEN` — without it,
/// the client can still be constructed but will fail authentication.
#[derive(Debug, Clone)]
pub struct HKaskMcpConfig {
    /// MCP endpoint URL (must be loopback).
    pub endpoint: String,
    /// Bearer token for authentication.
    pub token: Option<String>,
    /// TTL for the tools/list cache.
    pub tool_ttl: Duration,
    /// HTTP request timeout.
    pub timeout: Duration,
}

impl HKaskMcpConfig {
    /// Load configuration from environment variables.
    ///
    /// Does NOT fail on missing token — the client can be constructed
    /// in a degraded state (for health checks / reachability probes).
    pub fn from_env() -> Self {
        let endpoint =
            std::env::var(ENV_HKASK_MCP_ENDPOINT).unwrap_or_else(|_| DEFAULT_ENDPOINT.to_owned());

        let token = std::env::var(ENV_HKASK_MCP_TOKEN)
            .ok()
            .filter(|s| !s.is_empty());

        let tool_ttl = std::env::var(ENV_HKASK_MCP_TOOL_TTL_SECS)
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .map(Duration::from_secs)
            .unwrap_or(Duration::from_secs(DEFAULT_TOOL_TTL_SECS));

        let timeout = std::env::var(ENV_HKASK_MCP_TIMEOUT_SECS)
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .map(Duration::from_secs)
            .unwrap_or(Duration::from_secs(DEFAULT_TIMEOUT_SECS));

        Self {
            endpoint,
            token,
            tool_ttl,
            timeout,
        }
    }

    /// Validate that the configured endpoint is loopback.
    /// Called at connect time, not at construction time (so config
    /// can be inspected even if invalid).
    pub fn validate(&self) -> Result<()> {
        crate::client::validate_endpoint(&self.endpoint)?;
        Ok(())
    }

    /// Whether the client has a token configured (can authenticate).
    pub fn has_token(&self) -> bool {
        self.token.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_loopback() {
        let cfg = HKaskMcpConfig {
            endpoint: DEFAULT_ENDPOINT.to_owned(),
            token: Some("test-token".into()),
            tool_ttl: Duration::from_secs(DEFAULT_TOOL_TTL_SECS),
            timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECS),
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn remote_endpoint_rejected() {
        let cfg = HKaskMcpConfig {
            endpoint: "http://192.168.1.100:9500/mcp".to_owned(),
            token: Some("test-token".into()),
            tool_ttl: Duration::from_secs(300),
            timeout: Duration::from_secs(30),
        };
        assert!(cfg.validate().is_err());
    }
}

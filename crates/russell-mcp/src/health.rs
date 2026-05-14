// SPDX-License-Identifier: MIT OR Apache-2.0
//! Kask MCP health probe for proprioception.
//!
//! Provides a quick async reachability check suitable for inclusion
//! in Russell's self-vital cycle. Uses a short timeout (2s) to avoid
//! blocking the proprioception cadence.

use std::time::{Duration, Instant};

use tracing::debug;

use crate::config::KaskMcpConfig;

/// Timeout for the reachability probe (short — must not block proprio).
const HEALTH_TIMEOUT: Duration = Duration::from_secs(2);

/// Result of a Kask MCP health probe.
#[derive(Debug, Clone)]
pub struct KaskHealthResult {
    /// Whether the endpoint responded.
    pub reachable: bool,
    /// Round-trip latency in milliseconds (None if unreachable).
    pub latency_ms: Option<u64>,
    /// Error message if unreachable.
    pub error: Option<String>,
}

/// Probe the Kask MCP endpoint for reachability.
///
/// Sends a minimal HTTP POST (MCP `ping`) to the configured endpoint.
/// Uses a 2-second timeout to avoid blocking the proprioception cycle.
///
/// This is NOT a full MCP handshake — it's a fast health check that
/// verifies the endpoint is up and responding to HTTP. For a full
/// initialize + tools/list, use [`KaskMcpClient`](crate::client::KaskMcpClient).
pub async fn probe_reachability() -> KaskHealthResult {
    let config = KaskMcpConfig::from_env();

    // Validate endpoint first.
    if let Err(e) = config.validate() {
        return KaskHealthResult {
            reachable: false,
            latency_ms: None,
            error: Some(format!("config: {e}")),
        };
    }

    let http = match reqwest::Client::builder().timeout(HEALTH_TIMEOUT).build() {
        Ok(c) => c,
        Err(e) => {
            return KaskHealthResult {
                reachable: false,
                latency_ms: None,
                error: Some(format!("http client: {e}")),
            };
        }
    };

    // Send a MCP ping (JSON-RPC).
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "ping"
    });

    let start = Instant::now();

    let mut req = http
        .post(&config.endpoint)
        .header("Content-Type", "application/json");

    if let Some(ref token) = config.token {
        req = req.bearer_auth(token);
    }

    match req.json(&body).send().await {
        Ok(resp) => {
            let latency = start.elapsed().as_millis() as u64;
            if resp.status().is_success() {
                debug!(latency_ms = latency, "kask MCP reachable");
                KaskHealthResult {
                    reachable: true,
                    latency_ms: Some(latency),
                    error: None,
                }
            } else {
                let status = resp.status().as_u16();
                KaskHealthResult {
                    reachable: false,
                    latency_ms: Some(latency),
                    error: Some(format!("HTTP {status}")),
                }
            }
        }
        Err(e) => {
            let msg = if e.is_connect() {
                "connection refused".to_owned()
            } else if e.is_timeout() {
                "timeout (2s)".to_owned()
            } else {
                e.to_string()
            };
            debug!(error = %msg, "kask MCP unreachable");
            KaskHealthResult {
                reachable: false,
                latency_ms: None,
                error: Some(msg),
            }
        }
    }
}

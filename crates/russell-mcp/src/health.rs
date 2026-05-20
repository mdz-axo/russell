// SPDX-License-Identifier: MIT OR Apache-2.0
//! hKask MCP health probe for proprioception.
//!
//! Provides a quick async reachability check suitable for inclusion
//! in Russell's self-vital cycle. Uses a short timeout (2s) to avoid
//! blocking the proprioception cadence.

use std::time::{Duration, Instant};

use tracing::debug;

use crate::config::HKaskMcpConfig;

/// Timeout for the reachability probe (short — must not block proprio).
const HEALTH_TIMEOUT: Duration = Duration::from_secs(2);

/// Result of a hKask MCP health probe.
#[derive(Debug, Clone)]
pub struct HKaskHealthResult {
    /// Whether the endpoint responded.
    pub reachable: bool,
    /// Round-trip latency in milliseconds (None if unreachable).
    pub latency_ms: Option<u64>,
    /// Error message if unreachable.
    pub error: Option<String>,
}

/// Probe the hKask MCP endpoint for reachability.
///
/// Sends a GET request to the REST API `/health` endpoint.
/// Uses a 2-second timeout to avoid blocking the proprioception cycle.
///
/// This is a fast health check that verifies the endpoint is up and responding.
pub async fn probe_reachability() -> HKaskHealthResult {
    let config = HKaskMcpConfig::from_env();

    // Validate endpoint first.
    if let Err(e) = config.validate() {
        return HKaskHealthResult {
            reachable: false,
            latency_ms: None,
            error: Some(format!("config: {e}")),
        };
    }

    let http = match reqwest::Client::builder().timeout(HEALTH_TIMEOUT).build() {
        Ok(c) => c,
        Err(e) => {
            return HKaskHealthResult {
                reachable: false,
                latency_ms: None,
                error: Some(format!("http client: {e}")),
            };
        }
    };

    // Build health endpoint URL (REST API, not JSON-RPC).
    let health_url = format!("{}/health", config.endpoint.trim_end_matches('/'));

    let start = Instant::now();

    let mut req = http.get(&health_url).header("Accept", "application/json");

    if let Some(ref token) = config.token {
        req = req.bearer_auth(token);
    }

    match req.send().await {
        Ok(resp) => {
            let latency = start.elapsed().as_millis() as u64;
            if resp.status().is_success() {
                debug!(latency_ms = latency, "hKask MCP reachable");
                HKaskHealthResult {
                    reachable: true,
                    latency_ms: Some(latency),
                    error: None,
                }
            } else {
                let status = resp.status().as_u16();
                HKaskHealthResult {
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
            debug!(error = %msg, "hKask MCP unreachable");
            HKaskHealthResult {
                reachable: false,
                latency_ms: None,
                error: Some(msg),
            }
        }
    }
}

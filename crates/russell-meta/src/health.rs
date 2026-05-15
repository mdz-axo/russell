// SPDX-License-Identifier: MIT OR Apache-2.0
//! Shared Okapi health-check and auto-start pipeline.
//!
//! Both `russell jack` (one-shot SOAP) and `russell chat` (REPL) need
//! to verify that Okapi is reachable before dispatching LLM calls.
//! This module provides a single `ensure_ready` function that:
//!
//! 1. Pings Okapi's `/api/tags` endpoint (3 s timeout).
//! 2. If unreachable, attempts `systemctl --user start okapi`.
//! 3. Waits for readiness (up to ~6 s after start).
//! 4. Returns whether Okapi is ready.
//!
//! Design note: this is the *only* place that knows how to wake Okapi.
//! All call-sites go through [`ensure_ready`].

use tracing::{info, warn};

/// Default Okapi base URL (OpenAI-compat layer).
pub const DEFAULT_BASE_URL: &str = "http://127.0.0.1:11435/v1";

/// Derive the health-check URL from a base URL.
///
/// Strips any trailing `/` and `/v1` suffix, then appends `/api/tags`
/// (Ollama native endpoint).
#[must_use]
pub fn health_url_from_base(base_url: &str) -> String {
    let stripped = base_url
        .trim_end_matches('/')
        .trim_end_matches("/v1")
        .trim_end_matches('/');
    format!("{stripped}/api/tags")
}

/// Ensure Okapi is reachable, starting it if necessary.
///
/// Returns `true` if Okapi responded to a health check (either
/// immediately or after an auto-start attempt). Returns `false` if
/// Okapi could not be reached even after attempting to start it.
///
/// After a cold start, Okapi needs time for GPU discovery before it
/// can route inference requests (~10 s). This function polls until the
/// completions endpoint responds (up to ~15 s post-start).
pub async fn ensure_ready(base_url: &str) -> bool {
    let tags_url = health_url_from_base(base_url);

    // Fast path: if Okapi is already running and warmed up.
    if ping(&tags_url).await {
        return true;
    }

    info!("okapi not reachable — attempting auto-start");
    start_service().await;

    // Poll until the API responds. Okapi typically needs 10–12 s after
    // start to complete GPU discovery and become fully ready for
    // inference routing.
    wait_until_ready(&tags_url, 14, 1).await
}

/// Poll `health_url` up to `max_attempts` times, sleeping `interval_s`
/// between each attempt. Returns `true` on first success.
async fn wait_until_ready(health_url: &str, max_attempts: u32, interval_s: u64) -> bool {
    for attempt in 1..=max_attempts {
        if ping(health_url).await {
            info!(attempt, "okapi ready");
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_secs(interval_s)).await;
    }
    warn!("okapi did not become ready after polling");
    false
}

/// Lightweight ping: GET `/api/tags` with a 3 s timeout.
async fn ping(health_url: &str) -> bool {
    let Ok(client) = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
    else {
        return false;
    };
    match client.get(health_url).send().await {
        Ok(resp) => resp.status().is_success(),
        Err(_) => false,
    }
}

/// Attempt to start Okapi via `systemctl --user start okapi`.
///
/// If systemctl succeeds, gives the process 1 s to bind the port
/// before returning (the caller polls for full readiness separately).
async fn start_service() {
    let output = tokio::process::Command::new("systemctl")
        .args(["--user", "start", "okapi"])
        .output()
        .await;
    match output {
        Ok(o) if o.status.success() => {
            info!("okapi started via systemctl --user");
            // Brief pause for the port to bind; full readiness is polled by caller.
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
        Ok(o) => {
            warn!(
                stderr = %String::from_utf8_lossy(&o.stderr),
                code = o.status.code(),
                "systemctl --user start okapi returned non-zero"
            );
        }
        Err(e) => {
            warn!(error = %e, "failed to run systemctl --user start okapi");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_url_strips_v1() {
        assert_eq!(
            health_url_from_base("http://127.0.0.1:11435/v1"),
            "http://127.0.0.1:11435/api/tags"
        );
    }

    #[test]
    fn health_url_no_v1() {
        assert_eq!(
            health_url_from_base("http://127.0.0.1:11435"),
            "http://127.0.0.1:11435/api/tags"
        );
    }

    #[test]
    fn health_url_trailing_slash() {
        assert_eq!(
            health_url_from_base("http://127.0.0.1:11435/v1/"),
            "http://127.0.0.1:11435/api/tags"
        );
    }
}

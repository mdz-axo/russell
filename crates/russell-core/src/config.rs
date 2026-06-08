// SPDX-License-Identifier: MIT OR Apache-2.0
//! Runtime configuration — env-driven endpoint and operational settings.
//!
//! Centralizes all configurable endpoints and operational parameters
//! that were previously hardcoded (W-14). All values are read from
//! environment variables with sensible defaults for the standard
//! single-host deployment.

/// Default Okapi inference base URL.
pub const DEFAULT_OKAPI_ENDPOINT: &str = "http://127.0.0.1:11435/v1";

/// Environment variable: Okapi base URL.
pub const ENV_OKAPI_ENDPOINT: &str = "RUSSELL_OKAPI_ENDPOINT";

/// Runtime configuration for Russell's external service endpoints.
///
/// All values default to the standard single-host loopback deployment.
/// Override via environment variables or by constructing directly in tests.
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    /// Okapi base URL (default: `http://127.0.0.1:11435/v1`).
    pub okapi_endpoint: String,
}

impl RuntimeConfig {
    /// Load configuration from environment variables, falling back to defaults.
    #[must_use]
    pub fn from_env() -> Self {
        Self {
            okapi_endpoint: std::env::var(ENV_OKAPI_ENDPOINT)
                .unwrap_or_else(|_| DEFAULT_OKAPI_ENDPOINT.to_owned()),
        }
    }
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            okapi_endpoint: DEFAULT_OKAPI_ENDPOINT.to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_loopback_endpoints() {
        let cfg = RuntimeConfig::default();
        assert!(cfg.okapi_endpoint.contains("127.0.0.1"));
    }
}

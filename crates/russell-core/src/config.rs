// SPDX-License-Identifier: MIT OR Apache-2.0
//! Runtime configuration — env-driven endpoint and operational settings.
//!
//! Centralizes all configurable endpoints and operational parameters
//! that were previously hardcoded (W-14). All values are read from
//! environment variables with sensible defaults for the standard
//! single-host deployment.

/// Default hKask SOAP inference endpoint.
pub const DEFAULT_HKASK_ENDPOINT: &str = "http://127.0.0.1:8080/api/llm/infer";

/// Default Okapi inference base URL.
pub const DEFAULT_OKAPI_ENDPOINT: &str = "http://127.0.0.1:11435/v1";

/// Environment variable: hKask SOAP inference endpoint.
pub const ENV_HKASK_ENDPOINT: &str = "RUSSELL_HKASK_ENDPOINT";

/// Environment variable: Okapi base URL.
pub const ENV_OKAPI_ENDPOINT: &str = "RUSSELL_OKAPI_ENDPOINT";

/// Runtime configuration for Russell's external service endpoints.
///
/// All values default to the standard single-host loopback deployment.
/// Override via environment variables or by constructing directly in tests.
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    /// hKask SOAP inference endpoint (default: `http://127.0.0.1:8080/api/llm/infer`).
    pub hkask_endpoint: String,
    /// Okapi base URL (default: `http://127.0.0.1:11435/v1`).
    pub okapi_endpoint: String,
}

impl RuntimeConfig {
    /// Load configuration from environment variables, falling back to defaults.
    #[must_use]
    pub fn from_env() -> Self {
        Self {
            hkask_endpoint: std::env::var(ENV_HKASK_ENDPOINT)
                .unwrap_or_else(|_| DEFAULT_HKASK_ENDPOINT.to_owned()),
            okapi_endpoint: std::env::var(ENV_OKAPI_ENDPOINT)
                .unwrap_or_else(|_| DEFAULT_OKAPI_ENDPOINT.to_owned()),
        }
    }
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            hkask_endpoint: DEFAULT_HKASK_ENDPOINT.to_owned(),
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
        assert!(cfg.hkask_endpoint.contains("127.0.0.1"));
        assert!(cfg.okapi_endpoint.contains("127.0.0.1"));
    }
}

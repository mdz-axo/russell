// SPDX-License-Identifier: MIT OR Apache-2.0
//! Fallback inference adapter — tries primary backend, falls back to secondary.

use async_trait::async_trait;
use russell_core::error::Result;
use russell_core::inference::{InferencePort, InferenceResponse, SoapBundle};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::Duration;
use tracing::{info, warn};

/// Circuit breaker state for a single backend.
struct CircuitBreaker {
    /// Consecutive failures.
    failures: AtomicU32,
    /// Threshold of consecutive failures before tripping (default: 3).
    threshold: u32,
    /// Unix timestamp (ms) when the circuit last tripped open.
    opened_at_ms: AtomicU64,
    /// Duration before attempting half-open reset (default: 30s).
    reset_timeout: Duration,
}

impl CircuitBreaker {
    fn new(threshold: u32, reset_timeout: Duration) -> Self {
        Self {
            failures: AtomicU32::new(0),
            threshold,
            opened_at_ms: AtomicU64::new(0),
            reset_timeout,
        }
    }

    /// Whether the circuit is closed (allowing calls).
    fn is_closed(&self) -> bool {
        let opened = self.opened_at_ms.load(Ordering::Relaxed);
        if opened == 0 {
            return true;
        }
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        if now_ms >= opened + self.reset_timeout.as_millis() as u64 {
            return true;
        }
        false
    }

    fn record_success(&self) {
        self.failures.store(0, Ordering::Relaxed);
        self.opened_at_ms.store(0, Ordering::Relaxed);
    }

    fn record_failure(&self) {
        let count = self.failures.fetch_add(1, Ordering::Relaxed) + 1;
        if count >= self.threshold {
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            self.opened_at_ms.store(now_ms, Ordering::Relaxed);
        }
    }
}

/// Fallback inference adapter — tries primary backend first, falls back to secondary on failure.
///
/// Includes a circuit breaker per backend to avoid hammering a down service.
/// Typical usage: primary = Okapi, secondary = local fallback.
pub struct FallbackInferenceAdapter {
    /// Primary backend.
    primary: Arc<dyn InferencePort>,
    /// Secondary backend.
    secondary: Arc<dyn InferencePort>,
    /// Circuit breaker for primary.
    primary_cb: CircuitBreaker,
    /// Circuit breaker for secondary.
    secondary_cb: CircuitBreaker,
}

impl FallbackInferenceAdapter {
    /// Create a new fallback adapter with primary and secondary backends.
    ///
    /// Uses default circuit breaker settings: 3 failures to trip, 30s reset timeout.
    pub fn new(primary: Arc<dyn InferencePort>, secondary: Arc<dyn InferencePort>) -> Self {
        Self::with_circuit_breaker(primary, secondary, 3, Duration::from_secs(30))
    }

    /// Create a new fallback adapter with custom circuit breaker settings.
    pub fn with_circuit_breaker(
        primary: Arc<dyn InferencePort>,
        secondary: Arc<dyn InferencePort>,
        cb_threshold: u32,
        cb_reset_timeout: Duration,
    ) -> Self {
        Self {
            primary,
            secondary,
            primary_cb: CircuitBreaker::new(cb_threshold, cb_reset_timeout),
            secondary_cb: CircuitBreaker::new(cb_threshold, cb_reset_timeout),
        }
    }
}

#[async_trait]
impl InferencePort for FallbackInferenceAdapter {
    async fn infer(&self, prompt: &str, context: Option<&SoapBundle>) -> Result<InferenceResponse> {
        if self.primary_cb.is_closed() {
            match self.primary.infer(prompt, context).await {
                Ok(response) => {
                    self.primary_cb.record_success();
                    info!(backend = %response.backend, "inference succeeded via primary backend");
                    return Ok(response);
                }
                Err(primary_err) => {
                    self.primary_cb.record_failure();
                    warn!(
                        primary_backend = %self.primary.backend_id(),
                        error = %primary_err,
                        "primary backend failed, falling back to secondary"
                    );
                }
            }
        } else {
            warn!(primary_backend = %self.primary.backend_id(), "primary circuit breaker open, skipping");
        }

        if self.secondary_cb.is_closed() {
            match self.secondary.infer(prompt, context).await {
                Ok(response) => {
                    self.secondary_cb.record_success();
                    info!(backend = %response.backend, "inference succeeded via secondary backend");
                    Ok(response)
                }
                Err(secondary_err) => {
                    self.secondary_cb.record_failure();
                    Err(russell_core::error::CoreError::Invariant(format!(
                        "both backends failed: primary=circuit-open/failed, secondary={}",
                        secondary_err
                    )))
                }
            }
        } else {
            Err(russell_core::error::CoreError::Invariant(
                "both backends circuit-breaker open".to_string(),
            ))
        }
    }

    async fn health_check(&self) -> Result<bool> {
        // Healthy if either backend is healthy
        let primary_healthy = self.primary.health_check().await.unwrap_or(false);
        let secondary_healthy = self.secondary.health_check().await.unwrap_or(false);
        Ok(primary_healthy || secondary_healthy)
    }

    fn backend_id(&self) -> &str {
        "fallback"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use russell_core::inference::TokenUsage;

    struct MockAdapter {
        id: String,
        should_fail: bool,
    }

    #[async_trait]
    impl InferencePort for MockAdapter {
        async fn infer(
            &self,
            prompt: &str,
            _context: Option<&SoapBundle>,
        ) -> Result<InferenceResponse> {
            if self.should_fail {
                Err(russell_core::error::CoreError::Invariant(
                    "mock failure".to_string(),
                ))
            } else {
                Ok(InferenceResponse {
                    text: format!("mock response to: {}", prompt),
                    backend: self.id.clone(),
                    model: Some("mock-model".to_string()),
                    latency_ms: Some(100),
                    token_usage: Some(TokenUsage {
                        input_tokens: 10,
                        output_tokens: 20,
                        total_tokens: 30,
                    }),
                    actions: Vec::new(),
                })
            }
        }

        async fn health_check(&self) -> Result<bool> {
            Ok(!self.should_fail)
        }

        fn backend_id(&self) -> &str {
            &self.id
        }
    }

    #[tokio::test]
    async fn test_fallback_uses_primary_when_healthy() {
        let primary = Arc::new(MockAdapter {
            id: "primary".to_string(),
            should_fail: false,
        });
        let secondary = Arc::new(MockAdapter {
            id: "secondary".to_string(),
            should_fail: false,
        });

        let adapter = FallbackInferenceAdapter::new(primary, secondary);
        let response = adapter.infer("test prompt", None).await.unwrap();

        assert_eq!(response.backend, "primary");
    }

    #[tokio::test]
    async fn test_fallback_uses_secondary_when_primary_fails() {
        let primary = Arc::new(MockAdapter {
            id: "primary".to_string(),
            should_fail: true,
        });
        let secondary = Arc::new(MockAdapter {
            id: "secondary".to_string(),
            should_fail: false,
        });

        let adapter = FallbackInferenceAdapter::new(primary, secondary);
        let response = adapter.infer("test prompt", None).await.unwrap();

        assert_eq!(response.backend, "secondary");
    }

    #[tokio::test]
    async fn test_fallback_fails_when_both_fail() {
        let primary = Arc::new(MockAdapter {
            id: "primary".to_string(),
            should_fail: true,
        });
        let secondary = Arc::new(MockAdapter {
            id: "secondary".to_string(),
            should_fail: true,
        });

        let adapter = FallbackInferenceAdapter::new(primary, secondary);
        let result = adapter.infer("test prompt", None).await;

        assert!(result.is_err());
    }
}

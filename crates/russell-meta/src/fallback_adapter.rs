// SPDX-License-Identifier: MIT OR Apache-2.0
//! Fallback inference adapter — tries primary backend, falls back to secondary.

use async_trait::async_trait;
use russell_core::error::Result;
use russell_core::inference::{InferencePort, InferenceResponse, SoapBundle};
use std::sync::Arc;
use tracing::{info, warn};

/// Fallback inference adapter — tries primary backend first, falls back to secondary on failure.
///
/// Typical usage: primary = hKask (remote), secondary = Okapi (local).
pub struct FallbackInferenceAdapter {
    /// Primary backend (e.g., hKask).
    primary: Arc<dyn InferencePort>,
    /// Secondary backend (e.g., Okapi).
    secondary: Arc<dyn InferencePort>,
}

impl FallbackInferenceAdapter {
    /// Create a new fallback adapter with primary and secondary backends.
    pub fn new(primary: Arc<dyn InferencePort>, secondary: Arc<dyn InferencePort>) -> Self {
        Self { primary, secondary }
    }
}

#[async_trait]
impl InferencePort for FallbackInferenceAdapter {
    async fn infer(&self, prompt: &str, context: Option<&SoapBundle>) -> Result<InferenceResponse> {
        // Try primary first
        match self.primary.infer(prompt, context).await {
            Ok(response) => {
                info!(backend = %response.backend, "inference succeeded via primary backend");
                Ok(response)
            }
            Err(primary_err) => {
                warn!(
                    primary_backend = %self.primary.backend_id(),
                    error = %primary_err,
                    "primary backend failed, falling back to secondary"
                );

                // Fall back to secondary
                match self.secondary.infer(prompt, context).await {
                    Ok(response) => {
                        info!(backend = %response.backend, "inference succeeded via secondary backend");
                        Ok(response)
                    }
                    Err(secondary_err) => Err(russell_core::error::CoreError::Invariant(format!(
                        "both backends failed: primary={}, secondary={}",
                        primary_err, secondary_err
                    ))),
                }
            }
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

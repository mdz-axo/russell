// SPDX-License-Identifier: MIT OR Apache-2.0
//! hKask inference adapter — implements InferencePort for hKask REST API.

use async_trait::async_trait;
use russell_core::error::Result;
use russell_core::inference::{InferencePort, InferenceResponse, SoapBundle, TokenUsage};
use tracing::{debug, warn};

/// hKask inference adapter — connects to hKask's `/api/llm/infer` endpoint.
pub struct HkaskInferenceAdapter {
    /// hKask API endpoint URL.
    endpoint: String,
    /// Capability token for authentication.
    capability_token: Option<String>,
    /// HTTP client.
    client: reqwest::Client,
}

impl HkaskInferenceAdapter {
    /// Create a new hKask inference adapter.
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            capability_token: None,
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("failed to build HTTP client"),
        }
    }

    /// Set the capability token for authentication.
    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.capability_token = Some(token.into());
        self
    }

    /// Load capability token from the standard location.
    pub fn with_token_from_file(mut self) -> Option<Self> {
        let token = crate::help::load_capability_token()?;
        self.capability_token = Some(token);
        Some(self)
    }
}

#[async_trait]
impl InferencePort for HkaskInferenceAdapter {
    async fn infer(&self, prompt: &str, context: Option<&SoapBundle>) -> Result<InferenceResponse> {
        let start = std::time::Instant::now();

        let url = format!("{}/api/llm/infer", self.endpoint.trim_end_matches('/'));

        let mut request = self
            .client
            .post(&url)
            .header("Content-Type", "application/json");

        if let Some(ref token) = self.capability_token {
            request = request.header("Authorization", format!("Bearer {}", token));
        }

        let body = serde_json::json!({
            "subjective": prompt,
            "objective": context.map(|c| &c.objective).unwrap_or(&Vec::new()),
            "assessment": context.and_then(|c| c.assessment.as_deref()).unwrap_or(""),
            "plan": context.and_then(|c| c.plan.as_deref()).unwrap_or(&[]),
        });

        debug!(url = %url, "sending inference request to hKask");

        let response = request.json(&body).send().await.map_err(|e| {
            russell_core::error::CoreError::Invariant(format!("HTTP request failed: {e}"))
        })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            warn!(status = %status, body = %body, "hKask inference failed");
            return Err(russell_core::error::CoreError::Invariant(format!(
                "hKask returned {}: {}",
                status, body
            )));
        }

        let infer_response: serde_json::Value = response.json().await.map_err(|e| {
            russell_core::error::CoreError::Invariant(format!("failed to parse response: {e}"))
        })?;

        let text = infer_response
            .get("response")
            .and_then(|v| v.as_str())
            .unwrap_or("No response from hKask")
            .to_string();

        let latency_ms = start.elapsed().as_millis() as u64;

        Ok(InferenceResponse {
            text,
            backend: "hkask".to_string(),
            model: infer_response
                .get("model")
                .and_then(|v| v.as_str())
                .map(String::from),
            latency_ms: Some(latency_ms),
            token_usage: infer_response.get("token_usage").and_then(|v| {
                Some(TokenUsage {
                    input_tokens: v.get("input_tokens")?.as_u64()?,
                    output_tokens: v.get("output_tokens")?.as_u64()?,
                    total_tokens: v.get("total_tokens")?.as_u64()?,
                })
            }),
        })
    }

    async fn health_check(&self) -> Result<bool> {
        let url = format!("{}/health", self.endpoint.trim_end_matches('/'));
        match self.client.get(&url).send().await {
            Ok(resp) => Ok(resp.status().is_success()),
            Err(_) => Ok(false),
        }
    }

    fn backend_id(&self) -> &str {
        "hkask"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_creation() {
        let adapter = HkaskInferenceAdapter::new("http://localhost:8080");
        assert_eq!(adapter.backend_id(), "hkask");
        assert!(adapter.capability_token.is_none());
    }

    #[test]
    fn test_adapter_with_token() {
        let adapter = HkaskInferenceAdapter::new("http://localhost:8080").with_token("test-token");
        assert_eq!(adapter.capability_token.as_deref(), Some("test-token"));
    }
}

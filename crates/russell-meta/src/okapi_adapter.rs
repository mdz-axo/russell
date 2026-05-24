// SPDX-License-Identifier: MIT OR Apache-2.0
//! Okapi inference adapter — local-first fallback using Ollama-compatible API.

use async_trait::async_trait;
use russell_core::error::Result;
use russell_core::inference::{InferencePort, InferenceResponse, SoapBundle, TokenUsage};
use tracing::{debug, warn};

/// Okapi inference adapter — connects to Ollama-compatible local LLM endpoint.
///
/// Default endpoint: `http://localhost:11434` (Ollama standard port).
/// Supports model selection via `with_model()`.
pub struct OkapiInferenceAdapter {
    /// Ollama-compatible API endpoint URL.
    endpoint: String,
    /// Model identifier (e.g., "llama3", "mistral", "deepseek-v2").
    model: String,
    /// HTTP client.
    client: reqwest::Client,
}

impl OkapiInferenceAdapter {
    /// Create a new Okapi inference adapter with default model "llama3".
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            model: "llama3".to_string(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .expect("failed to build HTTP client"),
        }
    }

    /// Set the model identifier.
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }
}

#[async_trait]
impl InferencePort for OkapiInferenceAdapter {
    async fn infer(&self, prompt: &str, context: Option<&SoapBundle>) -> Result<InferenceResponse> {
        let start = std::time::Instant::now();

        let url = format!("{}/api/generate", self.endpoint.trim_end_matches('/'));

        // Build full prompt with SOAP context if provided
        let full_prompt = if let Some(soap) = context {
            format!("{}\n\n{}", prompt, soap.to_prompt())
        } else {
            prompt.to_string()
        };

        let body = serde_json::json!({
            "model": self.model,
            "prompt": full_prompt,
            "stream": false,
        });

        debug!(url = %url, model = %self.model, "sending inference request to Okapi");

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                russell_core::error::CoreError::Invariant(format!("HTTP request failed: {e}"))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            warn!(status = %status, body = %body, "Okapi inference failed");
            return Err(russell_core::error::CoreError::Invariant(format!(
                "Okapi returned {}: {}",
                status, body
            )));
        }

        let okapi_response: serde_json::Value = response.json().await.map_err(|e| {
            russell_core::error::CoreError::Invariant(format!("failed to parse response: {e}"))
        })?;

        let text = okapi_response
            .get("response")
            .and_then(|v| v.as_str())
            .unwrap_or("No response from Okapi")
            .to_string();

        let latency_ms = start.elapsed().as_millis() as u64;

        // Parse token usage from Ollama response
        let token_usage = okapi_response
            .get("eval_count")
            .and_then(|v| v.as_u64())
            .map(|output_tokens| {
                let input_tokens = okapi_response
                    .get("prompt_eval_count")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                TokenUsage {
                    input_tokens,
                    output_tokens,
                    total_tokens: input_tokens + output_tokens,
                }
            });

        Ok(InferenceResponse {
            text,
            backend: "okapi".to_string(),
            model: Some(self.model.clone()),
            latency_ms: Some(latency_ms),
            token_usage,
        })
    }

    async fn health_check(&self) -> Result<bool> {
        let url = format!("{}/api/tags", self.endpoint.trim_end_matches('/'));
        match self.client.get(&url).send().await {
            Ok(resp) => Ok(resp.status().is_success()),
            Err(_) => Ok(false),
        }
    }

    fn backend_id(&self) -> &str {
        "okapi"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_creation() {
        let adapter = OkapiInferenceAdapter::new("http://localhost:11434");
        assert_eq!(adapter.backend_id(), "okapi");
        assert_eq!(adapter.model, "llama3");
    }

    #[test]
    fn test_adapter_with_model() {
        let adapter = OkapiInferenceAdapter::new("http://localhost:11434").with_model("mistral");
        assert_eq!(adapter.model, "mistral");
    }
}

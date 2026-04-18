// SPDX-License-Identifier: MIT OR Apache-2.0
//! Shared trait + types for Russell's Doctor clients.
//!
//! Russell does **not** use the `stack-llm` trait. We define a
//! minimal local trait so the client surface matches MVP needs:
//! single round-trip, no streaming, no tool-calling.

use std::future::Future;

use serde::{Deserialize, Serialize};

use crate::error::Result;

/// Backend selector, per `RUSSELL_DOCTOR_BACKEND`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    /// OpenRouter (default if `OPENROUTER_API_KEY` is set).
    OpenRouter,
    /// Local Ollama (OpenAI-compatible at `http://localhost:11434/v1`).
    Ollama,
    /// Mock for tests.
    Mock,
    /// Offline rule-based fallback — never calls the network.
    Offline,
}

impl Backend {
    /// Parse from the environment. Returns the MVP default if
    /// nothing is configured.
    #[must_use]
    pub fn from_env() -> Self {
        match std::env::var("RUSSELL_DOCTOR_BACKEND").ok().as_deref() {
            Some("openrouter") => Self::OpenRouter,
            Some("ollama") => Self::Ollama,
            Some("mock") => Self::Mock,
            Some("offline") => Self::Offline,
            Some(other) => {
                tracing::warn!(backend = other, "unknown backend; using mock");
                Self::Mock
            }
            None => {
                if std::env::var("OPENROUTER_API_KEY").is_ok() {
                    Self::OpenRouter
                } else {
                    Self::Offline
                }
            }
        }
    }

    /// Human-readable label recorded in the journal.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::OpenRouter => "openrouter",
            Self::Ollama => "ollama",
            Self::Mock => "mock",
            Self::Offline => "offline",
        }
    }
}

/// Configuration resolved at call time from env + defaults.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// Backend to use.
    pub backend: Backend,
    /// Model identifier (e.g. `"moonshotai/kimi-k2.5"`).
    pub model: String,
    /// Base URL; `None` = backend's default.
    pub base_url: Option<String>,
    /// Bearer token; `None` = backend does not require one.
    pub api_key: Option<String>,
    /// Request timeout.
    pub timeout: std::time::Duration,
}

impl ClientConfig {
    /// Resolve from the environment, applying MVP defaults
    /// (`moonshotai/kimi-k2.5`, 60s timeout).
    pub fn from_env() -> Self {
        let backend = Backend::from_env();
        let model =
            std::env::var("RUSSELL_DOCTOR_MODEL").unwrap_or_else(|_| "moonshotai/kimi-k2.5".into());
        let base_url = std::env::var("RUSSELL_DOCTOR_BASE_URL").ok();
        let api_key = std::env::var("OPENROUTER_API_KEY").ok();
        Self {
            backend,
            model,
            base_url,
            api_key,
            timeout: std::time::Duration::from_secs(60),
        }
    }
}

/// A SOAP-shaped prompt given to the LLM client.
///
/// See the template at
/// [`docs/templates/soap-bundle.md`](../../../docs/templates/soap-bundle.md).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoapPrompt {
    /// The system prompt (Jack's persona).
    pub system: String,
    /// Subjective — operator note.
    pub subjective: String,
    /// Objective — gathered evidence rendered as Markdown.
    pub objective: String,
    /// The full rendered SOAP text as one user message.
    pub rendered: String,
}

/// The model's response plus a minimum of metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    /// The response text — what the operator sees.
    pub content: String,
    /// Optional model identifier echoed by the provider.
    pub model: Option<String>,
    /// Prompt tokens, if reported.
    pub prompt_tokens: Option<u32>,
    /// Completion tokens, if reported.
    pub completion_tokens: Option<u32>,
    /// Round-trip latency.
    pub latency_ms: u64,
}

/// The client surface — single method, one round-trip.
pub trait LlmClient: Send + Sync {
    /// Send the SOAP prompt and return the model's response.
    fn chat(&self, prompt: &SoapPrompt) -> impl Future<Output = Result<LlmResponse>> + Send;
}

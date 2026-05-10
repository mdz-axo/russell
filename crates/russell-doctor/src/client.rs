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
    /// Local Ollama — the default backend.
    /// OpenAI-compatible at `http://localhost:11434/v1`.
    Ollama,
    /// OpenRouter (opt-in if `OPENROUTER_API_KEY` is set).
    OpenRouter,
    /// Mock for tests.
    Mock,
    /// Offline rule-based fallback — never calls the network.
    Offline,
}

impl Backend {
    /// Parse from the environment. Default is Ollama.
    /// Falls back to Offline if Ollama is not reachable and
    /// the operator has not opted into OpenRouter.
    #[must_use]
    pub fn from_env() -> Self {
        match std::env::var("RUSSELL_DOCTOR_BACKEND").ok().as_deref() {
            Some("openrouter") => Self::OpenRouter,
            Some("ollama") => Self::Ollama,
            Some("mock") => Self::Mock,
            Some("offline") => Self::Offline,
            Some(other) => {
                tracing::warn!(backend = other, "unknown backend; using ollama");
                Self::Ollama
            }
            None => Self::Ollama,
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

/// Minimum severity to trigger LLM escalation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EscalateMin {
    /// Escalate only when crit > 0.
    Crit,
    /// Escalate when crit > 0 OR alert > 0 (default).
    #[default]
    Alert,
    /// Escalate when crit > 0 OR alert > 0 OR warn > 0.
    Warn,
    /// Always escalate (available via `RUSSELL_ESCALATE_MIN=always`; primarily for integration testing).
    Always,
}

impl EscalateMin {
    /// Parse from `RUSSELL_ESCALATE_MIN` env var.
    pub fn from_env() -> Self {
        match std::env::var("RUSSELL_ESCALATE_MIN").ok().as_deref() {
            Some("crit") => Self::Crit,
            Some("warn") => Self::Warn,
            Some("always") => Self::Always,
            Some("alert") | None => Self::Alert,
            Some(other) => {
                tracing::warn!(value = other, "unknown RUSSELL_ESCALATE_MIN; using alert");
                Self::Alert
            }
        }
    }

    /// Returns `true` if the given severity counts meet the threshold.
    pub fn satisfied_by(self, counts: &russell_core::journal::SeverityCounts) -> bool {
        match self {
            Self::Always => true,
            Self::Crit => counts.crit > 0,
            Self::Alert => counts.crit > 0 || counts.alert > 0,
            Self::Warn => counts.crit > 0 || counts.alert > 0 || counts.warn > 0,
        }
    }
}

/// Configuration resolved at call time from env + defaults.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// Backend to use.
    pub backend: Backend,
    /// Model identifier (e.g. `"deepseekv4pro"`).
    pub model: String,
    /// Base URL; `None` = backend's default.
    pub base_url: Option<String>,
    /// Bearer token; `None` = backend does not require one.
    pub api_key: Option<String>,
    /// Request timeout.
    pub timeout: std::time::Duration,
    /// Minimum severity for LLM escalation.
    pub escalate_min: EscalateMin,
}

impl ClientConfig {
    /// Resolve from the environment, applying MVP defaults
    /// (`deepseek-v4-pro:cloud`, 60s timeout, Ollama backend).
    pub fn from_env() -> Self {
        let backend = Backend::from_env();
        let model = std::env::var("RUSSELL_DOCTOR_MODEL")
            .unwrap_or_else(|_| "deepseek-v4-pro:cloud".into());
        let base_url = std::env::var("RUSSELL_DOCTOR_BASE_URL").ok();
        let api_key = std::env::var("OPENROUTER_API_KEY").ok();
        Self {
            backend,
            model,
            base_url,
            api_key,
            timeout: std::time::Duration::from_secs(60),
            escalate_min: EscalateMin::from_env(),
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

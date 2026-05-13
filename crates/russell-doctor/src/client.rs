// SPDX-License-Identifier: MIT OR Apache-2.0
//! LLM client trait and backend config. Okapi-only.
#![allow(missing_docs)]

use std::future::Future;

use serde::{Deserialize, Serialize};

use crate::error::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    /// Local Okapi at `http://localhost:11435/v1`.
    Okapi,
    /// Deterministic test client.
    Mock,
    /// Rule-based fallback; no network.
    Offline,
}

impl Backend {
    /// Parse from the environment. Default is Okapi.
    #[must_use]
    pub fn from_env() -> Self {
        match std::env::var("RUSSELL_DOCTOR_BACKEND").ok().as_deref() {
            Some("okapi") => Self::Okapi,
            // Legacy: accept "ollama" as alias for "okapi".
            Some("ollama") => Self::Okapi,
            Some("mock") => Self::Mock,
            Some("offline") => Self::Offline,
            Some(other) => {
                tracing::warn!(backend = other, "unknown backend; using okapi");
                Self::Okapi
            }
            None => Self::Okapi,
        }
    }

    /// Human-readable label recorded in the journal.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Okapi => "okapi",
            Self::Mock => "mock",
            Self::Offline => "offline",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EscalateMin {
    Crit,
    #[default]
    Alert,
    Warn,
    Always,
}

impl EscalateMin {
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

    pub fn satisfied_by(self, counts: &russell_core::journal::SeverityCounts) -> bool {
        match self {
            Self::Always => true,
            Self::Crit => counts.crit > 0,
            Self::Alert => counts.crit > 0 || counts.alert > 0,
            Self::Warn => counts.crit > 0 || counts.alert > 0 || counts.warn > 0,
        }
    }
}

/// Russell's default model. Shared across all code paths (`russell jack`,
/// `russell chat`). Russell owns its model config; Okapi is just the router.
pub const DEFAULT_MODEL: &str = "nemotron3-super:cloud";

#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub backend: Backend,
    pub model: String,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub timeout: std::time::Duration,
    pub escalate_min: EscalateMin,
}

impl ClientConfig {
    pub fn from_env() -> Self {
        let backend = Backend::from_env();
        let model = std::env::var("RUSSELL_DOCTOR_MODEL")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| DEFAULT_MODEL.to_string());
        let base_url = std::env::var("RUSSELL_DOCTOR_BASE_URL").ok();
        let api_key = std::env::var("RUSSELL_DOCTOR_API_KEY").ok();
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoapPrompt {
    pub system: String,
    pub subjective: String,
    pub objective: String,
    pub rendered: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    pub content: String,
    pub model: Option<String>,
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub latency_ms: u64,
}

pub trait LlmClient: Send + Sync {
    fn chat(&self, prompt: &SoapPrompt) -> impl Future<Output = Result<LlmResponse>> + Send;
}

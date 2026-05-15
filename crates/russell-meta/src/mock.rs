// SPDX-License-Identifier: MIT OR Apache-2.0
//! Mock client for tests.

use crate::client::{LlmClient, LlmResponse, SoapPrompt};
use crate::error::Result;

/// A deterministic client that returns a scripted response.
/// Used in tests and when `RUSSELL_DOCTOR_BACKEND=mock`.
#[derive(Debug, Clone)]
pub struct MockClient {
    reply: String,
}

impl MockClient {
    /// Construct a mock that always replies with `reply`.
    #[must_use]
    pub(crate) fn new(reply: impl Into<String>) -> Self {
        Self {
            reply: reply.into(),
        }
    }

    /// The default mock reply — Jack-voiced.
    #[must_use]
    pub fn jack_default() -> Self {
        Self::new(
            "Mock Jack here. I can't actually call the model, but I can \
             confirm your wiring works: the Nurse composed a SOAP bundle \
             and round-tripped to a client. Ensure Okapi is running \
             (port 11435) and try again for the real thing.",
        )
    }
}

impl LlmClient for MockClient {
    async fn chat(&self, _prompt: &SoapPrompt) -> Result<LlmResponse> {
        Ok(LlmResponse {
            content: self.reply.clone(),
            model: Some("mock".into()),
            prompt_tokens: None,
            completion_tokens: None,
            latency_ms: 0,
        })
    }
}

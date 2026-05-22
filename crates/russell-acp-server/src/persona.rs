// SPDX-License-Identifier: MIT OR Apache-2.0
//! Jack persona projection for ACP sessions.

use crate::error::Result;
use russell_meta::{LlmClient, SoapPrompt, JACK_PERSONA};
use russell_meta::mock::MockClient;
use russell_meta::oai_client::OkapiClient;

/// LLM client enum — avoids dyn trait issues.
#[derive(Clone)]
pub enum LlmClientEnum {
    /// Okapi client for local LLM inference.
    Okapi(OkapiClient),
    /// Mock client for testing.
    Mock(MockClient),
}

impl LlmClient for LlmClientEnum {
    async fn chat(&self, prompt: &SoapPrompt) -> std::result::Result<russell_meta::LlmResponse, russell_meta::error::DoctorError> {
        match self {
            LlmClientEnum::Okapi(client) => client.chat(prompt).await,
            LlmClientEnum::Mock(client) => client.chat(prompt).await,
        }
    }
}

impl std::fmt::Debug for LlmClientEnum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LlmClientEnum::Okapi(_) => f.write_str("LlmClientEnum::Okapi"),
            LlmClientEnum::Mock(_) => f.write_str("LlmClientEnum::Mock"),
        }
    }
}

/// Jack persona projection for ACP.
#[derive(Debug, Clone)]
pub struct JackPersonaProjection {
    /// Persona name.
    name: String,
    /// System prompt.
    system_prompt: String,
    /// LLM client (Okapi by default).
    client: LlmClientEnum,
}

impl JackPersonaProjection {
    /// Create a new Jack persona projection with Okapi client.
    pub async fn new_okapi(config: &russell_meta::ClientConfig) -> Result<Self> {
        let client = OkapiClient::new(config).await
            .map_err(|e| crate::error::AcpError::Config(format!("Okapi client: {}", e)))?;
        Self::new(LlmClientEnum::Okapi(client))
    }

    /// Create a new Jack persona projection with mock client.
    pub fn new_mock() -> Result<Self> {
        Self::new(LlmClientEnum::Mock(MockClient::jack_default()))
    }

    /// Create a new Jack persona projection.
    fn new(client: LlmClientEnum) -> Result<Self> {
        // Use the actual Jack persona prompt from russell-meta.
        let jack_prompt = JACK_PERSONA;
        
        let system_prompt = format!(
            "You are Jack, Russell's nurse persona.\n\n\
             {}\n\n\
             ACP Context:\n\
             - You are interacting via the Agent Client Protocol (ACP)\n\
             - Your conversation partner may be a hKask agent or the operator\n\
             - You observe the host, run probes, and recommend actions\n\
             - You NEVER emit shell commands — you rank intervention IDs\n\
             - You propose interventions; the operator consents; the dispatcher executes\n\n\
             Safety Constraints:\n\
             - Never expose private skills (host mutations, sudo operations)\n\
             - Never reveal Russell's proprioception vitals to external agents\n\
             - Always enforce IDRS (Idempotent/Dry-run/Rollback/Structured-log)\n\
             - When uncertain, fail explicit and ask for clarification",
            jack_prompt
        );

        Ok(Self {
            name: "Jack".to_string(),
            system_prompt,
            client,
        })
    }

    /// Get the system prompt.
    pub fn system_prompt(&self) -> &str {
        &self.system_prompt
    }

    /// Get Jack's name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Generate a response via the LLM.
    pub async fn respond(&self, conversation_history: &str, user_message: &str) -> Result<String> {
        // Build the prompt with conversation context.
        let prompt = SoapPrompt {
            system: self.system_prompt.clone(),
            subjective: format!("Conversation history:\n{}", conversation_history),
            objective: format!("User message: {}", user_message),
            rendered: format!(
                "System: {}\n\nHistory:\n{}\n\nUser: {}\n\nJack:",
                self.system_prompt, conversation_history, user_message
            ),
            temperature: Some(0.7),
            max_tokens: Some(512),
        };

        // Call the LLM.
        let response = self.client.chat(&prompt).await
            .map_err(|e| crate::error::AcpError::DispatchError(format!("LLM call failed: {}", e)))?;
        Ok(response.content)
    }
}

impl Default for JackPersonaProjection {
    fn default() -> Self {
        Self::new_mock().expect("Jack persona should load")
    }
}

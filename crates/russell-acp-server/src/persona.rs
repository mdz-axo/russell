// SPDX-License-Identifier: MIT OR Apache-2.0
//! Jack persona projection for ACP sessions.
//!
//! Forwards to hKask for LLM inference.

use crate::error::Result;
use russell_meta::JACK_PERSONA;

/// Jack persona projection for ACP.
#[derive(Debug, Clone)]
pub struct JackPersonaProjection {
    name: String,
    system_prompt: String,
}

impl JackPersonaProjection {
    /// Create a new Jack persona projection.
    pub fn new() -> Result<Self> {
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
}

impl Default for JackPersonaProjection {
    fn default() -> Self {
        Self::new().expect("Jack persona should load")
    }
}

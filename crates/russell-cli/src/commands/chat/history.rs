// SPDX-License-Identifier: MIT OR Apache-2.0
//! Chat history persistence and types.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// One turn in the conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Turn {
    pub role: String,
    pub content: String,
}

/// Full conversation history (excludes the system prompt which is
/// separate and fixed).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatHistory {
    pub session_id: String,
    pub turns: Vec<Turn>,
}

impl ChatHistory {
    pub fn new(session_id: String) -> Self {
        Self {
            session_id,
            turns: Vec::new(),
        }
    }
}

/// Persist the conversation history to disk.
pub fn save_history(chat_path: &std::path::Path, history: &ChatHistory) -> Result<()> {
    let json = serde_json::to_string(history)?;
    std::fs::write(chat_path.with_extension("json"), &json)
        .with_context(|| format!("writing {}", chat_path.display()))?;
    Ok(())
}

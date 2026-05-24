// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell-api-server` — HTTP REST API for Russell.
//!
//! Provides functionally equivalent access to the same interactive Jack
//! session used by the CLI and ACP surfaces. The API server exposes
//! session create/message/consent/close endpoints that map to the same
//! logical operations as the CLI's `russell chat` and the ACP server's
//! JSON-RPC session methods.

#![deny(unsafe_code)]
#![deny(rust_2018_idioms)]
#![warn(missing_docs)]

pub mod routes;

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use russell_meta::JACK_CHAT_PERSONA;

/// A session — multi-turn conversation with Jack.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiSession {
    /// Session ID.
    pub id: String,
    /// Persona name.
    pub persona: String,
    /// Turn history.
    pub turns: Vec<ApiTurn>,
    /// Creation timestamp.
    pub created: DateTime<Utc>,
    /// Last activity.
    pub last_activity: DateTime<Utc>,
    /// Session state.
    pub state: ApiSessionState,
    /// Pending action.
    pub pending_action: Option<ApiPendingAction>,
}

/// Session state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiSessionState {
    /// Session is active and accepting messages.
    /// Session is active and accepting messages.
    Active,
    /// Session is waiting for operator consent.
    InputRequired,
    /// Session is closed.
    Closed,
}

/// A turn in the conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiTurn {
    /// Turn ID.
    pub id: String,
    /// Role.
    pub role: String,
    /// Content.
    pub content: String,
    /// Timestamp.
    pub timestamp: DateTime<Utc>,
}

/// Pending action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiPendingAction {
    /// Action type.
    pub action_type: String,
    /// Skill ID.
    pub skill_id: String,
    /// Intervention ID.
    pub intervention_id: String,
    /// Risk level.
    pub risk: String,
    /// Action ID.
    pub action_id: String,
}

/// Shared application state.
#[derive(Clone)]
pub struct AppState {
    /// Sessions.
    sessions: Arc<Mutex<HashMap<String, ApiSession>>>,
    /// System prompt.
    system_prompt: String,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    /// Create new app state.
    pub fn new() -> Self {
        let system_prompt = format!(
            "You are Jack, Russell's nurse persona.\n\n\
             {}\n\n\
             API Context:\n\
             - You are interacting via the HTTP REST API\n\
             - You observe the host, run probes, and recommend actions\n\
             - You NEVER emit shell commands — you rank intervention IDs",
            JACK_CHAT_PERSONA
        );
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            system_prompt,
        }
    }

    /// Get the system prompt.
    pub fn system_prompt(&self) -> &str {
        &self.system_prompt
    }

    /// Get sessions map.
    pub fn sessions(&self) -> &Arc<Mutex<HashMap<String, ApiSession>>> {
        &self.sessions
    }
}

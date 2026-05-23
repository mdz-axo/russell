// SPDX-License-Identifier: MIT OR Apache-2.0
//! Session management — multi-turn conversation state.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// ACP session — multi-turn conversation with Jack persona.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Session ID (UUID v4).
    pub id: String,
    /// Persona name.
    pub persona: String,
    /// Turn history.
    pub turns: Vec<Turn>,
    /// Creation timestamp.
    pub created: DateTime<Utc>,
    /// Last activity timestamp.
    pub last_activity: DateTime<Utc>,
    /// Session state.
    pub state: SessionState,
}

impl Session {
    /// Create a new session.
    pub fn new(persona: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            persona: persona.into(),
            turns: Vec::new(),
            created: now,
            last_activity: now,
            state: SessionState::Active,
        }
    }

    /// Add a turn to the session.
    pub fn add_turn(&mut self, turn: Turn) {
        self.last_activity = turn.timestamp;
        self.turns.push(turn);
    }

    /// Get the last N turns.
    pub fn recent_turns(&self, n: usize) -> &[Turn] {
        if self.turns.len() <= n {
            &self.turns
        } else {
            &self.turns[self.turns.len() - n..]
        }
    }

    /// Close the session.
    pub fn close(&mut self) {
        self.state = SessionState::Closed;
        self.last_activity = Utc::now();
    }

    /// Whether the session is active.
    pub fn is_active(&self) -> bool {
        matches!(
            self.state,
            SessionState::Active | SessionState::InputRequired
        )
    }
}

/// Session state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    /// Active conversation.
    Active,
    /// Waiting for operator input/consent.
    InputRequired,
    /// Conversation completed.
    Completed,
    /// Session closed (cleanup pending).
    Closed,
}

/// A turn in the conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Turn {
    /// Turn ID.
    pub id: String,
    /// Role.
    pub role: TurnRole,
    /// Content.
    pub content: String,
    /// Tool calls (if any).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCallRecord>,
    /// Timestamp.
    pub timestamp: DateTime<Utc>,
}

impl Turn {
    /// Create a new turn.
    pub fn new(role: TurnRole, content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            role,
            content: content.into(),
            tool_calls: Vec::new(),
            timestamp: Utc::now(),
        }
    }

    /// Add a tool call record.
    pub fn add_tool_call(&mut self, record: ToolCallRecord) {
        self.tool_calls.push(record);
    }
}

/// Turn role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TurnRole {
    /// Operator or hKask agent.
    User,
    /// Jack persona.
    Assistant,
    /// MCP tool response.
    Tool,
}

/// Tool call record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRecord {
    /// Skill ID.
    pub skill_id: String,
    /// Intervention ID (if intervention).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intervention_id: Option<String>,
    /// Probe ID (if probe).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub probe_id: Option<String>,
    /// Arguments.
    pub args: serde_json::Value,
    /// Result.
    pub result: String,
    /// Visibility (for audit).
    pub visibility: crate::types::Visibility,
}

/// Session manager — holds all active sessions.
#[derive(Debug, Default)]
pub struct SessionManager {
    sessions: std::collections::HashMap<String, Session>,
}

impl SessionManager {
    /// Create a new session manager.
    pub fn new() -> Self {
        Self {
            sessions: std::collections::HashMap::new(),
        }
    }

    /// Create a new session.
    pub fn create_session(&mut self, persona: impl Into<String>) -> String {
        let session = Session::new(persona);
        let id = session.id.clone();
        self.sessions.insert(id.clone(), session);
        id
    }

    /// Get a session by ID.
    pub fn get_session(&self, id: &str) -> Option<&Session> {
        self.sessions.get(id)
    }

    /// Get a mutable session by ID.
    pub fn get_session_mut(&mut self, id: &str) -> Option<&mut Session> {
        self.sessions.get_mut(id)
    }

    /// Close a session.
    pub fn close_session(&mut self, id: &str) -> bool {
        if let Some(session) = self.sessions.get_mut(id) {
            session.close();
            true
        } else {
            false
        }
    }

    /// Remove a closed session.
    pub fn remove_session(&mut self, id: &str) -> bool {
        self.sessions.remove(id).is_some()
    }

    /// Get active session count.
    pub fn active_count(&self) -> usize {
        self.sessions.values().filter(|s| s.is_active()).count()
    }

    /// Cleanup old closed sessions (older than 1 hour).
    pub fn cleanup_old_sessions(&mut self) -> usize {
        let one_hour_ago = Utc::now() - chrono::Duration::hours(1);
        let old_ids: Vec<String> = self
            .sessions
            .iter()
            .filter(|(_, s)| s.state == SessionState::Closed && s.last_activity < one_hour_ago)
            .map(|(id, _)| id.clone())
            .collect();

        let count = old_ids.len();
        for id in old_ids {
            self.sessions.remove(&id);
        }
        count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_creation() {
        let session = Session::new("jack");
        assert_eq!(session.persona, "jack");
        assert_eq!(session.turns.len(), 0);
        assert_eq!(session.state, SessionState::Active);
    }

    #[test]
    fn session_add_turn() {
        let mut session = Session::new("jack");
        session.add_turn(Turn::new(TurnRole::User, "Hello"));
        assert_eq!(session.turns.len(), 1);
        assert_eq!(session.turns[0].role, TurnRole::User);
    }

    #[test]
    fn session_manager_create() {
        let mut manager = SessionManager::new();
        let session_id = manager.create_session("jack");
        assert!(session_id.starts_with("sess_") || session_id.len() == 36); // UUID
    }
}

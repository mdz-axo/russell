// SPDX-License-Identifier: MIT OR Apache-2.0
//! Session engine — core reusable logic for multi-turn Jack sessions.
//!
//! This engine is the single source of truth for session message handling
//! and consent flow. All three surfaces (CLI, API, ACP) delegate to this
//! engine, ensuring functional equivalence.

use chrono::Utc;
use tracing::{info, warn};

use russell_core::inference::{InferencePort, SoapBundle};

use crate::error::{SessionError, SessionResult};
use crate::port::InterventionPort;
use crate::session::{ConsentDecision, Session, SessionManager, SessionState, Turn, TurnRole};
use crate::types::{
    ConsentRequest, ConsentResponse, CreateSessionResponse, SessionMessageResponse, TurnInfo,
    ToolCallSummary,
};

/// Session engine — shared across CLI, API, and ACP surfaces.
pub struct SessionEngine {
    /// Session manager (holds all active sessions).
    sessions: SessionManager,
    /// Jack persona system prompt.
    system_prompt: String,
    /// Inference backend for LLM responses.
    inference: Option<std::sync::Arc<dyn InferencePort>>,
    /// Intervention execution port.
    intervention_port: Option<Box<dyn InterventionPort + Send>>,
}

impl SessionEngine {
    /// Create a new session engine.
    pub fn new(system_prompt: impl Into<String>) -> Self {
        Self {
            sessions: SessionManager::new(),
            system_prompt: system_prompt.into(),
            inference: None,
            intervention_port: None,
        }
    }

    /// Set the inference backend.
    pub fn with_inference(mut self, inference: std::sync::Arc<dyn InferencePort>) -> Self {
        self.inference = Some(inference);
        self
    }

    /// Set the intervention execution port.
    pub fn with_intervention_port(mut self, port: Box<dyn InterventionPort + Send>) -> Self {
        self.intervention_port = Some(port);
        self
    }

    /// Access the session manager (for GC, inspection).
    pub fn sessions_mut(&mut self) -> &mut SessionManager {
        &mut self.sessions
    }

    /// Create a new session.
    pub fn create_session(&mut self, persona: impl Into<String>) -> SessionResult<CreateSessionResponse> {
        let session_id = self.sessions.create_session(persona);
        info!(session_id = %session_id, "Created session");
        Ok(CreateSessionResponse {
            session_id: session_id.clone(),
            created_at: Utc::now().to_rfc3339(),
            persona: "jack".to_string(),
        })
    }

    /// Create a new session bound to a token (ACP).
    pub fn create_session_with_token(
        &mut self,
        persona: impl Into<String>,
        token_id: Option<String>,
    ) -> SessionResult<CreateSessionResponse> {
        let session_id = self.sessions.create_session_with_token(persona, token_id);
        info!(session_id = %session_id, "Created session with token binding");
        Ok(CreateSessionResponse {
            session_id: session_id.clone(),
            created_at: Utc::now().to_rfc3339(),
            persona: "jack".to_string(),
        })
    }

    /// Send a message in a session.
    pub fn send_message(
        &mut self,
        session_id: &str,
        message: &str,
    ) -> SessionResult<SessionMessageResponse> {
        {
            let session = self
                .sessions
                .get_session_mut(session_id)
                .ok_or_else(|| SessionError::SessionNotFound(session_id.to_string()))?;

            if !session.is_active() {
                return Err(SessionError::SessionClosed(session_id.to_string()));
            }

            let user_turn = Turn::new(TurnRole::User, message);
            session.add_turn(user_turn);
        }

        let conversation_history = {
            let session = self
                .sessions
                .get_session(session_id)
                .ok_or_else(|| SessionError::SessionNotFound(session_id.to_string()))?;
            session
                .recent_turns(10)
                .iter()
                .map(|t| format!("{}: {}", role_label(t.role), t.content))
                .collect::<Vec<_>>()
                .join("\n")
        };

        let response = self.generate_response(message, &conversation_history);

        {
            let session = self
                .sessions
                .get_session_mut(session_id)
                .ok_or_else(|| SessionError::SessionNotFound(session_id.to_string()))?;
            let assistant_turn = Turn::new(TurnRole::Assistant, &response);
            session.add_turn(assistant_turn);
        }

        let session = self
            .sessions
            .get_session(session_id)
            .ok_or_else(|| SessionError::SessionNotFound(session_id.to_string()))?;

        Ok(SessionMessageResponse {
            session_id: session.id.clone(),
            response,
            turns: session.recent_turns(10).iter().map(turn_to_info).collect(),
            state: format!("{:?}", session.state),
            pending_action: session.pending_action.clone(),
        })
    }

    /// Respond to a consent request.
    pub fn respond_consent(
        &mut self,
        request: ConsentRequest,
    ) -> SessionResult<ConsentResponse> {
        let session = self
            .sessions
            .get_session_mut(&request.session_id)
            .ok_or_else(|| SessionError::SessionNotFound(request.session_id.clone()))?;

        if session.state != SessionState::InputRequired {
            return Err(SessionError::NotWaitingForConsent(
                request.session_id.clone(),
                session.state,
            ));
        }

        let pending = session
            .pending_action
            .as_ref()
            .ok_or(SessionError::NoPendingAction)?;

        if pending.action_id != request.action_id {
            return Err(SessionError::ActionIdMismatch(
                request.action_id.clone(),
                pending.action_id.clone(),
            ));
        }

        let skill_id = pending.skill_id.clone();
        let intervention_id = pending.intervention_id.clone();
        let args = pending.args.clone();

        let (result, error) = match request.decision {
            ConsentDecision::Approve => {
                info!(
                    session_id = %request.session_id,
                    action_id = %request.action_id,
                    skill_id = %skill_id,
                    intervention_id = %intervention_id,
                    "Consent approved, executing intervention"
                );
                match self.execute_intervention(&skill_id, &intervention_id, &args) {
                    Ok(res) => (Some(res), None),
                    Err(e) => {
                        warn!(error = %e, "Intervention execution failed after approval");
                        (None, Some(e))
                    }
                }
            }
            ConsentDecision::Deny => {
                info!(
                    session_id = %request.session_id,
                    action_id = %request.action_id,
                    reason = ?request.reason,
                    "Consent denied"
                );
                (None, Some("Action denied by operator".to_string()))
            }
        };

        let session = self
            .sessions
            .get_session_mut(&request.session_id)
            .ok_or_else(|| SessionError::SessionNotFound(request.session_id.clone()))?;

        session.pending_action = None;
        session.state = SessionState::Active;
        session.last_activity = Utc::now();

        let decision_text = match request.decision {
            ConsentDecision::Approve => "approved",
            ConsentDecision::Deny => "denied",
        };
        let turn_content = format!(
            "Operator {} intervention {}/{}{}",
            decision_text,
            skill_id,
            intervention_id,
            request.reason
                .as_ref()
                .map(|r| format!(" (reason: {})", r))
                .unwrap_or_default()
        );
        session.add_turn(Turn::new(TurnRole::User, turn_content));

        Ok(ConsentResponse {
            session_id: request.session_id,
            action_id: request.action_id,
            decision: request.decision,
            result,
            error,
        })
    }

    /// Close a session.
    pub fn close_session(&mut self, session_id: &str) -> SessionResult<()> {
        if !self.sessions.close_session(session_id) {
            return Err(SessionError::SessionNotFound(session_id.to_string()));
        }
        info!(session_id = %session_id, "Closed session");
        Ok(())
    }

    /// Get session state (for status queries).
    pub fn get_session(&self, session_id: &str) -> SessionResult<&Session> {
        self.sessions
            .get_session(session_id)
            .ok_or_else(|| SessionError::SessionNotFound(session_id.to_string()))
    }

    /// Verify session ownership (for ACP token binding).
    pub fn verify_session_ownership(&self, session_id: &str, token_id: Option<&str>) -> bool {
        self.sessions.verify_session_ownership(session_id, token_id)
    }

    fn generate_response(&self, message: &str, conversation_history: &str) -> String {
        if let Some(ref inference) = self.inference {
            let prompt = format!(
                "{}\n\nConversation History:\n{}\n\nUser: {}",
                self.system_prompt, conversation_history, message
            );
            let soap = SoapBundle::new(&prompt);
            match tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    inference.infer(&prompt, Some(&soap)).await
                })
            }) {
                Ok(resp) => resp.text,
                Err(e) => {
                    warn!(error = %e, "inference failed, using fallback");
                    format!("[Inference unavailable: {}]", e)
                }
            }
        } else {
            format!(
                "[No inference backend configured. Message received: {}]",
                message
            )
        }
    }

    fn execute_intervention(
        &self,
        skill_id: &str,
        intervention_id: &str,
        args: &serde_json::Value,
    ) -> Result<String, String> {
        if let Some(ref port) = self.intervention_port {
            match tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    port.execute(skill_id, intervention_id, args).await
                })
            }) {
                Ok(res) => Ok(res),
                Err(e) => Err(e),
            }
        } else {
            Err("No intervention port configured".to_string())
        }
    }
}

fn role_label(role: TurnRole) -> &'static str {
    match role {
        TurnRole::User => "User",
        TurnRole::Assistant => "Jack",
        TurnRole::Tool => "Tool",
    }
}

fn turn_to_info(turn: &Turn) -> TurnInfo {
    TurnInfo {
        id: turn.id.clone(),
        role: format!("{:?}", turn.role),
        content: turn.content.clone(),
        timestamp: turn.timestamp.to_rfc3339(),
        tool_calls: turn
            .tool_calls
            .iter()
            .map(|tc| ToolCallSummary {
                skill_id: tc.skill_id.clone(),
                probe_id: tc.probe_id.clone(),
                intervention_id: tc.intervention_id.clone(),
                args: tc.args.clone(),
                result: tc.result.clone(),
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_create_session() {
        let mut engine = SessionEngine::new("You are Jack.");
        let resp = engine.create_session("jack").unwrap();
        assert!(!resp.session_id.is_empty());
        assert_eq!(resp.persona, "jack");
    }

    #[test]
    fn engine_send_message_no_inference() {
        let mut engine = SessionEngine::new("You are Jack.");
        let resp = engine.create_session("jack").unwrap();
        let msg = engine.send_message(&resp.session_id, "Hello").unwrap();
        assert!(msg.response.contains("No inference backend"));
    }

    #[test]
    fn engine_close_session() {
        let mut engine = SessionEngine::new("You are Jack.");
        let resp = engine.create_session("jack").unwrap();
        engine.close_session(&resp.session_id).unwrap();
        let result = engine.send_message(&resp.session_id, "Hello");
        assert!(result.is_err());
    }

    #[test]
    fn engine_session_not_found() {
        let engine = SessionEngine::new("You are Jack.");
        let result = engine.get_session("nonexistent");
        assert!(result.is_err());
    }
}

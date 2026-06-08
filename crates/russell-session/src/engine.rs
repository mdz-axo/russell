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
    ConsentRequest, ConsentResponse, CreateSessionResponse, SessionMessageResponse,
    ToolCallSummary, TurnInfo,
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
    intervention_port: Option<Box<dyn InterventionPort>>,
    /// Operator consent grants (TODO-13: scoped, versioned, expiring).
    consent: Option<russell_core::sovereignty::OperatorConsent>,
}

impl SessionEngine {
    /// Create a new session engine.
    pub fn new(system_prompt: impl Into<String>) -> Self {
        Self {
            sessions: SessionManager::new(),
            system_prompt: system_prompt.into(),
            inference: None,
            intervention_port: None,
            consent: None,
        }
    }

    /// Set the inference backend.
    pub fn with_inference(mut self, inference: std::sync::Arc<dyn InferencePort>) -> Self {
        self.inference = Some(inference);
        self
    }

    /// Set the intervention execution port.
    pub fn with_intervention_port(mut self, port: Box<dyn InterventionPort>) -> Self {
        self.intervention_port = Some(port);
        self
    }

    /// Access the session manager (for GC, inspection).
    pub fn sessions_mut(&mut self) -> &mut SessionManager {
        &mut self.sessions
    }

    /// Grant operator consent for a specific action.
    /// Records the grant with scope, version, and expiry per P2 (Affirmative Consent).
    pub fn grant_consent(
        &mut self,
        action: String,
        grant: russell_core::sovereignty::ConsentGrant,
    ) {
        if self.consent.is_none() {
            self.consent = Some(russell_core::sovereignty::OperatorConsent::new());
        }
        if let Some(ref mut consent) = self.consent {
            consent.grant(action, grant);
        }
    }

    /// Revoke operator consent for a specific action.
    pub fn revoke_consent(&mut self, action: &str) {
        if let Some(ref mut consent) = self.consent {
            consent.revoke(action);
        }
    }

    /// Check whether consent exists for a given action, considering
    /// scope, version, and expiry per P2 (Affirmative Consent).
    pub fn check_consent(
        &self,
        action: &str,
        current_version: Option<&str>,
    ) -> russell_core::sovereignty::ConsentStatus {
        match &self.consent {
            Some(consent) => consent.check_consent(action, current_version),
            None => russell_core::sovereignty::ConsentStatus::Denied,
        }
    }

    /// Resolve consent for a skill/action using hierarchical scope matching.
    ///
    /// P2 (Affirmative Consent): most-specific-wins resolution across all
    /// grants. `PerActionType` > `PerSkill` > `Master`.
    pub fn resolve_consent(
        &self,
        skill_id: &str,
        action_type: &str,
        current_version: Option<&str>,
    ) -> russell_core::sovereignty::ConsentStatus {
        match &self.consent {
            Some(consent) => consent.resolve_consent(skill_id, action_type, current_version),
            None => russell_core::sovereignty::ConsentStatus::Denied,
        }
    }

    /// Check whether an action is pre-approved via hierarchical consent resolution.
    ///
    /// Returns `true` if `resolve_consent` returns `Granted`, meaning the
    /// action can be auto-executed without presenting a consent prompt.
    /// ACP/API surfaces should call this before presenting an intervention
    /// for approval.
    pub fn execute_if_pre_approved(
        &self,
        skill_id: &str,
        action_type: &str,
        current_version: Option<&str>,
    ) -> bool {
        matches!(
            self.resolve_consent(skill_id, action_type, current_version),
            russell_core::sovereignty::ConsentStatus::Granted { .. }
        )
    }

    /// Create a new session.
    pub fn create_session(
        &mut self,
        persona: impl Into<String>,
    ) -> SessionResult<CreateSessionResponse> {
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
    pub fn respond_consent(&mut self, request: ConsentRequest) -> SessionResult<ConsentResponse> {
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

                // Record the consent grant with scope, version, and expiry (P2).
                let action_key = format!("{}/{}", skill_id, intervention_id);
                let grant = russell_core::sovereignty::ConsentGrant {
                    categories: std::collections::HashSet::new(),
                    resource_version: None,
                    expires_at: Some(chrono::Utc::now() + chrono::Duration::seconds(300)),
                    scope: russell_core::sovereignty::ConsentScope::PerSkill {
                        skill_id: skill_id.clone(),
                    },
                    granted_at: chrono::Utc::now(),
                };
                self.grant_consent(action_key, grant);

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
            request
                .reason
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
            let inference = inference.clone();

            let (tx, rx) = tokio::sync::oneshot::channel();
            tokio::runtime::Handle::current().spawn(async move {
                let result = inference.infer(&prompt, Some(&soap)).await;
                let _ = tx.send(result);
            });

            match tokio::task::block_in_place(|| rx.blocking_recv()) {
                Ok(Ok(resp)) => resp.text,
                Ok(Err(e)) => {
                    warn!(error = %e, "inference failed, using fallback");
                    format!("[Inference unavailable: {}]", e)
                }
                Err(_) => {
                    warn!("inference oneshot channel closed unexpectedly");
                    "[Inference unavailable: channel closed]".to_string()
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
                tokio::runtime::Handle::current()
                    .block_on(async { port.execute(skill_id, intervention_id, args).await })
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

    // ── Consent tests (TODO-13) ──────────────────────────────────────

    #[test]
    fn engine_consent_default_is_deny() {
        let engine = SessionEngine::new("You are Jack.");
        let status = engine.check_consent("test/action", None);
        assert!(
            matches!(status, russell_core::sovereignty::ConsentStatus::Denied),
            "consent should be denied by default (P2: fail-closed)"
        );
    }

    #[test]
    fn engine_grant_and_check_consent() {
        use russell_core::sovereignty::{ConsentGrant, ConsentScope};
        use std::collections::HashSet;

        let mut engine = SessionEngine::new("You are Jack.");
        let grant = ConsentGrant {
            categories: HashSet::new(),
            resource_version: Some("v1".to_string()),
            expires_at: Some(chrono::Utc::now() + chrono::Duration::hours(1)),
            scope: ConsentScope::PerSkill {
                skill_id: "test-skill".to_string(),
            },
            granted_at: chrono::Utc::now(),
        };
        engine.grant_consent("test-skill/action".to_string(), grant);

        let status = engine.check_consent("test-skill/action", None);
        assert!(
            matches!(
                status,
                russell_core::sovereignty::ConsentStatus::Granted { .. }
            ),
            "consent should be granted after explicit grant"
        );
    }

    #[test]
    fn engine_consent_version_mismatch() {
        use russell_core::sovereignty::{ConsentGrant, ConsentScope, ConsentStatus};
        use std::collections::HashSet;

        let mut engine = SessionEngine::new("You are Jack.");
        let grant = ConsentGrant {
            categories: HashSet::new(),
            resource_version: Some("v1".to_string()),
            expires_at: None,
            scope: ConsentScope::Master,
            granted_at: chrono::Utc::now(),
        };
        engine.grant_consent("test/action".to_string(), grant);

        // Same version → Granted
        let status = engine.check_consent("test/action", Some("v1"));
        assert!(matches!(status, ConsentStatus::Granted { .. }));

        // Different version → VersionMismatch
        let status = engine.check_consent("test/action", Some("v2"));
        assert!(matches!(status, ConsentStatus::VersionMismatch { .. }));
    }

    #[test]
    fn engine_revoke_consent() {
        use russell_core::sovereignty::{ConsentGrant, ConsentScope, ConsentStatus};
        use std::collections::HashSet;

        let mut engine = SessionEngine::new("You are Jack.");
        let grant = ConsentGrant {
            categories: HashSet::new(),
            resource_version: None,
            expires_at: None,
            scope: ConsentScope::Master,
            granted_at: chrono::Utc::now(),
        };
        engine.grant_consent("test/action".to_string(), grant);

        // Granted
        let status = engine.check_consent("test/action", None);
        assert!(matches!(status, ConsentStatus::Granted { .. }));

        // Revoke
        engine.revoke_consent("test/action");
        let status = engine.check_consent("test/action", None);
        assert!(matches!(status, ConsentStatus::Denied));
    }

    #[test]
    fn engine_resolve_consent_hierarchical() {
        use russell_core::sovereignty::{ConsentGrant, ConsentScope, ConsentStatus};
        use std::collections::HashSet;

        let mut engine = SessionEngine::new("You are Jack.");

        // Grant Master scope
        let master_grant = ConsentGrant {
            categories: HashSet::new(),
            resource_version: None,
            expires_at: None,
            scope: ConsentScope::Master,
            granted_at: chrono::Utc::now(),
        };
        engine.grant_consent("master".to_string(), master_grant);

        // Grant PerSkill scope
        let skill_grant = ConsentGrant {
            categories: HashSet::new(),
            resource_version: None,
            expires_at: None,
            scope: ConsentScope::PerSkill {
                skill_id: "sysadmin".to_string(),
            },
            granted_at: chrono::Utc::now(),
        };
        engine.grant_consent("sysadmin/grant".to_string(), skill_grant);

        // resolve_consent should find the most specific grant
        let status = engine.resolve_consent("sysadmin", "action", None);
        assert!(matches!(
            status,
            ConsentStatus::Granted {
                scope: ConsentScope::PerSkill { .. }
            }
        ));

        // For a different skill, Master should apply
        let status = engine.resolve_consent("other-skill", "action", None);
        assert!(matches!(
            status,
            ConsentStatus::Granted {
                scope: ConsentScope::Master
            }
        ));
    }

    #[test]
    fn engine_execute_if_pre_approved() {
        use russell_core::sovereignty::{ConsentGrant, ConsentScope};
        use std::collections::HashSet;

        let mut engine = SessionEngine::new("You are Jack.");

        // No consent granted — not pre-approved
        assert!(!engine.execute_if_pre_approved("sysadmin", "intervention", None));

        // Grant PerSkill consent for sysadmin
        let grant = ConsentGrant {
            categories: HashSet::new(),
            resource_version: None,
            expires_at: Some(chrono::Utc::now() + chrono::Duration::hours(1)),
            scope: ConsentScope::PerSkill {
                skill_id: "sysadmin".to_string(),
            },
            granted_at: chrono::Utc::now(),
        };
        engine.grant_consent("sysadmin/restart".to_string(), grant);

        // Same skill → pre-approved
        assert!(engine.execute_if_pre_approved("sysadmin", "intervention", None));

        // Different skill → not pre-approved
        assert!(!engine.execute_if_pre_approved("other-skill", "intervention", None));
    }
}

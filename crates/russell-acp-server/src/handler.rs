// SPDX-License-Identifier: MIT OR Apache-2.0
//! ACP handler — processes JSON-RPC requests.

use chrono::Utc;
use serde_json::json;
use tracing::{debug, info, warn};

use crate::CapabilityToken;
use crate::SessionState;
use crate::auth::MacaroonAuth;
use crate::cns::CnsPort;
use crate::error::{AcpError, Result};
use crate::persona::JackPersonaProjection;
use crate::port::SkillDispatchPort;
use crate::rate_limit::RateLimiter;
use crate::session::{SessionManager, Turn, TurnRole};
use crate::types::*;

/// ACP handler — processes JSON-RPC requests.
pub struct AcpHandler {
    /// Session manager.
    sessions: SessionManager,
    /// Jack persona.
    #[allow(dead_code)]
    persona: JackPersonaProjection,
    /// Skill dispatch port (hexagonal).
    dispatch: Box<dyn SkillDispatchPort>,
    /// Macaroon auth.
    auth: MacaroonAuth,
    /// Rate limiter.
    rate_limiter: RateLimiter,
    /// Whether authentication is required for all requests.
    require_auth: bool,
    /// Inference backend for LLM responses (T6).
    inference: Option<std::sync::Arc<dyn russell_core::inference::InferencePort>>,
    /// Journal reader for proprioception notifications (T2-2).
    journal_reader: Option<russell_core::journal::JournalReader>,
    /// CNS span emitter for observability (T2-3).
    cns: Option<Box<dyn CnsPort>>,
}

impl AcpHandler {
    /// Create a new ACP handler.
    pub fn new(
        persona: JackPersonaProjection,
        dispatch: impl SkillDispatchPort + 'static,
        auth: MacaroonAuth,
        rate_limiter: RateLimiter,
    ) -> Self {
        Self {
            sessions: SessionManager::new(),
            persona,
            dispatch: Box::new(dispatch),
            auth,
            rate_limiter,
            require_auth: true,
            inference: None,
            journal_reader: None,
            cns: None,
        }
    }

    /// Set the inference backend for LLM responses.
    pub fn with_inference(
        mut self,
        inference: std::sync::Arc<dyn russell_core::inference::InferencePort>,
    ) -> Self {
        self.inference = Some(inference);
        self
    }

    /// Set the journal reader for proprioception notifications (T2-2).
    pub fn with_journal_reader(mut self, reader: russell_core::journal::JournalReader) -> Self {
        self.journal_reader = Some(reader);
        self
    }

    /// Set the CNS span emitter for observability (T2-3).
    pub fn with_cns(mut self, cns: impl CnsPort + 'static) -> Self {
        self.cns = Some(Box::new(cns));
        self
    }

    /// Set whether authentication is required for all requests.
    pub fn with_require_auth(mut self, require: bool) -> Self {
        self.require_auth = require;
        self
    }

    /// Access the session manager (for GC from transport layer).
    pub fn sessions_mut(&mut self) -> &mut SessionManager {
        &mut self.sessions
    }

    /// Handle a JSON-RPC request.
    pub async fn handle(&mut self, request: JsonRpcRequest) -> JsonRpcResponse {
        debug!(method = %request.method, "Handling ACP request");

        if self.require_auth && request.auth.is_none() {
            return JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: None,
                error: Some(JsonRpcError::from(AcpError::AuthFailed(
                    "authentication required".to_string(),
                ))),
            };
        }

        // Check rate limit (if auth provided).
        if let Some(ref auth) = request.auth
            && let Err(e) = self.rate_limiter.check(&auth.token)
        {
            return JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: None,
                error: Some(JsonRpcError::from(e)),
            };
        }

        // Validate auth (if provided).
        let validated_token = if let Some(ref auth_info) = request.auth {
            match self.validate_auth(auth_info) {
                Ok(token) => Some(token),
                Err(e) => {
                    return JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: request.id,
                        result: None,
                        error: Some(JsonRpcError::from(e)),
                    };
                }
            }
        } else {
            None
        };

        // Dispatch to method handler.
        let result = match request.method.as_str() {
            "acp/session.create" => {
                self.create_session(request.params, validated_token.as_ref())
                    .await
            }
            "acp/session.message" => {
                self.session_message(request.params, validated_token.as_ref())
                    .await
            }
            "acp/session.close" => {
                self.close_session(request.params, validated_token.as_ref())
                    .await
            }
            "acp/session.status" => {
                self.session_status(request.params, validated_token.as_ref())
                    .await
            }
            "acp/consent.respond" => {
                self.consent_respond(request.params, validated_token.as_ref())
                    .await
            }
            "acp/capabilities" => self.get_capabilities(request.params).await,
            "acp/notifications.list" => self.list_notifications(request.params).await,
            "acp/skill/info" => self.get_skill_info(request.params).await,
            "acp/skill/run" => self.run_skill(request.params).await,
            "acp/probe/run" => self.run_probe(request.params).await,
            _ => Err(AcpError::InvalidRequest(format!(
                "unknown method: {}",
                request.method
            ))),
        };

        match result {
            Ok(value) => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: Some(value),
                error: None,
            },
            Err(e) => {
                warn!(error = %e, "ACP request failed");
                JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: None,
                    error: Some(JsonRpcError::from(e)),
                }
            }
        }
    }

    /// Validate authentication info and return the validated token.
    fn validate_auth(&self, auth_info: &AuthInfo) -> Result<CapabilityToken> {
        if auth_info.auth_type != "macaroon" {
            return Err(AcpError::AuthFailed(format!(
                "unknown auth type: {}",
                auth_info.auth_type
            )));
        }

        let token = self.auth.decode_wire_token(&auth_info.token)?;
        self.auth.validate(&token)?;
        Ok(token)
    }

    /// Create a new session.
    async fn create_session(
        &mut self,
        params: Option<serde_json::Value>,
        token: Option<&CapabilityToken>,
    ) -> Result<serde_json::Value> {
        let req: CreateSessionRequest = params
            .map(serde_json::from_value)
            .unwrap_or_else(|| {
                Ok(CreateSessionRequest {
                    persona: "jack".to_string(),
                })
            })
            .map_err(|e| AcpError::InvalidRequest(format!("invalid params: {}", e)))?;

        let token_id = token.map(|t| t.token_id.clone());
        let session_id = self
            .sessions
            .create_session_with_token(&req.persona, token_id);
        info!(session_id = %session_id, "Created ACP session");

        // Emit CNS span for session creation (T2-3)
        if let Some(ref cns) = self.cns {
            cns.emit_session_created(&session_id, &req.persona);
        }

        Ok(json!(CreateSessionResponse {
            session_id: session_id.clone(),
            created_at: Utc::now().to_rfc3339(),
            persona: req.persona,
        }))
    }

    /// Send a message in a session.
    async fn session_message(
        &mut self,
        params: Option<serde_json::Value>,
        token: Option<&CapabilityToken>,
    ) -> Result<serde_json::Value> {
        let req: SessionMessageRequest = params
            .ok_or_else(|| AcpError::InvalidRequest("missing params".to_string()))
            .and_then(|p| {
                serde_json::from_value(p)
                    .map_err(|e| AcpError::InvalidRequest(format!("invalid params: {}", e)))
            })?;

        let session_id = req.session_id.clone();

        if !self
            .sessions
            .verify_session_ownership(&session_id, token.map(|t| t.token_id.as_str()))
        {
            return Err(AcpError::CapabilityNotGranted(format!(
                "session '{session_id}' not owned by this token"
            )));
        }

        let session = self
            .sessions
            .get_session_mut(&session_id)
            .ok_or_else(|| AcpError::SessionNotFound(session_id.clone()))?;

        if !session.is_active() {
            return Err(AcpError::SessionClosed(req.session_id.clone()));
        }

        // Add user turn.
        let user_turn = Turn::new(TurnRole::User, &req.message);
        session.add_turn(user_turn);

        // Build conversation context from recent turns.
        let conversation_history = session
            .recent_turns(10)
            .iter()
            .map(|t| format!("{}: {}", role_label(t.role), t.content))
            .collect::<Vec<_>>()
            .join("\n");

        // Generate Jack's response using inference backend if available.
        let response = if let Some(ref inference) = self.inference {
            let prompt = format!(
                "{}\n\nConversation History:\n{}\n\nUser: {}",
                self.persona.system_prompt(),
                conversation_history,
                req.message
            );
            let soap = russell_core::inference::SoapBundle::new(&prompt);
            match inference.infer(&prompt, Some(&soap)).await {
                Ok(resp) => {
                    // Emit CNS span for LLM escalation (T2-3)
                    if let Some(ref cns) = self.cns {
                        cns.emit_llm_escalation(
                            &resp.backend,
                            resp.model.as_deref(),
                            resp.latency_ms.unwrap_or(0),
                        );
                    }
                    resp.text
                }
                Err(e) => {
                    warn!(error = %e, "inference failed, using fallback");
                    format!("[Inference unavailable: {}]", e)
                }
            }
        } else {
            format!(
                "[No inference backend configured. Message: {}]",
                req.message
            )
        };

        // Add assistant turn.
        let assistant_turn = Turn::new(TurnRole::Assistant, &response);
        session.add_turn(assistant_turn);

        Ok(json!(SessionMessageResponse {
            session_id: session.id.clone(),
            response,
            turns: session.recent_turns(10).iter().map(turn_to_info).collect(),
            state: format!("{:?}", session.state),
            pending_action: None, // Interventions require consent via upstream hKask workflow
        }))
    }

    /// Close a session.
    async fn close_session(
        &mut self,
        params: Option<serde_json::Value>,
        token: Option<&CapabilityToken>,
    ) -> Result<serde_json::Value> {
        let session_id = params
            .and_then(|p| {
                p.get("session_id")
                    .and_then(|v| v.as_str())
                    .map(String::from)
            })
            .ok_or_else(|| AcpError::InvalidRequest("missing session_id".to_string()))?;

        if !self
            .sessions
            .verify_session_ownership(&session_id, token.map(|t| t.token_id.as_str()))
        {
            return Err(AcpError::CapabilityNotGranted(format!(
                "session '{session_id}' not owned by this token"
            )));
        }

        if !self.sessions.close_session(&session_id) {
            return Err(AcpError::SessionNotFound(session_id.clone()));
        }

        info!(session_id = %session_id, "Closed ACP session");
        Ok(json!({"session_id": session_id, "closed_at": Utc::now().to_rfc3339()}))
    }

    /// Get session status.
    async fn session_status(
        &mut self,
        params: Option<serde_json::Value>,
        token: Option<&CapabilityToken>,
    ) -> Result<serde_json::Value> {
        let session_id = params
            .and_then(|p| {
                p.get("session_id")
                    .and_then(|v| v.as_str())
                    .map(String::from)
            })
            .ok_or_else(|| AcpError::InvalidRequest("missing session_id".to_string()))?;

        if !self
            .sessions
            .verify_session_ownership(&session_id, token.map(|t| t.token_id.as_str()))
        {
            return Err(AcpError::CapabilityNotGranted(format!(
                "session '{session_id}' not owned by this token"
            )));
        }

        let session = self
            .sessions
            .get_session(&session_id)
            .ok_or_else(|| AcpError::SessionNotFound(session_id.clone()))?;

        Ok(json!({
            "session_id": session.id,
            "turn_count": session.turns.len(),
            "last_activity": session.last_activity.to_rfc3339(),
            "persona": session.persona,
            "state": format!("{:?}", session.state),
        }))
    }

    /// Get capabilities (public skills + probes).
    async fn get_capabilities(
        &self,
        _params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        let skills = self.dispatch.load_public_skills();
        let probes = self.dispatch.list_probes();

        Ok(json!(CapabilitiesResponse { skills, probes }))
    }

    /// Get skill info.
    async fn get_skill_info(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let skill_id = params
            .and_then(|p| p.get("skill_id").and_then(|v| v.as_str()).map(String::from))
            .ok_or_else(|| AcpError::InvalidRequest("missing skill_id".to_string()))?;

        let skill = self
            .dispatch
            .get_skill_info(&skill_id)
            .ok_or_else(|| AcpError::SkillNotFound(skill_id.clone()))?;

        Ok(json!(skill))
    }

    /// Run a skill.
    async fn run_skill(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let (skill_id, args) = params
            .ok_or_else(|| AcpError::InvalidRequest("missing params".to_string()))
            .and_then(|p| {
                let skill_id = p
                    .get("skill_id")
                    .and_then(|v| v.as_str())
                    .map(String::from)
                    .ok_or_else(|| AcpError::InvalidRequest("missing skill_id".to_string()))?;
                let args = p.get("args").cloned().unwrap_or(json!({}));
                Ok((skill_id, args))
            })?;

        let result = self.dispatch.dispatch_skill(&skill_id, &args).await?;

        // Emit CNS span for skill dispatch (T2-3)
        if let Some(ref cns) = self.cns {
            cns.emit_skill_dispatched(&skill_id, "run");
        }

        Ok(json!({"result": result}))
    }

    /// Run a probe.
    async fn run_probe(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let (skill_id, probe_id, args) = params
            .ok_or_else(|| AcpError::InvalidRequest("missing params".to_string()))
            .and_then(|p| {
                let skill_id = p
                    .get("skill_id")
                    .and_then(|v| v.as_str())
                    .map(String::from)
                    .ok_or_else(|| AcpError::InvalidRequest("missing skill_id".to_string()))?;
                let probe_id = p
                    .get("probe_id")
                    .and_then(|v| v.as_str())
                    .map(String::from)
                    .ok_or_else(|| AcpError::InvalidRequest("missing probe_id".to_string()))?;
                let args = p.get("args").cloned().unwrap_or(json!({}));
                Ok((skill_id, probe_id, args))
            })?;

        let result = self.dispatch.run_probe(&skill_id, &probe_id, &args).await?;
        Ok(json!({"result": result}))
    }

    /// Respond to a consent request (approve/deny pending action).
    async fn consent_respond(
        &mut self,
        params: Option<serde_json::Value>,
        token: Option<&CapabilityToken>,
    ) -> Result<serde_json::Value> {
        let req: ConsentRequest = params
            .ok_or_else(|| AcpError::InvalidRequest("missing params".to_string()))
            .and_then(|p| {
                serde_json::from_value(p)
                    .map_err(|e| AcpError::InvalidRequest(format!("invalid params: {}", e)))
            })?;

        // Verify session ownership
        if !self
            .sessions
            .verify_session_ownership(&req.session_id, token.map(|t| t.token_id.as_str()))
        {
            return Err(AcpError::CapabilityNotGranted(format!(
                "session '{}' not owned by this token",
                req.session_id
            )));
        }

        // Get session and verify it's waiting for consent
        let session = self
            .sessions
            .get_session_mut(&req.session_id)
            .ok_or_else(|| AcpError::SessionNotFound(req.session_id.clone()))?;

        if session.state != SessionState::InputRequired {
            return Err(AcpError::InvalidRequest(format!(
                "session '{}' is not waiting for consent (state: {:?})",
                req.session_id, session.state
            )));
        }

        // Verify action_id matches pending action
        let pending = session
            .pending_action
            .as_ref()
            .ok_or_else(|| AcpError::InvalidRequest("no pending action in session".to_string()))?;

        if pending.action_id != req.action_id {
            return Err(AcpError::InvalidRequest(format!(
                "action_id '{}' does not match pending action '{}'",
                req.action_id, pending.action_id
            )));
        }

        // Extract action details before consuming pending
        let skill_id = pending.skill_id.clone();
        let intervention_id = pending.intervention_id.clone();
        let args = pending.args.clone();

        // Process decision
        let (result, error) = match req.decision {
            ConsentDecision::Approve => {
                info!(
                    session_id = %req.session_id,
                    action_id = %req.action_id,
                    skill_id = %skill_id,
                    intervention_id = %intervention_id,
                    "Consent approved, executing intervention"
                );

                match self.dispatch.dispatch_skill(&skill_id, &args).await {
                    Ok(res) => (Some(res), None),
                    Err(e) => {
                        warn!(error = %e, "Intervention execution failed after approval");
                        (None, Some(e.to_string()))
                    }
                }
            }
            ConsentDecision::Deny => {
                info!(
                    session_id = %req.session_id,
                    action_id = %req.action_id,
                    reason = ?req.reason,
                    "Consent denied"
                );
                (None, Some("Action denied by operator".to_string()))
            }
        };

        // Clear pending action and return to active state
        session.pending_action = None;
        session.state = SessionState::Active;
        session.last_activity = Utc::now();

        // Add turn recording the consent decision
        let decision_text = match req.decision {
            ConsentDecision::Approve => "approved",
            ConsentDecision::Deny => "denied",
        };
        let turn_content = format!(
            "Operator {} intervention {}/{}{}",
            decision_text,
            skill_id,
            intervention_id,
            req.reason
                .as_ref()
                .map(|r| format!(" (reason: {})", r))
                .unwrap_or_default()
        );
        session.add_turn(Turn::new(TurnRole::User, turn_content));

        // Emit CNS span for consent decision (T2-3)
        if let Some(ref cns) = self.cns {
            cns.emit_consent_decision(&req.action_id, decision_text);
        }

        Ok(json!(ConsentResponse {
            session_id: req.session_id,
            action_id: req.action_id,
            decision: req.decision,
            result,
            error,
        }))
    }

    /// List recent proprioception notifications (T2-2).
    ///
    /// Queries the journal for `self_vital_breach` events in the last N hours
    /// (default: 24h, max: 168h/7 days). Returns structured notifications
    /// that hKask agents can surface to the operator.
    async fn list_notifications(
        &self,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        let hours = params
            .as_ref()
            .and_then(|p| p.get("hours"))
            .and_then(|v| v.as_i64())
            .unwrap_or(24)
            .min(168); // Cap at 7 days

        let reader = self
            .journal_reader
            .as_ref()
            .ok_or_else(|| AcpError::Internal("journal reader not configured".into()))?;

        let since_unix = russell_core::time::now_unix() - (hours * 3600);

        // Query self_vital_breach events from the journal
        let events = reader
            .list_events_by_action("self_vital_breach", since_unix, i64::MAX)
            .map_err(|e| AcpError::Internal(format!("journal query failed: {e}")))?;

        let notifications: Vec<ProprioNotification> = events
            .iter()
            .filter_map(|row| {
                let summary = row.summary.as_deref()?;
                // Parse the summary to extract vital name and value
                // Format: "<vital> = <value> (threshold: <threshold>)"
                let parts: Vec<&str> = summary.splitn(2, " = ").collect();
                if parts.len() < 2 {
                    return None;
                }
                let vital = parts[0].trim().to_string();
                let value_str = parts[1].split(" (threshold: ").next().unwrap_or("?");
                let threshold_str = parts[1]
                    .split("threshold: ")
                    .nth(1)
                    .and_then(|s| s.strip_suffix(')'))
                    .unwrap_or("?");

                let value = serde_json::Value::String(value_str.to_string());
                let threshold = serde_json::Value::String(threshold_str.to_string());

                Some(ProprioNotification {
                    id: row.id.clone(),
                    vital,
                    severity: format!("{:?}", row.severity).to_lowercase(),
                    value,
                    threshold,
                    summary: summary.to_string(),
                    timestamp: row.ts.clone(),
                })
            })
            .collect();

        let total = notifications.len();

        Ok(json!(NotificationsResponse {
            notifications,
            total,
        }))
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

impl From<crate::error::AcpError> for JsonRpcError {
    fn from(err: crate::error::AcpError) -> Self {
        let (code, data) = match &err {
            AcpError::SessionNotFound(_) => (404, None),
            AcpError::SessionClosed(_) => (400, None),
            AcpError::SkillNotExposed(_) => (403, Some(json!({"visibility": "private"}))),
            AcpError::SkillNotFound(_) => (404, None),
            AcpError::ProbeNotFound(_) => (404, None),
            AcpError::AuthFailed(_) => (401, None),
            AcpError::TokenExpired(_) => (401, Some(json!({"expired": true}))),
            AcpError::CapabilityNotGranted(_) => (403, Some(json!({"capability": "not_granted"}))),
            AcpError::RateLimitExceeded(_) => (429, Some(json!({"retry_after": 60}))),
            AcpError::InvalidRequest(_) => (400, None),
            _ => (500, None),
        };

        JsonRpcError {
            code,
            message: err.to_string(),
            data,
        }
    }
}

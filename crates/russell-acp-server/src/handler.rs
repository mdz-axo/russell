// SPDX-License-Identifier: MIT OR Apache-2.0
//! ACP handler — processes JSON-RPC requests.
//!
//! Delegates session logic to `russell_session::SessionEngine` so that
//! ACP, CLI, and API surfaces are functionally equivalent.

use chrono::Utc;
use serde_json::json;
use tracing::{debug, info, warn};

use crate::CapabilityToken;
use crate::auth::MacaroonAuth;
use crate::cns::CnsPort;
use crate::error::{AcpError, Result};
use crate::persona::JackPersonaProjection;
use crate::port::SkillDispatchPort;
use crate::rate_limit::RateLimiter;
use crate::types::*;
use russell_session::{
    ConsentDecision, ConsentRequest, SessionEngine, SessionState, Turn, TurnRole,
};

/// ACP-specific intervention port adapter.
struct AcpInterventionAdapter {
    dispatch: std::sync::Arc<dyn SkillDispatchPort + Send + Sync>,
}

#[async_trait::async_trait]
impl russell_session::InterventionPort for AcpInterventionAdapter {
    async fn execute(
        &self,
        skill_id: &str,
        _intervention_id: &str,
        args: &serde_json::Value,
    ) -> Result<String, String> {
        self.dispatch
            .dispatch_skill(skill_id, args)
            .await
            .map_err(|e| e.to_string())
    }
}

/// ACP handler — processes JSON-RPC requests.
pub struct AcpHandler {
    /// Session engine (shared logic across all surfaces).
    engine: SessionEngine,
    /// Jack persona (for system prompt).
    #[allow(dead_code)]
    persona: JackPersonaProjection,
    /// Skill dispatch port (hexagonal).
    dispatch: std::sync::Arc<dyn SkillDispatchPort + Send + Sync>,
    /// Macaroon auth.
    auth: MacaroonAuth,
    /// Rate limiter.
    rate_limiter: RateLimiter,
    /// Whether authentication is required for all requests.
    require_auth: bool,
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
        let dispatch_arc: std::sync::Arc<dyn SkillDispatchPort + Send + Sync> =
            std::sync::Arc::new(dispatch);
        let intervention_adapter: Box<dyn russell_session::InterventionPort + Send> =
            Box::new(AcpInterventionAdapter {
                dispatch: std::sync::Arc::clone(&dispatch_arc),
            });

        let engine = SessionEngine::new(persona.system_prompt())
            .with_intervention_port(intervention_adapter);

        Self {
            engine,
            persona,
            dispatch: dispatch_arc,
            auth,
            rate_limiter,
            require_auth: true,
            journal_reader: None,
            cns: None,
        }
    }

    /// Set the inference backend for LLM responses.
    pub fn with_inference(
        mut self,
        inference: std::sync::Arc<dyn russell_core::inference::InferencePort>,
    ) -> Self {
        self.engine = self.engine.with_inference(inference);
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
    pub fn sessions_mut(&mut self) -> &mut russell_session::SessionManager {
        self.engine.sessions_mut()
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
            "acp/skill.run" => self.run_skill(request.params).await,
            "acp/probe.run" => self.run_probe(request.params).await,
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

    fn validate_auth(&self, auth_info: &AuthInfo) -> crate::error::Result<CapabilityToken> {
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
        let resp = self
            .engine
            .create_session_with_token(&req.persona, token_id)
            .map_err(|e| AcpError::InvalidRequest(e.to_string()))?;

        info!(session_id = %resp.session_id, "Created ACP session");

        if let Some(ref cns) = self.cns {
            cns.emit_session_created(&resp.session_id, &req.persona);
        }

        Ok(json!(resp))
    }

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

        if !self
            .engine
            .verify_session_ownership(&req.session_id, token.map(|t| t.token_id.as_str()))
        {
            return Err(AcpError::CapabilityNotGranted(format!(
                "session '{}' not owned by this token",
                req.session_id
            )));
        }

        let resp = self
            .engine
            .send_message(&req.session_id, &req.message)
            .map_err(|e| match e {
                russell_session::SessionError::SessionNotFound(id) => {
                    AcpError::SessionNotFound(id)
                }
                russell_session::SessionError::SessionClosed(id) => AcpError::SessionClosed(id),
                _ => AcpError::InvalidRequest(e.to_string()),
            })?;

        Ok(json!(resp))
    }

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
            .engine
            .verify_session_ownership(&session_id, token.map(|t| t.token_id.as_str()))
        {
            return Err(AcpError::CapabilityNotGranted(format!(
                "session '{session_id}' not owned by this token"
            )));
        }

        self.engine
            .close_session(&session_id)
            .map_err(|e| AcpError::InvalidRequest(e.to_string()))?;

        info!(session_id = %session_id, "Closed ACP session");
        Ok(json!({"session_id": session_id, "closed_at": Utc::now().to_rfc3339()}))
    }

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
            .engine
            .verify_session_ownership(&session_id, token.map(|t| t.token_id.as_str()))
        {
            return Err(AcpError::CapabilityNotGranted(format!(
                "session '{session_id}' not owned by this token"
            )));
        }

        let session = self
            .engine
            .get_session(&session_id)
            .map_err(|e| AcpError::InvalidRequest(e.to_string()))?;

        Ok(json!({
            "session_id": session.id,
            "turn_count": session.turns.len(),
            "last_activity": session.last_activity.to_rfc3339(),
            "persona": session.persona,
            "state": format!("{:?}", session.state),
        }))
    }

    async fn consent_respond(
        &mut self,
        params: Option<serde_json::Value>,
        token: Option<&CapabilityToken>,
    ) -> Result<serde_json::Value> {
        let raw_req: ConsentRequest = params
            .ok_or_else(|| AcpError::InvalidRequest("missing params".to_string()))
            .and_then(|p| {
                serde_json::from_value(p)
                    .map_err(|e| AcpError::InvalidRequest(format!("invalid params: {}", e)))
            })?;

        if !self.engine.verify_session_ownership(
            &raw_req.session_id,
            token.map(|t| t.token_id.as_str()),
        ) {
            return Err(AcpError::CapabilityNotGranted(format!(
                "session '{}' not owned by this token",
                raw_req.session_id
            )));
        }

        let consent_req = russell_session::ConsentRequest {
            session_id: raw_req.session_id,
            action_id: raw_req.action_id,
            decision: raw_req.decision,
            reason: raw_req.reason,
        };

        let resp = self.engine.respond_consent(consent_req).map_err(|e| match e {
            russell_session::SessionError::SessionNotFound(id) => AcpError::SessionNotFound(id),
            russell_session::SessionError::NotWaitingForConsent(id, state) => {
                AcpError::InvalidRequest(format!(
                    "session '{}' is not waiting for consent (state: {:?})",
                    id, state
                ))
            }
            russell_session::SessionError::NoPendingAction => {
                AcpError::InvalidRequest("no pending action in session".to_string())
            }
            russell_session::SessionError::ActionIdMismatch(got, expected) => {
                AcpError::InvalidRequest(format!(
                    "action_id '{}' does not match pending action '{}'",
                    got, expected
                ))
            }
            _ => AcpError::InvalidRequest(e.to_string()),
        })?;

        if let Some(ref cns) = self.cns {
            let decision_text = match resp.decision {
                ConsentDecision::Approve => "approved",
                ConsentDecision::Deny => "denied",
            };
            cns.emit_consent_decision(&resp.action_id, decision_text);
        }

        Ok(json!(resp))
    }

    async fn get_capabilities(
        &self,
        _params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        let skills = self.dispatch.load_public_skills();
        let probes = self.dispatch.list_probes();
        Ok(json!(CapabilitiesResponse { skills, probes }))
    }

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

        if let Some(ref cns) = self.cns {
            cns.emit_skill_dispatched(&skill_id, "run");
        }

        Ok(json!({"result": result}))
    }

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

    async fn list_notifications(
        &self,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        let hours = params
            .as_ref()
            .and_then(|p| p.get("hours"))
            .and_then(|v| v.as_i64())
            .unwrap_or(24)
            .min(168);

        let reader = self
            .journal_reader
            .as_ref()
            .ok_or_else(|| AcpError::Internal("journal reader not configured".into()))?;

        let since_unix = russell_core::time::now_unix() - (hours * 3600);

        let events = reader
            .list_events_by_action("self_vital_breach", since_unix, i64::MAX)
            .map_err(|e| AcpError::Internal(format!("journal query failed: {e}")))?;

        let notifications: Vec<ProprioNotification> = events
            .iter()
            .filter_map(|row| {
                let summary = row.summary.as_deref()?;
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

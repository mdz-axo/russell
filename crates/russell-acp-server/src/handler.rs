// SPDX-License-Identifier: MIT OR Apache-2.0
//! ACP handler — processes JSON-RPC requests.

use chrono::Utc;
use serde_json::json;
use tracing::{debug, info, warn};

use crate::CapabilityToken;
use crate::auth::MacaroonAuth;
use crate::dispatch::AcpDispatch;
use crate::error::{AcpError, Result};
use crate::persona::JackPersonaProjection;
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
    /// Skill dispatch.
    dispatch: AcpDispatch,
    /// Macaroon auth.
    auth: MacaroonAuth,
    /// Rate limiter.
    rate_limiter: RateLimiter,
    /// Whether authentication is required for all requests.
    require_auth: bool,
    /// Inference backend for LLM responses (T6).
    inference: Option<std::sync::Arc<dyn russell_core::inference::InferencePort>>,
}

impl AcpHandler {
    /// Create a new ACP handler.
    pub fn new(
        persona: JackPersonaProjection,
        dispatch: AcpDispatch,
        auth: MacaroonAuth,
        rate_limiter: RateLimiter,
    ) -> Self {
        Self {
            sessions: SessionManager::new(),
            persona,
            dispatch,
            auth,
            rate_limiter,
            require_auth: false,
            inference: None,
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
            "acp/capabilities" => self.get_capabilities(request.params).await,
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
                Ok(resp) => resp.text,
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

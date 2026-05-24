// SPDX-License-Identifier: MIT OR Apache-2.0
//! HTTP REST routes for the Russell API server.
//!
//! All endpoints delegate to `SessionEngine` from `russell-session`,
//! ensuring functional equivalence with CLI and ACP surfaces.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use tracing::warn;

use russell_session::{ConsentDecision, ConsentRequest, SessionEngine};

use crate::AppState;

/// Build the API router.
pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/sessions", post(create_session))
        .route("/sessions/{id}", get(get_session).delete(close_session))
        .route("/sessions/{id}/messages", post(send_message))
        .route("/sessions/{id}/consent", post(respond_consent))
        .route("/health", get(health_check))
        .with_state(state)
}

/// Create session request body.
#[derive(Debug, Deserialize)]
pub struct CreateSessionBody {
    #[serde(default = "default_persona")]
    pub persona: String,
}

fn default_persona() -> String {
    "jack".to_string()
}

async fn create_session(
    State(state): State<AppState>,
    Json(body): Json<CreateSessionBody>,
) -> impl IntoResponse {
    let mut engine = state.engine.lock().unwrap();
    match engine.create_session(&body.persona) {
        Ok(resp) => (StatusCode::CREATED, Json(resp)).into_response(),
        Err(e) => {
            warn!(error = %e, "Failed to create session");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct SendMessageBody {
    pub message: String,
}

async fn send_message(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Json(body): Json<SendMessageBody>,
) -> impl IntoResponse {
    let mut engine = state.engine.lock().unwrap();
    match engine.send_message(&session_id, &body.message) {
        Ok(resp) => (StatusCode::OK, Json(resp)).into_response(),
        Err(e) => {
            let status = match &e {
                russell_session::SessionError::SessionNotFound(_) => StatusCode::NOT_FOUND,
                russell_session::SessionError::SessionClosed(_) => StatusCode::GONE,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            (status, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ConsentBody {
    pub action_id: String,
    pub decision: ConsentDecision,
    #[serde(default)]
    pub reason: Option<String>,
}

async fn respond_consent(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Json(body): Json<ConsentBody>,
) -> impl IntoResponse {
    let request = ConsentRequest {
        session_id: session_id.clone(),
        action_id: body.action_id,
        decision: body.decision,
        reason: body.reason,
    };

    let mut engine = state.engine.lock().unwrap();
    match engine.respond_consent(request) {
        Ok(resp) => (StatusCode::OK, Json(resp)).into_response(),
        Err(e) => {
            let status = match &e {
                russell_session::SessionError::SessionNotFound(_) => StatusCode::NOT_FOUND,
                russell_session::SessionError::NotWaitingForConsent(_, _) => StatusCode::CONFLICT,
                russell_session::SessionError::NoPendingAction => StatusCode::CONFLICT,
                russell_session::SessionError::ActionIdMismatch(_, _) => StatusCode::BAD_REQUEST,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            (status, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

async fn get_session(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    let engine = state.engine.lock().unwrap();
    match engine.get_session(&session_id) {
        Ok(session) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "session_id": session.id,
                "turn_count": session.turns.len(),
                "last_activity": session.last_activity.to_rfc3339(),
                "persona": session.persona,
                "state": format!("{:?}", session.state),
                "pending_action": session.pending_action,
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn close_session(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    let mut engine = state.engine.lock().unwrap();
    match engine.close_session(&session_id) {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({"session_id": session_id, "closed": true})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({"status": "ok", "service": "russell-api-server"}))
}

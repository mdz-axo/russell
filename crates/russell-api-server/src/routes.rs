// SPDX-License-Identifier: MIT OR Apache-2.0
//! HTTP REST routes for the Russell API server.

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};

use crate::AppState;
use crate::{ApiSession, ApiSessionState, ApiTurn};

/// Build the axum router with all API routes.
/// Build the axum router with all API routes.
pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/sessions", post(create_session))
        .route("/sessions/{id}", get(get_session).delete(close_session))
        .route("/sessions/{id}/messages", post(send_message))
        .route("/sessions/{id}/consent", post(respond_consent))
        .route("/health", get(health_check))
        .with_state(state)
}

#[derive(Debug, Deserialize)]
struct CreateSessionBody {
    #[serde(default = "default_persona")]
    persona: String,
}

fn default_persona() -> String {
    "jack".to_string()
}

#[derive(Debug, Serialize)]
struct SessionResponse {
    session_id: String,
    created_at: String,
    persona: String,
}

async fn create_session(
    State(state): State<AppState>,
    Json(body): Json<CreateSessionBody>,
) -> impl IntoResponse {
    let session = ApiSession {
        id: uuid::Uuid::new_v4().to_string(),
        persona: body.persona,
        turns: Vec::new(),
        created: chrono::Utc::now(),
        last_activity: chrono::Utc::now(),
        state: ApiSessionState::Active,
        pending_action: None,
    };

    let resp = SessionResponse {
        session_id: session.id.clone(),
        created_at: session.created.to_rfc3339(),
        persona: session.persona.clone(),
    };

    state
        .sessions()
        .lock()
        .await
        .insert(session.id.clone(), session);

    (StatusCode::CREATED, Json(resp)).into_response()
}

#[derive(Debug, Deserialize)]
struct SendMessageBody {
    message: String,
}

async fn send_message(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Json(body): Json<SendMessageBody>,
) -> impl IntoResponse {
    let mut sessions = state.sessions().lock().await;

    let session = match sessions.get_mut(&session_id) {
        Some(s) if s.state != ApiSessionState::Closed => s,
        Some(_) => {
            return (
                StatusCode::GONE,
                Json(serde_json::json!({"error": "session is closed"})),
            )
                .into_response();
        }
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "session not found"})),
            )
                .into_response();
        }
    };

    let user_turn = ApiTurn {
        id: uuid::Uuid::new_v4().to_string(),
        role: "user".to_string(),
        content: body.message.clone(),
        timestamp: chrono::Utc::now(),
    };
    session.turns.push(user_turn);

    let response_text =
        "[No inference backend configured. Use the CLI or ACP for full LLM integration.]"
            .to_string();

    let assistant_turn = ApiTurn {
        id: uuid::Uuid::new_v4().to_string(),
        role: "assistant".to_string(),
        content: response_text.clone(),
        timestamp: chrono::Utc::now(),
    };
    session.turns.push(assistant_turn);
    session.last_activity = chrono::Utc::now();

    let turns_summary: Vec<serde_json::Value> = session
        .turns
        .iter()
        .map(|t| {
            serde_json::json!({
                "id": t.id,
                "role": t.role,
                "content": t.content,
                "timestamp": t.timestamp.to_rfc3339(),
            })
        })
        .collect();

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "session_id": session.id,
            "response": response_text,
            "turns": turns_summary,
            "state": format!("{:?}", session.state),
            "pending_action": session.pending_action,
        })),
    )
        .into_response()
}

#[derive(Debug, Deserialize)]
struct ConsentBody {
    action_id: String,
    decision: String,
    #[serde(default)]
    reason: Option<String>,
}

async fn respond_consent(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Json(body): Json<ConsentBody>,
) -> impl IntoResponse {
    let mut sessions = state.sessions().lock().await;

    let session = match sessions.get_mut(&session_id) {
        Some(s) => s,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "session not found"})),
            )
                .into_response();
        }
    };

    if session.state != ApiSessionState::InputRequired {
        return (
            StatusCode::CONFLICT,
            Json(serde_json::json!({"error": "session is not waiting for consent"})),
        )
            .into_response();
    }

    match session.pending_action.as_ref() {
        Some(pa) if pa.action_id == body.action_id => {}
        Some(pa) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": format!("action_id '{}' does not match pending '{}'", body.action_id, pa.action_id)
                })),
            )
                .into_response()
        }
        None => {
            return (
                StatusCode::CONFLICT,
                Json(serde_json::json!({"error": "no pending action"})),
            )
                .into_response()
        }
    }

    let approved = body.decision == "approve";
    let result = if approved {
        Some("Intervention approved (execution via skill dispatcher)".to_string())
    } else {
        None
    };
    let error = if approved {
        None
    } else {
        Some("Action denied by operator".to_string())
    };

    session.pending_action = None;
    session.state = ApiSessionState::Active;
    session.last_activity = chrono::Utc::now();

    session.turns.push(ApiTurn {
        id: uuid::Uuid::new_v4().to_string(),
        role: "user".to_string(),
        content: format!(
            "Operator {} intervention{}",
            if approved { "approved" } else { "denied" },
            body.reason
                .as_ref()
                .map(|r| format!(" (reason: {})", r))
                .unwrap_or_default()
        ),
        timestamp: chrono::Utc::now(),
    });

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "session_id": session_id,
            "action_id": body.action_id,
            "decision": body.decision,
            "result": result,
            "error": error,
        })),
    )
        .into_response()
}

async fn get_session(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    let sessions = state.sessions().lock().await;
    match sessions.get(&session_id) {
        Some(session) => (
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
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "session not found"})),
        )
            .into_response(),
    }
}

async fn close_session(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    let mut sessions = state.sessions().lock().await;
    match sessions.get_mut(&session_id) {
        Some(session) => {
            session.state = ApiSessionState::Closed;
            session.last_activity = chrono::Utc::now();
            (
                StatusCode::OK,
                Json(serde_json::json!({"session_id": session_id, "closed": true})),
            )
                .into_response()
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "session not found"})),
        )
            .into_response(),
    }
}

async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({"status": "ok", "service": "russell-api-server"}))
}

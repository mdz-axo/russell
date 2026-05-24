// SPDX-License-Identifier: MIT OR Apache-2.0
//! Session error types.

use thiserror::Error;

/// Errors from the session engine.
#[derive(Debug, Error)]
pub enum SessionError {
    /// Session not found.
    #[error("session not found: {0}")]
    SessionNotFound(String),
    /// Session is closed.
    #[error("session is closed: {0}")]
    SessionClosed(String),
    /// Session is not waiting for consent.
    #[error("session '{0}' is not waiting for consent (state: {1:?})")]
    NotWaitingForConsent(String, crate::session::SessionState),
    /// No pending action in session.
    #[error("no pending action in session")]
    NoPendingAction,
    /// Action ID mismatch.
    #[error("action_id '{0}' does not match pending action '{1}'")]
    ActionIdMismatch(String, String),
    /// Session ownership verification failed.
    #[error("session '{0}' not owned by this token")]
    OwnershipFailed(String),
    /// Inference backend unavailable.
    #[error("inference unavailable: {0}")]
    InferenceUnavailable(String),
    /// Invalid request.
    #[error("invalid request: {0}")]
    InvalidRequest(String),
}

/// Result type for session operations.
pub type SessionResult<T> = Result<T, SessionError>;

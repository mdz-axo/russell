// SPDX-License-Identifier: MIT OR Apache-2.0
//! ACP server error taxonomy.

use thiserror::Error;

/// ACP server error types.
#[derive(Debug, Error)]
pub enum AcpError {
    /// Session not found.
    #[error("session '{0}' not found")]
    SessionNotFound(String),

    /// Session already closed.
    #[error("session '{0}' is closed")]
    SessionClosed(String),

    /// Skill is private (not exposed via ACP).
    #[error("skill '{0}' is private and not exposed via ACP")]
    SkillNotExposed(String),

    /// Skill not found in registry.
    #[error("skill '{0}' not found in registry")]
    SkillNotFound(String),

    /// Probe not found.
    #[error("probe '{0}' not found")]
    ProbeNotFound(String),

    /// Macaroon authentication failed.
    #[error("macaroon authentication failed: {0}")]
    AuthFailed(String),

    /// Capability token expired.
    #[error("capability token expired at {0}")]
    TokenExpired(String),

    /// Capability not granted (skill not in token's attenuation list).
    #[error("capability '{0}' not granted by token")]
    CapabilityNotGranted(String),

    /// Rate limit exceeded.
    #[error("rate limit exceeded: {0} calls/minute")]
    RateLimitExceeded(u32),

    /// Invalid JSON-RPC request.
    #[error("invalid JSON-RPC request: {0}")]
    InvalidRequest(String),

    /// Internal dispatch error (IDRS failure, probe timeout, etc.).
    #[error("dispatch error: {0}")]
    DispatchError(String),

    /// Configuration error.
    #[error("configuration error: {0}")]
    Config(String),

    /// Transport error (stdio, TCP).
    #[error("transport error: {0}")]
    Transport(String),

    /// Serialization error.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Internal error (bug, invariant violation).
    #[error("internal error: {0}")]
    Internal(String),

    /// Invalid capability token (malformed or tampered).
    #[error("invalid token: {0}")]
    InvalidToken(String),
}

/// Result type alias.
pub type Result<T> = std::result::Result<T, AcpError>;

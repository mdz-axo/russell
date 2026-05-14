// SPDX-License-Identifier: MIT OR Apache-2.0
//! Error types for the Russell MCP client.

use thiserror::Error;

/// Errors that can occur during MCP client operations.
#[derive(Debug, Error)]
pub enum McpError {
    /// The configured endpoint is not a loopback address.
    /// Russell refuses to connect to non-localhost MCP servers
    /// (ADR-0025 §4).
    #[error("refused: endpoint is not loopback ({url})")]
    NonLoopbackRefused {
        /// The rejected URL.
        url: String,
    },

    /// The endpoint URL could not be parsed.
    #[error("invalid endpoint URL: {0}")]
    InvalidUrl(String),

    /// HTTP transport error (connection refused, timeout, etc.).
    #[error("transport error: {message}")]
    Transport {
        /// Human-readable error description.
        message: String,
        /// Whether this was a connection error (endpoint unreachable).
        is_connect: bool,
        /// Whether this was a timeout.
        is_timeout: bool,
    },

    /// MCP protocol error — server returned a JSON-RPC error.
    #[error("mcp protocol error {code}: {message}")]
    Protocol {
        /// JSON-RPC error code.
        code: i64,
        /// Error message from server.
        message: String,
    },

    /// Response could not be deserialized.
    #[error("invalid response: {0}")]
    InvalidResponse(String),

    /// Authentication failed (401 from server).
    #[error("authentication failed — check KASK_MCP_TOKEN")]
    Unauthenticated,

    /// Authorization denied (403 from server).
    #[error("authorization denied for this operation")]
    Forbidden,

    /// Server returned an unexpected HTTP status.
    #[error("unexpected HTTP status {status}: {body}")]
    HttpStatus {
        /// HTTP status code.
        status: u16,
        /// Response body (truncated).
        body: String,
    },

    /// Configuration error (missing or invalid env vars).
    #[error("configuration error: {0}")]
    Config(String),

    /// The tool registry is unavailable (Kask unreachable, cache empty).
    #[error("tool registry unavailable: {0}")]
    RegistryUnavailable(String),
}

/// Result alias for MCP operations.
pub type Result<T> = std::result::Result<T, McpError>;

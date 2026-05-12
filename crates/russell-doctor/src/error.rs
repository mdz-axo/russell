// SPDX-License-Identifier: MIT OR Apache-2.0
//! Nurse error type.

use std::path::PathBuf;

/// Nurse result alias.
pub type Result<T> = std::result::Result<T, DoctorError>;

/// Errors produced by the Nurse.
#[derive(Debug, thiserror::Error)]
pub enum DoctorError {
    /// An I/O failure on a known path.
    #[error("io error on {path}: {source}")]
    Io {
        /// The path being operated on.
        path: PathBuf,
        /// The underlying error.
        #[source]
        source: std::io::Error,
    },

    /// A JSON serialisation / deserialisation failure.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    /// Error from `russell-core`.
    #[error("core error: {0}")]
    Core(#[from] russell_core::CoreError),

    /// HTTP transport error.
    #[error("http {status:?}: {message}")]
    Http {
        /// HTTP status, if any.
        status: Option<u16>,
        /// Human-readable message.
        message: String,
        /// Whether the underlying error was a connection error.
        is_connect: bool,
        /// Whether the underlying error was a timeout.
        is_timeout: bool,
    },

    /// Authentication with the upstream provider failed.
    #[error("authentication failed: {0}")]
    Authentication(String),

    /// The model requested is not available on the configured provider.
    #[error("model not found: {0}")]
    ModelNotFound(String),

    /// The provider is rate-limiting us.
    #[error("rate limited (retry after: {retry_after_seconds:?}s)")]
    RateLimited {
        /// Suggested seconds to wait, if provided by the server.
        retry_after_seconds: Option<u64>,
    },

    /// ZDR requirement could not be satisfied by any available provider.
    #[error("ZDR routing failed: {0}")]
    ZdrRoutingFailed(String),

    /// The operator's env file is missing required keys for the
    /// configured backend, and the fallback path refused.
    #[error("configuration error: {0}")]
    Config(String),

    /// A response body could not be parsed.
    #[error("bad response: {0}")]
    BadResponse(String),

    /// String-formatting failed — should never happen in practice.
    #[error("fmt error: {0}")]
    Fmt(#[from] std::fmt::Error),

    /// Catch-all for rare conditions.
    #[error("{0}")]
    Other(String),
}

impl DoctorError {
    /// Construct an I/O error with path context.
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}

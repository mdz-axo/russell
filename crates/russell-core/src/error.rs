// SPDX-License-Identifier: MIT OR Apache-2.0
//! Error type for `russell-core`.
//!
//! Per [`docs/standards/coding-rust.md` §3](../../../docs/standards/coding-rust.md),
//! library crates return a typed `Result`; we use `thiserror`.

use std::path::PathBuf;

/// Result alias used throughout `russell-core`.
pub type Result<T> = std::result::Result<T, CoreError>;

/// Errors produced by core paths, event, profile, and journal subsystems.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    /// The environment did not expose a usable HOME / XDG variable.
    #[error("could not resolve base path: {0}")]
    BasePath(String),

    /// Filesystem I/O failed.
    #[error("io error on {path}: {source}")]
    Io {
        /// Path being operated on.
        path: PathBuf,
        /// Underlying error.
        #[source]
        source: std::io::Error,
    },

    /// JSON serialisation / deserialisation failed.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    /// SQLite returned an error.
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    /// A schema version in a persisted artefact is not one this build
    /// recognises. See ADR-0006 — Russell does not silently downgrade.
    #[error("unknown schema version: expected {expected}, found {found}")]
    UnknownSchema {
        /// The version the current build understands.
        expected: &'static str,
        /// The version found in the artefact.
        found: String,
    },

    /// A migration step could not run. The journal is in an
    /// undefined state and must not be written to by this process.
    #[error("journal migration {version} failed: {reason}")]
    Migration {
        /// Numeric migration version.
        version: u32,
        /// Human-readable reason.
        reason: String,
    },

    /// An invariant that the type system could not express was violated.
    /// These are bugs, not environmental failures.
    #[error("invariant violation: {0}")]
    Invariant(String),
}

impl CoreError {
    /// Wrap an [`std::io::Error`] with the path it was operating on.
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io { path: path.into(), source }
    }
}

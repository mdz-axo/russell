// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell-api-server` — HTTP REST API for Russell.
//!
//! Provides functionally equivalent access to the same `SessionEngine`
//! used by the CLI and ACP surfaces. All three surfaces share the same
//! session, consent, and inference machinery.
//!
//! ## Endpoints
//!
//! - `POST /sessions` — Create a new session
//! - `POST /sessions/:id/messages` — Send a message
//! - `POST /sessions/:id/consent` — Respond to a consent request
//! - `GET /sessions/:id` — Get session status
//! - `DELETE /sessions/:id` — Close a session

#![deny(unsafe_code)]
#![deny(rust_2018_idioms)]
#![warn(missing_docs)]

pub mod routes;

use russell_session::SessionEngine;

/// Shared application state — holds the session engine.
pub struct AppState {
    /// The session engine (shared across all surfaces).
    pub engine: tokio::sync::Mutex<SessionEngine>,
}

impl AppState {
    /// Create new app state with the given session engine.
    pub fn new(engine: SessionEngine) -> Self {
        Self {
            engine: tokio::sync::Mutex::new(engine),
        }
    }
}

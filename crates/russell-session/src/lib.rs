// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell-session` — shared session engine for Russell.
//!
//! Provides the multi-turn interactive session with Jack persona, including
//! consent flow for interventions. Used by all three interaction surfaces:
//!
//! - **CLI** — `russell jack` interactive REPL
//! - **API** — HTTP REST endpoints
//! - **ACP** — JSON-RPC over stdio (hKask integration)
//!
//! All three surfaces are functionally equivalent. They exercise the same
//! `SessionEngine` and share the same `Session`, `SessionManager`, and
//! consent machinery.

#![deny(unsafe_code)]
#![deny(rust_2018_idioms)]
#![warn(missing_docs)]

pub mod engine;
pub mod error;
pub mod port;
pub mod session;
pub mod types;

pub use engine::SessionEngine;
pub use error::{SessionError, SessionResult};
pub use port::InterventionPort;
pub use session::{
    ConsentDecision, PendingAction, Session, SessionManager, SessionState, ToolCallRecord, Turn,
    TurnRole,
};
pub use types::{
    ConsentRequest, ConsentResponse, CreateSessionRequest, CreateSessionResponse,
    SessionMessageRequest, SessionMessageResponse, ToolCallSummary, TurnInfo,
};

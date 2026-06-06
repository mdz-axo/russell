// SPDX-License-Identifier: MIT OR Apache-2.0
//! Russell ACP Server — session-oriented interface for hKask integration.
//!
//! This crate implements the Agent Client Protocol (ACP) server for Russell,
//! exposing public skills and host probes to hKask agents while maintaining
//! a security boundary around private skills and proprioception data.
//!
//! ## Architecture
//!
//! ```text
//! hKask Agent
//!   │  (JSON-RPC over stdio)
//!   ▼
//! AcpServer
//!   ├── SessionEngine (from russell-session, shared with CLI and API)
//!   ├── JackPersonaProjection (stub)
//!   ├── AcpDispatch (public skill filtering)
//!   ├── MacaroonAuth (OCAP validation)
//!   └── RateLimiter (100 calls/min per token)
//! ```
//!
//! ## Three Surfaces
//!
//! Russell's interactive Jack session is available on three functionally
//! equivalent surfaces:
//!
//! - **CLI** — `russell chat` interactive REPL
//! - **API** — HTTP REST endpoints (via `russell-api-server`)
//! - **ACP** — JSON-RPC over stdio (this crate, via `russell-session`)
//!
//! All three use the same `SessionEngine` from `russell-session`.

#![deny(unsafe_code)]
#![warn(missing_docs)]
#![warn(rustdoc::missing_crate_level_docs)]

pub mod auth;
pub mod cns;
pub mod dispatch;
pub mod encryption;
pub mod error;
pub mod handler;
pub mod persona;
pub mod port;
pub mod rate_limit;
pub mod transport;
pub mod types;

// Re-export session types from russell-session for backward compatibility.
pub use russell_session::{
    ConsentDecision, PendingAction, Session, SessionManager, SessionState, ToolCallRecord, Turn,
    TurnRole,
};

// Re-export main types for convenience.
pub use auth::{CapabilityToken, MacaroonAuth, token_errors};
pub use cns::{AcpCnsEmitter, CnsPort, LoggingCnsAdapter, NoopCnsAdapter};
pub use dispatch::AcpDispatch;
pub use encryption::{EncryptionKey, decrypt_token, encrypt_token};
pub use error::{AcpError, Result};
pub use handler::AcpHandler;
pub use persona::JackPersonaProjection;
pub use port::SkillDispatchPort;
pub use rate_limit::RateLimiter;
pub use transport::AcpServer;
pub use types::{
    InterventionInfo, LexiconCategorization, LexiconDomain, ProbeInfo, SafetyInfo, SkillInfo,
    Visibility,
};

// Re-export protocol types from russell-protocol for cross-project alignment.
pub use russell_protocol::{
    ACP_VERSION,
    auth::CapabilityToken as ProtocolCapabilityToken,
    jsonrpc::{JsonRpcError, JsonRpcRequest, JsonRpcResponse},
    notification::{NotificationsResponse, ProprioNotification},
    skill::CapabilitiesResponse as ProtocolCapabilitiesResponse,
};

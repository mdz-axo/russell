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
//!   ├── SessionManager (multi-turn state)
//!   ├── JackPersonaProjection (stub)
//!   ├── AcpDispatch (public skill filtering)
//!   ├── MacaroonAuth (OCAP validation)
//!   └── RateLimiter (100 calls/min per token)
//! ```
//!
//! ## Security Boundaries
//!
//! - **Public skills** (8): Exposed via ACP — read-only or informational
//! - **Private skills** (6): Russell-only — host mutations, sudo operations
//! - **Proprioception**: Never exposed — Russell self-vitals are security-sensitive
//!
//! ## Usage
//!
//! ```rust,no_run
//! use russell_acp_server::{AcpServer, AcpDispatch, AcpHandler, JackPersonaProjection, MacaroonAuth, RateLimiter};
//! use russell_skills;
//! use std::path::PathBuf;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Initialize Jack persona.
//!     let persona = JackPersonaProjection::new()?;
//!
//!     // Load skills.
//!     let skills_dir = PathBuf::from(std::env::var("HOME")?)
//!         .join(".local/share/harness/skills");
//!     let skills = russell_skills::load_all(&skills_dir)?;
//!
//!     // Initialize components.
//!     let dispatch = AcpDispatch::new(skills, skills_dir);
//!     let auth = MacaroonAuth::new(None, true);
//!     let rate_limiter = RateLimiter::default();
//!     let handler = AcpHandler::new(persona, dispatch, auth, rate_limiter);
//!     let server = AcpServer::new(handler);
//!
//!     // Serve over stdio.
//!     server.serve_stdio().await?;
//!     Ok(())
//! }
//! ```
//!
//! ## ADR-0026 Compliance
//!
//! This crate implements the decisions in
//! [ADR-0026](../../docs/adr/0026-acp-integration.md):
//! - Hybrid deployment (ACP server + sentinel timer)
//! - Visibility boundary (public/private skills)
//! - Macaroon-based OCAP authentication
//! - Persistence independence (SQLite journal remains local)

#![deny(unsafe_code)]
#![warn(missing_docs)]
#![warn(rustdoc::missing_crate_level_docs)]

pub mod auth;
pub mod dispatch;
pub mod encryption;
pub mod error;
pub mod handler;
pub mod persona;
pub mod rate_limit;
pub mod session;
pub mod transport;
pub mod types;

// Re-export main types for convenience.
pub use auth::{CapabilityToken, MacaroonAuth};
pub use dispatch::AcpDispatch;
pub use encryption::{EncryptionKey, decrypt_token, encrypt_token};
pub use error::{AcpError, Result};
pub use handler::AcpHandler;
pub use persona::JackPersonaProjection;
pub use rate_limit::RateLimiter;
pub use session::{Session, SessionManager, SessionState, ToolCallRecord, Turn, TurnRole};
pub use transport::AcpServer;
pub use types::{
    InterventionInfo, LexiconCategorization, LexiconDomain, ProbeInfo, SafetyInfo, SkillInfo,
    Visibility,
};

// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell-mcp` — MCP client for hKask tool access.
//!
//! **Status:** Tertiary interface — ACP is primary for hKask integration.
//!
//! **TOGAF Phase:** Phase C (Application Architecture)
//!
//! Per [ADR-0025](../../docs/adr/0025-hkask-mcp-client-trusted-relationship.md),
//! Russell gains an MCP client that connects **exclusively** to the local
//! hKask installation's MCP endpoint. No general remote MCP servers.
//!
//! This crate provides MCP **client** functionality only. The MCP server
//! feature was removed in v0.20.0; use `russell-acp-server` for hKask
//! agent integration (ADR-0027).
//!
//! # Architecture
//!
//! - [`auth::TokenProvider`] — Token authentication with automatic refresh.
//! - [`client::HKaskMcpClient`] — the REST API client.
//! - [`config::HKaskMcpConfig`] — env-driven configuration.
//! - [`registry::ToolRegistry`] — cached `tools/list` with TTL refresh.
//! - [`error::McpError`] — error taxonomy.
//!
//! # Feature flags
//!
//! - `client` (default) — MCP client for hKask tool access.
//!
//! # Safety constraints
//!
//! The client **refuses** to connect to any non-loopback address. This
//! is enforced at the transport layer in [`client::validate_endpoint`].

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms)]
#![warn(missing_docs)]

#[cfg(feature = "client")]
pub mod auth;
#[cfg(feature = "client")]
pub mod client;
#[cfg(feature = "client")]
pub mod config;
#[cfg(feature = "client")]
pub mod error;
#[cfg(feature = "client")]
pub mod health;
#[cfg(feature = "client")]
pub mod port;
#[cfg(feature = "client")]
pub mod registry;
#[cfg(feature = "client")]
pub mod types;

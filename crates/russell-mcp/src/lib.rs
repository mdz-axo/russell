// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell-mcp` — MCP client for hKask and MCP server for IDE frontends.
//!
//! **TOGAF Phase:** Phase C (Application Architecture) — exposes Russell's
//! telemetry as MCP tools (`russell_jack`, `russell_sentinel`, `russell_proprio`)
//! for agentic consumption via hKask's MCP infrastructure.
//!
//! Per [ADR-0025](../../docs/adr/0025-hkask-mcp-client-trusted-relationship.md),
//! Russell gains an MCP client that connects **exclusively** to the local
//! hKask installation's MCP endpoint. No general remote MCP servers. No
//! remote skill registries. hKask is the sole trust boundary.
//!
//! This crate also provides an MCP server for IDE frontends (Zed, Claude Desktop,
//! Cline/Roo) via stdio transport (ADR-0003).
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
//! - `client` (default) — Include MCP client for hKask integration.
//! - `server` — Include MCP server for IDE frontends.
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
pub mod registry;
#[cfg(feature = "client")]
pub mod types;

#[cfg(feature = "server")]
pub mod server;

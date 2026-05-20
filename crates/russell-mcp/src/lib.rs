// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell-mcp` — MCP client for Kask and MCP server for IDE frontends.
//!
//! **TOGAF Phase:** Phase C (Application Architecture) — exposes Russell's
//! telemetry as MCP tools (`russell_jack`, `russell_sentinel`, `russell_proprio`)
//! for agentic consumption via Kask's MCP infrastructure.
//!
//! Per [ADR-0025](../../docs/adr/0025-kask-mcp-client-trusted-relationship.md),
//! Russell gains an MCP client that connects **exclusively** to the local
//! Kask installation's MCP endpoint. No general remote MCP servers. No
//! remote skill registries. Kask is the sole trust boundary.
//!
//! This crate also provides an MCP server for IDE frontends (Zed, Claude Desktop,
//! Cline/Roo) via stdio transport (ADR-0003).
//!
//! # Architecture
//!
//! - [`auth::TokenProvider`] — Token authentication with automatic refresh.
//! - [`client::KaskMcpClient`] — the REST API client.
//! - [`config::KaskMcpConfig`] — env-driven configuration.
//! - [`registry::ToolRegistry`] — cached `tools/list` with TTL refresh.
//! - [`error::McpError`] — error taxonomy.
//!
//! # Feature flags
//!
//! - `client` (default) — Include MCP client for Kask integration.
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

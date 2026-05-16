// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell-mcp` — MCP client for the trusted Kask relationship.
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
//! # Architecture
//!
//! - [`auth::TokenProvider`] — Token authentication with automatic refresh.
//! - [`config::KaskMcpConfig`] — env-driven configuration.
//! - [`client::KaskMcpClient`] — the REST API client.
//! - [`registry::ToolRegistry`] — cached `tools/list` with TTL refresh.
//! - [`error::McpError`] — error taxonomy.
//!
//! # Safety constraints
//!
//! The client **refuses** to connect to any non-loopback address. This
//! is enforced at the transport layer in [`client::validate_endpoint`].

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms)]
#![warn(missing_docs)]

pub mod auth;
pub mod client;
pub mod config;
pub mod error;
pub mod health;
pub mod registry;
pub mod types;

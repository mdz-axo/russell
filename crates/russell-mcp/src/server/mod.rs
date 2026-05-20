// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell-mcp::server` — native MCP server for Russell.
//!
//! Implements [ADR-0003](../../../docs/adr/deferred/0003-mcp-transport.md):
//! stdio-only JSON-RPC transport, read-only tools exposing Russell's
//! journal, probe history, and health summary.
//!
//! # Usage
//!
//! ```text
//! russell mcp
//! ```
//!
//! Frontends (Zed, Claude Desktop, Cline/Roo) spawn Russell and
//! communicate via stdin/stdout JSON-RPC.

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms)]
#![warn(missing_docs)]

mod tools;

use rmcp::{
    ServerHandler,
    handler::server::router::tool::ToolRouter,
    model::{ServerCapabilities, ServerInfo},
    tool_handler,
};
use russell_core::paths::Paths;

/// The Russell MCP server handler.
///
/// All tools are read-only (risk: none). They query the journal
/// and probe data without mutating host state.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct RussellServer {
    /// Paths to Russell's on-disk state (journal, config, etc.)
    pub(crate) paths: Paths,
    tool_router: ToolRouter<Self>,
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for RussellServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions(
                "Russell is a cybernetic health harness for a Linux AI/ML workstation. \
                 These tools provide read-only access to system telemetry, journal events, \
                 probe history, and health baselines. All tools are risk:none — they observe \
                 but never mutate host state.",
            )
    }
}

/// Start the MCP server over stdio. Blocks until the frontend
/// disconnects (stdin EOF).
pub async fn serve_stdio(paths: Paths) -> anyhow::Result<()> {
    use rmcp::{ServiceExt, transport::io::stdio};

    tracing::info!("russell MCP server starting (stdio transport)");

    let server = RussellServer::new(paths);
    let service = server
        .serve(stdio())
        .await
        .map_err(|e| anyhow::anyhow!("MCP server initialization failed: {e}"))?;

    service.waiting().await?;
    tracing::info!("russell MCP server exiting");
    Ok(())
}

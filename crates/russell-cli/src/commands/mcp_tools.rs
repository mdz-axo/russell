// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell mcp-tools` — list available Kask MCP tools.
//!
//! Connects to the local Kask MCP endpoint, performs the initialize
//! handshake, calls `tools/list`, and prints the available tools.
//! This is the smoke test for the Phase 4A MCP client (ADR-0025).

use anyhow::{Context, Result};

use russell_mcp::client::KaskMcpClient;
use russell_mcp::config::KaskMcpConfig;

/// Run the `mcp-tools` command.
///
/// Connects to Kask's MCP endpoint, authenticates, and lists tools.
pub async fn run() -> Result<()> {
    let config = KaskMcpConfig::from_env();

    // Report configuration.
    println!("Russell MCP Client — Kask tool discovery");
    println!("  endpoint:  {}", config.endpoint);
    println!(
        "  token:     {}",
        if config.has_token() {
            "configured"
        } else {
            "<not set — set KASK_MCP_TOKEN>"
        }
    );
    println!("  tool TTL:  {}s", config.tool_ttl.as_secs());
    println!("  timeout:   {}s", config.timeout.as_secs());
    println!();

    // Validate endpoint is loopback.
    config
        .validate()
        .context("endpoint validation failed — only loopback addresses are permitted")?;

    // Build client.
    let mut client = KaskMcpClient::new(config).context("failed to construct MCP client")?;

    // Perform handshake.
    println!("Connecting...");
    let init_result = client
        .connect()
        .await
        .context("MCP initialize handshake failed — is Kask running?")?;

    println!(
        "  server:    {}",
        client.server_name().unwrap_or("<unknown>")
    );
    println!(
        "  protocol:  {}",
        init_result
            .protocol_version
            .as_deref()
            .unwrap_or("<unknown>")
    );
    println!();

    // List tools.
    let tools = client.list_tools().await.context("tools/list failed")?;

    if tools.is_empty() {
        println!("No tools available (Kask returned empty tools/list).");
        return Ok(());
    }

    println!("Available Kask MCP tools ({} total):", tools.len());
    println!("{:<35} DESCRIPTION", "TOOL");
    println!("{}", "-".repeat(80));

    for tool in &tools {
        let desc = tool
            .description
            .as_deref()
            .unwrap_or("")
            .lines()
            .next()
            .unwrap_or("");
        let desc_truncated = if desc.len() > 44 {
            format!("{}...", &desc[..41])
        } else {
            desc.to_owned()
        };
        println!("{:<35} {}", tool.name, desc_truncated);
    }

    Ok(())
}

/// Run a ping check against the Kask MCP endpoint.
/// Returns Ok if reachable, Err otherwise.
pub async fn ping() -> Result<()> {
    let config = KaskMcpConfig::from_env();
    config.validate().context("endpoint validation")?;

    let mut client = KaskMcpClient::new(config).context("client construction")?;
    client.connect().await.context("handshake")?;
    client.ping().await.context("ping")?;

    println!("Kask MCP endpoint: reachable");
    Ok(())
}

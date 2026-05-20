// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell mcp-tools` — list available hKask MCP tools.
//!
//! Connects to the local hKask MCP endpoint, performs the initialize
//! handshake, calls `tools/list`, and prints the available tools.
//! This is the smoke test for the Phase 4A MCP client (ADR-0025).

use anyhow::{Context, Result};

use russell_mcp::client::HKaskMcpClient;
use russell_mcp::config::HKaskMcpConfig;

/// Run the `mcp-tools` command.
///
/// Connects to hKask's MCP endpoint, authenticates, and lists tools.
pub async fn run() -> Result<()> {
    let config = HKaskMcpConfig::from_env();

    // Report configuration.
    println!("Russell MCP Client — hKask tool discovery");
    println!("  endpoint:  {}", config.endpoint);
    println!(
        "  token:     {}",
        if config.has_token() {
            "configured"
        } else {
            "<not set — set HKASK_MCP_TOKEN>"
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
    let mut client = HKaskMcpClient::new(config).context("failed to construct MCP client")?;

    // Perform handshake.
    println!("Connecting...");
    client
        .connect()
        .await
        .context("MCP connection failed — is Kask running?")?;

    println!(
        "  server:    {}",
        client.server_name().unwrap_or("<unknown>")
    );
    println!();

    // List tools.
    let tools = client.list_tools().await.context("tools/list failed")?;

    if tools.is_empty() {
        println!("No tools available (Kask returned empty tools/list).");
        return Ok(());
    }

    println!("Available hKask MCP tools ({} total):", tools.len());
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

/// Run a ping check against the hKask MCP endpoint.
/// Returns Ok if reachable, Err otherwise.
pub async fn ping() -> Result<()> {
    let config = HKaskMcpConfig::from_env();
    config.validate().context("endpoint validation")?;

    let mut client = HKaskMcpClient::new(config).context("client construction")?;
    client.connect().await.context("handshake")?;
    client.ping().await.context("ping")?;

    println!("hKask MCP endpoint: reachable");
    Ok(())
}

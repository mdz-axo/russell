// SPDX-License-Identifier: MIT OR Apache-2.0
//! Tool registry — cached `tools/list` with TTL refresh.
//!
//! The registry holds the last-known tool set from Kask's MCP
//! endpoint. It provides the poka-yoke validation surface: any
//! tool ID proposed by the LLM or the operator must exist in this
//! registry (for `kask/` prefixed actions) to be dispatched.

use std::time::{Duration, Instant};

use tracing::{debug, info, warn};

use crate::client::KaskMcpClient;
use crate::error::Result;
use crate::types::McpToolDefinition;

/// Cached tool registry backed by a Kask MCP connection.
///
/// Thread-safe via interior mutability is NOT provided here — the
/// caller (CLI, chat loop) owns the registry and refreshes it on
/// their schedule. This keeps the design simple and testable.
pub struct ToolRegistry {
    /// Cached tool definitions.
    tools: Vec<McpToolDefinition>,
    /// When the cache was last populated.
    last_refresh: Option<Instant>,
    /// Cache TTL.
    ttl: Duration,
}

impl ToolRegistry {
    /// Create an empty registry with the given TTL.
    pub fn new(ttl: Duration) -> Self {
        Self {
            tools: Vec::new(),
            last_refresh: None,
            ttl,
        }
    }

    /// Refresh the tool list from the Kask MCP server.
    ///
    /// On success, replaces the cached tools and resets the TTL.
    /// On failure, the previous cache is retained (graceful degradation).
    pub async fn refresh(&mut self, client: &KaskMcpClient) -> Result<()> {
        match client.list_tools().await {
            Ok(tools) => {
                info!(
                    count = tools.len(),
                    "kask tool registry refreshed"
                );
                self.tools = tools;
                self.last_refresh = Some(Instant::now());
                Ok(())
            }
            Err(e) => {
                warn!(
                    error = %e,
                    cached_count = self.tools.len(),
                    "kask tool registry refresh failed; retaining stale cache"
                );
                Err(e)
            }
        }
    }

    /// Refresh if the cache is stale (past TTL). Returns Ok(true) if
    /// a refresh was performed, Ok(false) if cache is still fresh.
    ///
    /// On refresh failure, returns the error but retains stale cache.
    pub async fn refresh_if_stale(&mut self, client: &KaskMcpClient) -> Result<bool> {
        if self.is_stale() {
            self.refresh(client).await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Whether the cache is stale (past TTL or never populated).
    pub fn is_stale(&self) -> bool {
        match self.last_refresh {
            None => true,
            Some(t) => t.elapsed() > self.ttl,
        }
    }

    /// Whether the registry has any tools cached.
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// Number of cached tools.
    pub fn tool_count(&self) -> usize {
        self.tools.len()
    }

    /// Check whether a tool name exists in the registry (poka-yoke).
    pub fn has_tool(&self, name: &str) -> bool {
        self.tools.iter().any(|t| t.name == name)
    }

    /// Look up a tool definition by name.
    pub fn get_tool(&self, name: &str) -> Option<&McpToolDefinition> {
        self.tools.iter().find(|t| t.name == name)
    }

    /// All cached tool definitions (for display / prompt building).
    pub fn tools(&self) -> &[McpToolDefinition] {
        &self.tools
    }

    /// Get the tool names as a sorted list (for display).
    pub fn tool_names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.tools.iter().map(|t| t.name.as_str()).collect();
        names.sort_unstable();
        names
    }

    /// Time since last refresh, or None if never refreshed.
    pub fn age(&self) -> Option<Duration> {
        self.last_refresh.map(|t| t.elapsed())
    }

    /// Extract the risk annotation from a tool's metadata, if present.
    ///
    /// Kask tools may declare risk in `annotations.risk_band`. If absent,
    /// returns `None` (caller should treat as medium per ADR-0025 §6).
    pub fn tool_risk_band(&self, name: &str) -> Option<String> {
        self.get_tool(name)
            .and_then(|t| t.annotations.as_ref())
            .and_then(|a| a.get("risk_band"))
            .and_then(|v| v.as_str())
            .map(String::from)
    }

    /// Populate the registry from a pre-fetched tool list (for testing
    /// or offline initialization from a cached file).
    pub fn populate(&mut self, tools: Vec<McpToolDefinition>) {
        debug!(count = tools.len(), "registry populated directly");
        self.tools = tools;
        self.last_refresh = Some(Instant::now());
    }

    /// Clear the cache (e.g., on disconnect).
    pub fn clear(&mut self) {
        self.tools.clear();
        self.last_refresh = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tool(name: &str, desc: &str) -> McpToolDefinition {
        McpToolDefinition {
            name: name.to_owned(),
            description: Some(desc.to_owned()),
            input_schema: None,
            annotations: None,
        }
    }

    fn make_tool_with_risk(name: &str, risk: &str) -> McpToolDefinition {
        McpToolDefinition {
            name: name.to_owned(),
            description: Some("test tool".into()),
            input_schema: None,
            annotations: Some(serde_json::json!({ "risk_band": risk })),
        }
    }

    #[test]
    fn empty_registry() {
        let reg = ToolRegistry::new(Duration::from_secs(300));
        assert!(reg.is_empty());
        assert!(reg.is_stale());
        assert!(!reg.has_tool("anything"));
        assert_eq!(reg.tool_count(), 0);
    }

    #[test]
    fn populate_and_query() {
        let mut reg = ToolRegistry::new(Duration::from_secs(300));
        reg.populate(vec![
            make_tool("paradigm_shift_query", "Cascade query"),
            make_tool("russell_host_snapshot", "Host health"),
        ]);

        assert!(!reg.is_empty());
        assert!(!reg.is_stale());
        assert_eq!(reg.tool_count(), 2);
        assert!(reg.has_tool("paradigm_shift_query"));
        assert!(reg.has_tool("russell_host_snapshot"));
        assert!(!reg.has_tool("nonexistent"));
    }

    #[test]
    fn tool_names_sorted() {
        let mut reg = ToolRegistry::new(Duration::from_secs(300));
        reg.populate(vec![
            make_tool("zeta", "z"),
            make_tool("alpha", "a"),
            make_tool("middle", "m"),
        ]);
        assert_eq!(reg.tool_names(), vec!["alpha", "middle", "zeta"]);
    }

    #[test]
    fn risk_band_extraction() {
        let mut reg = ToolRegistry::new(Duration::from_secs(300));
        reg.populate(vec![
            make_tool_with_risk("risky_tool", "medium"),
            make_tool("safe_tool", "no risk annotation"),
        ]);
        assert_eq!(reg.tool_risk_band("risky_tool"), Some("medium".into()));
        assert_eq!(reg.tool_risk_band("safe_tool"), None);
        assert_eq!(reg.tool_risk_band("nonexistent"), None);
    }

    #[test]
    fn stale_after_zero_ttl() {
        let mut reg = ToolRegistry::new(Duration::from_secs(0));
        reg.populate(vec![make_tool("tool", "test")]);
        // With TTL=0, cache is immediately stale.
        std::thread::sleep(Duration::from_millis(1));
        assert!(reg.is_stale());
    }

    #[test]
    fn clear_resets_state() {
        let mut reg = ToolRegistry::new(Duration::from_secs(300));
        reg.populate(vec![make_tool("tool", "test")]);
        assert!(!reg.is_empty());
        reg.clear();
        assert!(reg.is_empty());
        assert!(reg.is_stale());
    }
}

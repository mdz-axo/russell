// SPDX-License-Identifier: MIT OR Apache-2.0
//! Tool registry — cached `tools/list` with TTL refresh.
//!
//! The registry holds the last-known tool set from hKask's MCP
//! endpoint. It provides the poka-yoke validation surface: any
//! tool ID proposed by the LLM or the operator must exist in this
//! registry (for `hkask/` prefixed actions) to be dispatched.

use std::path::Path;
use std::time::{Duration, Instant};

use tracing::{debug, info, warn};

use crate::client::HKaskMcpClient;
use crate::error::Result;
use crate::types::McpToolDefinition;

/// Cached tool registry backed by a hKask MCP connection.
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

    /// Refresh the tool list from the hKask MCP server.
    ///
    /// On success, replaces the cached tools and resets the TTL.
    /// On failure, the previous cache is retained (graceful degradation).
    pub async fn refresh(&mut self, client: &HKaskMcpClient) -> Result<()> {
        match client.list_tools().await {
            Ok(tools) => {
                info!(count = tools.len(), "hKask tool registry refreshed");
                self.tools = tools;
                self.last_refresh = Some(Instant::now());
                Ok(())
            }
            Err(e) => {
                warn!(
                    error = %e,
                    cached_count = self.tools.len(),
                    "hKask tool registry refresh failed; retaining stale cache"
                );
                Err(e)
            }
        }
    }

    /// Refresh if the cache is stale (past TTL). Returns Ok(true) if
    /// a refresh was performed, Ok(false) if cache is still fresh.
    ///
    /// On refresh failure, returns the error but retains stale cache.
    pub async fn refresh_if_stale(&mut self, client: &HKaskMcpClient) -> Result<bool> {
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

    /// Invalidate the cache immediately.
    ///
    /// Call this when a `notifications/tools/list_changed` event is received
    /// from the MCP server, or when explicit cache invalidation is needed.
    pub fn invalidate(&mut self) {
        debug!("tool registry cache invalidated");
        self.last_refresh = None;
    }

    /// Remove a specific tool from the cache (fine-grained invalidation).
    ///
    /// Returns `true` if the tool was found and removed, `false` otherwise.
    pub fn remove_tool(&mut self, name: &str) -> bool {
        let initial_len = self.tools.len();
        self.tools.retain(|t| t.name != name);
        let removed = self.tools.len() < initial_len;
        if removed {
            debug!(tool = name, "tool removed from cache");
        }
        removed
    }

    /// Add or update a tool in the cache (for explicit tool registration).
    pub fn upsert_tool(&mut self, tool: McpToolDefinition) {
        if let Some(existing) = self.tools.iter_mut().find(|t| t.name == tool.name) {
            *existing = tool;
            debug!(tool = %existing.name, "tool updated in cache");
        } else {
            let tool_name = tool.name.clone();
            self.tools.push(tool);
            debug!(tool = %tool_name, "tool added to cache");
        }
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

    /// Save the cached tool definitions to a JSON file on disk.
    ///
    /// This allows Russell to show stale-but-useful tool info even on
    /// first boot before hKask is reachable. The file is saved after
    /// every successful refresh from hKask (ADR-0025 §5, graceful degradation).
    ///
    /// # Errors
    ///
    /// Returns an I/O error if the directory cannot be created or the
    /// file cannot be written.
    pub fn save_to_disk(&self, path: &Path) -> Result<()> {
        if self.tools.is_empty() {
            return Ok(());
        }

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                crate::error::McpError::Config(format!(
                    "failed to create cache directory {}: {e}",
                    parent.display()
                ))
            })?;
        }

        let json = serde_json::to_string_pretty(&self.tools).map_err(|e| {
            crate::error::McpError::Config(format!("failed to serialize tool cache: {e}"))
        })?;

        std::fs::write(path, &json).map_err(|e| {
            crate::error::McpError::Config(format!(
                "failed to write tool cache {}: {e}",
                path.display()
            ))
        })?;

        debug!(
            path = %path.display(),
            tool_count = self.tools.len(),
            "hKask tool cache saved to disk"
        );
        Ok(())
    }

    /// Load cached tool definitions from a JSON file on disk.
    ///
    /// Populates the registry with stale-but-useful tool info. The
    /// `last_refresh` timestamp is NOT set — callers should also
    /// attempt a live refresh.
    ///
    /// # Errors
    ///
    /// Returns an error if the file exists but cannot be parsed.
    /// Missing file is not an error (returns `Ok(())` with no change).
    pub fn load_from_disk(&mut self, path: &Path) -> Result<()> {
        match std::fs::read_to_string(path) {
            Ok(json) => {
                let tools: Vec<McpToolDefinition> = serde_json::from_str(&json).map_err(|e| {
                    crate::error::McpError::Config(format!(
                        "failed to parse tool cache {}: {e}",
                        path.display()
                    ))
                })?;
                debug!(
                    path = %path.display(),
                    tool_count = tools.len(),
                    "hKask tool cache loaded from disk"
                );
                self.tools = tools;
                // Don't set last_refresh — this is stale data.
                Ok(())
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(crate::error::McpError::Config(format!(
                "failed to read tool cache {}: {e}",
                path.display()
            ))),
        }
    }
}


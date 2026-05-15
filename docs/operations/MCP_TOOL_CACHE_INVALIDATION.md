---
title: "MCP Tool Cache Invalidation via notifications/tools/list_changed"
audience: [developers, architects]
last_updated: 2026-05-14
togaf_phase: "G"
version: "1.0.0"
status: "Active"
---

<!-- TOGAF_DOMAIN: Governance -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-05-14 -->

# MCP Tool Cache Invalidation via notifications/tools/list_changed

## Overview

This document describes how Kask MCP servers can notify Russell of tool list changes, enabling immediate cache invalidation without waiting for TTL expiry.

## MCP Protocol Support

The Model Context Protocol (MCP) specification defines a `notifications/tools/list_changed` notification that servers can send to clients when the available tool set changes.

```json
{
  "jsonrpc": "2.0",
  "method": "notifications/tools/list_changed",
  "params": {}
}
```

## Current State

### Russell Side (✅ Complete)

Russell's `ToolRegistry` supports explicit cache invalidation:

```rust
// Full cache invalidation
registry.invalidate();
registry.refresh(&client).await?;

// Fine-grained invalidation
registry.remove_tool("deprecated_tool");
registry.upsert_tool(new_tool_definition);
```

### Kask Side (⚠️ To Implement)

Kask MCP servers (`arsenal-mcp-*`, 16 servers, 193 tools) should emit `notifications/tools/list_changed` when:

1. A tool is dynamically registered or unregistered
2. Tool capabilities change (e.g., new endpoints added)
3. Tool metadata is updated (description, schema changes)

## Implementation Guide for Kask MCP Servers

### 1. Track Tool Changes

In your MCP server implementation, track when tools change:

```rust
// Example: arsenal-mcp-russell/src/server.rs
impl RussellServer {
    pub fn register_tool(&mut self, tool: McpToolDefinition) {
        self.tools.push(tool);
        self.notify_tools_changed(); // <-- Emit notification
    }
    
    pub fn unregister_tool(&mut self, name: &str) {
        self.tools.retain(|t| t.name != name);
        self.notify_tools_changed(); // <-- Emit notification
    }
    
    fn notify_tools_changed(&self) {
        // Send notification to connected clients
        self.send_notification("notifications/tools/list_changed", json!({}));
    }
}
```

### 2. Use stack-mcp Notification Helpers

The `stack-mcp` crate provides notification helpers:

```rust
use stack_mcp::server::McpServer;

// In your server implementation
impl McpToolServer for YourServer {
    async fn call_tool(&self, name: &str, args: Value) -> Result<ToolCallResult> {
        // ... tool logic ...
        
        // If tool registration changes:
        self.notify_tools_changed().await;
    }
}
```

### 3. Hot-Reload Scenarios

For servers that support hot-reloading tool definitions (e.g., from config files):

```rust
// Watch config file for changes
tokio::spawn(async move {
    let mut watcher = notify::recommended_watcher(move |event| {
        if event.kind.is_modify() {
            // Reload tools from config
            server.reload_tools_from_config();
            // Notify clients of change
            server.notify_tools_changed();
        }
    });
    watcher.watch(&config_path, RecursiveMode::NonRecursive)?;
});
```

## Russell Client-Side Handling

When Russell receives a `notifications/tools/list_changed` notification:

```rust
// In russell-mcp/src/client.rs or registry.rs
pub async fn handle_notification(&self, method: &str, _params: &Value) {
    match method {
        "notifications/tools/list_changed" => {
            // Invalidate cache immediately
            self.registry.invalidate();
            
            // Optional: Refresh immediately
            if let Err(e) = self.registry.refresh(&self.client).await {
                warn!(error = %e, "failed to refresh tools after list_changed");
            }
        }
        _ => {}
    }
}
```

## Testing

### Server-Side Test

```rust
#[tokio::test]
async fn server_emits_list_changed_on_tool_add() {
    let mut server = TestServer::new();
    let mut client = server.connect().await;
    
    // Subscribe to notifications
    let mut notifications = client.notifications();
    
    // Add a tool
    server.register_tool(test_tool());
    
    // Verify notification was sent
    let notification = notifications.recv().await.unwrap();
    assert_eq!(notification.method, "notifications/tools/list_changed");
}
```

### Client-Side Test

```rust
#[tokio::test]
async fn client_invalidates_cache_on_notification() {
    let (server, client) = setup_test_server().await;
    
    // Initial tool list
    let tools = client.list_tools().await.unwrap();
    assert_eq!(tools.len(), 1);
    
    // Server adds tool and sends notification
    server.register_tool(test_tool());
    server.send_tools_changed_notification().await;
    
    // Client should have invalidated cache
    assert!(client.registry.is_stale());
}
```

## Benefits

| Without notifications | With notifications |
|----------------------|-------------------|
| Cache stale for up to TTL (5 min) | Immediate invalidation |
| Client polls unnecessarily | Server pushes changes |
| Delayed tool availability | Instant tool discovery |
| Wasted bandwidth | Efficient updates |

## Migration Path

For existing Kask MCP servers:

1. **Phase 1**: Add notification emission on tool changes (non-breaking)
2. **Phase 2**: Update Russell client to handle notifications (this PR)
3. **Phase 3**: Enable notifications in production deployments

## Related

- [MCP Specification - notifications/tools/list_changed](https://modelcontextprotocol.io/specification/2024-11-05/server/tools#list-changed)
- Russell ADR-0025: Kask MCP Client — Trusted Local Relationship
- Russell `crates/russell-mcp/src/registry.rs` — Cache invalidation implementation

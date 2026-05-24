// SPDX-License-Identifier: MIT OR Apache-2.0
//! MCP port — hexagonal trait for MCP tool access.
//!
//! Defines the port interface for MCP operations, decoupling consumers
//! from the concrete `HKaskMcpClient` implementation.

use crate::error::Result;
use crate::types::{McpToolDefinition, ToolCallResult};

/// Port for MCP tool access operations.
///
/// Implementations provide tool discovery, invocation, and health checking.
/// The `HKaskMcpClient` is the primary adapter; `MockMcpPort` for testing.
#[async_trait::async_trait]
pub trait McpPort: Send + Sync {
    /// List available MCP tools.
    async fn list_tools(&self) -> Result<Vec<McpToolDefinition>>;

    /// Call an MCP tool by name with optional arguments.
    async fn call_tool(
        &self,
        name: &str,
        arguments: Option<serde_json::Value>,
    ) -> Result<ToolCallResult>;

    /// Health check — verify the MCP endpoint is reachable.
    async fn health_check(&self) -> Result<()>;
}

/// Mock MCP port for testing.
#[cfg(test)]
pub struct MockMcpPort {
    tools: Vec<McpToolDefinition>,
    health: bool,
}

#[cfg(test)]
impl MockMcpPort {
    /// Create a new mock port with given tools.
    pub fn new(tools: Vec<McpToolDefinition>, health: bool) -> Self {
        Self { tools, health }
    }

    /// Create a healthy mock with no tools.
    pub fn healthy() -> Self {
        Self::new(Vec::new(), true)
    }
}

#[cfg(test)]
#[async_trait::async_trait]
impl McpPort for MockMcpPort {
    async fn list_tools(&self) -> Result<Vec<McpToolDefinition>> {
        Ok(self.tools.clone())
    }

    async fn call_tool(
        &self,
        name: &str,
        _arguments: Option<serde_json::Value>,
    ) -> Result<ToolCallResult> {
        Ok(ToolCallResult {
            content: vec![crate::types::ToolContent {
                content_type: "text".to_string(),
                text: Some(format!("mock result for {}", name)),
                extra: serde_json::Value::Null,
            }],
            is_error: false,
        })
    }

    async fn health_check(&self) -> Result<()> {
        if self.health {
            Ok(())
        } else {
            Err(crate::error::McpError::Transport {
                message: "mock health check failed".to_string(),
                is_connect: true,
                is_timeout: false,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_port_satisfies_trait() {
        let port: Box<dyn McpPort> = Box::new(MockMcpPort::healthy());
        assert!(port.list_tools().await.unwrap().is_empty());
        assert!(port.health_check().await.is_ok());
        let result = port.call_tool("test", None).await.unwrap();
        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn mock_port_unhealthy() {
        let port: Box<dyn McpPort> = Box::new(MockMcpPort::new(Vec::new(), false));
        assert!(port.health_check().await.is_err());
    }
}

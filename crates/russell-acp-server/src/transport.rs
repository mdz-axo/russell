// SPDX-License-Identifier: MIT OR Apache-2.0
//! ACP server transport (stdio).

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{debug, error, info};

use crate::error::Result;
use crate::handler::AcpHandler;
use crate::types::{JsonRpcRequest, JsonRpcResponse};

/// ACP server — stdio JSON-RPC transport.
pub struct AcpServer {
    /// Handler.
    handler: AcpHandler,
}

impl AcpServer {
    /// Create a new ACP server.
    pub fn new(handler: AcpHandler) -> Self {
        Self { handler }
    }

    /// Serve over stdio (JSON-RPC).
    pub async fn serve_stdio(mut self) -> Result<()> {
        info!("ACP server starting on stdio");

        let stdin = tokio::io::stdin();
        let mut reader = BufReader::new(stdin);
        let mut stdout = tokio::io::stdout();
        let mut line = String::new();

        loop {
            line.clear();
            let bytes_read = match reader.read_line(&mut line).await {
                Ok(0) => {
                    // EOF — client disconnected.
                    info!("ACP server: client disconnected (EOF)");
                    break;
                }
                Ok(n) => n,
                Err(e) => {
                    error!(error = %e, "ACP server: read error");
                    continue;
                }
            };

            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            debug!(bytes = %bytes_read, "Read ACP request");

            // Parse JSON-RPC request.
            let request: JsonRpcRequest = match serde_json::from_str(line) {
                Ok(req) => req,
                Err(e) => {
                    let response = JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: serde_json::Value::Null,
                        result: None,
                        error: Some(crate::types::JsonRpcError::new(
                            -32700,
                            format!("parse error: {}", e),
                        )),
                    };
                    send_response(&mut stdout, &response).await?;
                    continue;
                }
            };

            // Handle request.
            let response = self.handler.handle(request).await;
            send_response(&mut stdout, &response).await?;
        }

        info!("ACP server stopped");
        Ok(())
    }
}

/// Send a JSON-RPC response.
async fn send_response(
    writer: &mut (impl AsyncWriteExt + Unpin),
    response: &JsonRpcResponse,
) -> Result<()> {
    let json = serde_json::to_string(response).map_err(crate::error::AcpError::Serialization)?;

    writer
        .write_all(json.as_bytes())
        .await
        .map_err(|e| crate::error::AcpError::Transport(e.to_string()))?;
    writer
        .write_all(b"\n")
        .await
        .map_err(|e| crate::error::AcpError::Transport(e.to_string()))?;
    writer
        .flush()
        .await
        .map_err(|e| crate::error::AcpError::Transport(e.to_string()))?;

    Ok(())
}

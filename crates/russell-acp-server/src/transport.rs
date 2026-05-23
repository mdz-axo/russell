// SPDX-License-Identifier: MIT OR Apache-2.0
//! ACP server transport (stdio).

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{debug, error, info};

use crate::error::Result;
use crate::handler::AcpHandler;
use crate::types::{JsonRpcRequest, JsonRpcResponse};

const GC_INTERVAL_REQUESTS: u64 = 100;

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
        let mut request_count: u64 = 0;

        let shutdown = tokio::signal::ctrl_c();
        tokio::pin!(shutdown);

        loop {
            line.clear();

            let read_result = tokio::select! {
                result = reader.read_line(&mut line) => result,
                _ = &mut shutdown => {
                    info!("ACP server: received shutdown signal");
                    break;
                }
            };

            let bytes_read = match read_result {
                Ok(0) => {
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

            let response = self.handler.handle(request).await;
            send_response(&mut stdout, &response).await?;

            request_count += 1;
            if request_count.is_multiple_of(GC_INTERVAL_REQUESTS) {
                let cleaned = self.handler.sessions_mut().cleanup_old_sessions();
                if cleaned > 0 {
                    debug!(cleaned, "GC: removed old sessions");
                }
            }
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

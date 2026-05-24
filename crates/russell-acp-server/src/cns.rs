// SPDX-License-Identifier: MIT OR Apache-2.0
//! CNS span emission for ACP server (T2-3).
//!
//! Lightweight emitter that sends structured observability spans to hKask CNS.
//! Gracefully degrades to local logging when no endpoint is configured (JR-2).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// CNS span — structured event for observability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcpCnsSpan {
    /// Span name (e.g., "cns.russell.acp.skill.dispatch")
    pub name: String,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Source identifier
    pub source: String,
    /// Span attributes
    pub attributes: serde_json::Value,
}

/// CNS span emitter for ACP server.
#[derive(Clone)]
pub struct AcpCnsEmitter {
    /// Source identifier (e.g., "russell-acp-server")
    source: String,
    /// CNS endpoint (if configured)
    cns_endpoint: Option<String>,
    /// HTTP client for CNS emission
    http_client: Option<reqwest::Client>,
}

impl AcpCnsEmitter {
    /// Create a new ACP CNS emitter.
    pub fn new(source: impl Into<String>) -> Self {
        let cns_endpoint = std::env::var("HKASK_CNS_ENDPOINT").ok();
        let http_client = cns_endpoint
            .as_ref()
            .and_then(|_| reqwest::Client::builder().build().ok());

        Self {
            source: source.into(),
            cns_endpoint,
            http_client,
        }
    }

    /// Emit a CNS span.
    fn emit(&self, span: AcpCnsSpan) {
        tracing::debug!(span = %span.name, "Emitting ACP CNS span");

        if let (Some(endpoint), Some(client)) = (&self.cns_endpoint, &self.http_client) {
            let span_clone = span.clone();
            let endpoint_clone = endpoint.clone();
            let client_clone = client.clone();

            tokio::spawn(async move {
                let _ = send_to_cns(&client_clone, &endpoint_clone, span_clone).await;
            });
            tracing::info!("ACP CNS span emitted: {} → {}", span.name, endpoint);
        } else {
            tracing::info!("ACP CNS span (local only): {}", span.name);
        }
    }

    /// Emit skill dispatched span.
    pub fn emit_skill_dispatched(&self, skill_id: &str, action: &str) {
        let span = AcpCnsSpan {
            name: "cns.russell.acp.skill.dispatch".to_string(),
            timestamp: Utc::now(),
            source: self.source.clone(),
            attributes: serde_json::json!({
                "skill_id": skill_id,
                "action": action
            }),
        };
        self.emit(span);
    }

    /// Emit LLM escalation span.
    pub fn emit_llm_escalation(&self, backend: &str, model: Option<&str>, latency_ms: u64) {
        let span = AcpCnsSpan {
            name: "cns.russell.acp.llm.escalation".to_string(),
            timestamp: Utc::now(),
            source: self.source.clone(),
            attributes: serde_json::json!({
                "backend": backend,
                "model": model,
                "latency_ms": latency_ms
            }),
        };
        self.emit(span);
    }

    /// Emit session created span.
    pub fn emit_session_created(&self, session_id: &str, persona: &str) {
        let span = AcpCnsSpan {
            name: "cns.russell.acp.session.created".to_string(),
            timestamp: Utc::now(),
            source: self.source.clone(),
            attributes: serde_json::json!({
                "session_id": session_id,
                "persona": persona
            }),
        };
        self.emit(span);
    }

    /// Emit consent decision span.
    pub fn emit_consent_decision(&self, action_id: &str, decision: &str) {
        let span = AcpCnsSpan {
            name: "cns.russell.acp.consent.decision".to_string(),
            timestamp: Utc::now(),
            source: self.source.clone(),
            attributes: serde_json::json!({
                "action_id": action_id,
                "decision": decision
            }),
        };
        self.emit(span);
    }
}

/// Send CNS span to hKask endpoint.
async fn send_to_cns(
    client: &reqwest::Client,
    endpoint: &str,
    span: AcpCnsSpan,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let response = client
        .post(endpoint)
        .json(&span)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await?;

    if response.status().is_success() {
        tracing::debug!("ACP CNS span accepted");
        Ok(())
    } else {
        tracing::warn!("ACP CNS span rejected: {}", response.status());
        Err("CNS rejected span".into())
    }
}

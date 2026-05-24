// SPDX-License-Identifier: MIT OR Apache-2.0
//! CNS span emission for ACP server (T2-3).
//!
//! Defines the `CnsPort` trait (hexagonal port) and provides adapters:
//! - `AcpCnsEmitter` — HTTP adapter (sends spans to hKask CNS)
//! - `LoggingCnsAdapter` — logs spans locally, no network
//! - `NoopCnsAdapter` — discards all spans (for testing/benchmarking)
//!
//! Gracefully degrades to local logging when no endpoint is configured (JR-2).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// CNS port — hexagonal trait for observability span emission.
///
/// Implementations decide what happens with emitted spans:
/// HTTP delivery, local logging, or silent discard.
pub trait CnsPort: Send + Sync {
    /// Emit a skill dispatch span.
    fn emit_skill_dispatched(&self, skill_id: &str, action: &str);

    /// Emit an LLM escalation span.
    fn emit_llm_escalation(&self, backend: &str, model: Option<&str>, latency_ms: u64);

    /// Emit a session creation span.
    fn emit_session_created(&self, session_id: &str, persona: &str);

    /// Emit a consent decision span.
    fn emit_consent_decision(&self, action_id: &str, decision: &str);
}

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

/// HTTP adapter — sends spans to hKask CNS endpoint.
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

    /// Emit a CNS span via HTTP or local logging.
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
}

impl CnsPort for AcpCnsEmitter {
    fn emit_skill_dispatched(&self, skill_id: &str, action: &str) {
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

    fn emit_llm_escalation(&self, backend: &str, model: Option<&str>, latency_ms: u64) {
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

    fn emit_session_created(&self, session_id: &str, persona: &str) {
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

    fn emit_consent_decision(&self, action_id: &str, decision: &str) {
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

/// Logging adapter — emits spans as structured log lines, no network I/O.
pub struct LoggingCnsAdapter {
    source: String,
}

impl LoggingCnsAdapter {
    /// Create a new logging CNS adapter.
    pub fn new(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
        }
    }
}

impl CnsPort for LoggingCnsAdapter {
    fn emit_skill_dispatched(&self, skill_id: &str, action: &str) {
        tracing::info!(source = %self.source, skill_id, action, "CNS: skill dispatched");
    }

    fn emit_llm_escalation(&self, backend: &str, model: Option<&str>, latency_ms: u64) {
        tracing::info!(
            source = %self.source, backend, model, latency_ms, "CNS: LLM escalation"
        );
    }

    fn emit_session_created(&self, session_id: &str, persona: &str) {
        tracing::info!(
            source = %self.source, session_id, persona, "CNS: session created"
        );
    }

    fn emit_consent_decision(&self, action_id: &str, decision: &str) {
        tracing::info!(
            source = %self.source, action_id, decision, "CNS: consent decision"
        );
    }
}

/// No-op adapter — discards all spans. Useful for testing and benchmarking.
pub struct NoopCnsAdapter;

impl CnsPort for NoopCnsAdapter {
    fn emit_skill_dispatched(&self, _skill_id: &str, _action: &str) {}
    fn emit_llm_escalation(&self, _backend: &str, _model: Option<&str>, _latency_ms: u64) {}
    fn emit_session_created(&self, _session_id: &str, _persona: &str) {}
    fn emit_consent_decision(&self, _action_id: &str, _decision: &str) {}
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noop_adapter_satisfies_port() {
        let port: Box<dyn CnsPort> = Box::new(NoopCnsAdapter);
        port.emit_skill_dispatched("test", "run");
        port.emit_llm_escalation("hkask", None, 100);
        port.emit_session_created("s1", "jack");
        port.emit_consent_decision("a1", "approved");
    }

    #[test]
    fn logging_adapter_satisfies_port() {
        let port: Box<dyn CnsPort> = Box::new(LoggingCnsAdapter::new("test"));
        port.emit_skill_dispatched("test", "run");
        port.emit_llm_escalation("hkask", Some("llama3"), 50);
        port.emit_session_created("s1", "jack");
        port.emit_consent_decision("a1", "denied");
    }
}

// SPDX-License-Identifier: MIT OR Apache-2.0
//! CNS span emission for Russell agent.
//!
//! Spans are sent to a CNS endpoint at `POST /api/v1/cns/span`.
//! The span is mapped to a NuEvent-compatible schema for compatibility.
//! When no endpoint is configured, spans are logged locally (graceful degradation).

use crate::persona::AgentPersona;
use crate::pod::PodID;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Default CNS endpoint.
pub const DEFAULT_CNS_ENDPOINT: &str = "http://127.0.0.1:8080/api/v1/cns/span";

/// Environment variable for overriding the CNS endpoint.
const ENV_CNS_ENDPOINT: &str = "RUSSELL_CNS_ENDPOINT";

/// CNS port — hexagonal trait for observability span emission.
///
/// Implementations decide what happens with emitted spans:
/// HTTP delivery, local logging, or silent discard.
pub trait CnsPort: Send + Sync {
    /// Emit pod populated span.
    fn emit_populated(&self);

    /// Emit pod registered span.
    fn emit_registered(&self);

    /// Emit pod activated span.
    fn emit_activated(&self);

    /// Emit pod deactivated span.
    fn emit_deactivated(&self);

    /// Emit probe executed span.
    fn emit_probe_executed(&self, probe_id: &str, skill_id: &str);

    /// Emit skill dispatched span.
    fn emit_skill_dispatched(&self, skill_id: &str, action: &str);

    /// Emit LLM escalation span.
    fn emit_llm_escalation(&self, model: &str, latency_ms: u64);
}

/// CNS span — structured event for observability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CnsSpan {
    /// Span name (e.g., "cns.russell.activated")
    pub name: String,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Pod ID
    pub pod_id: String,
    /// Agent name
    pub agent_name: String,
    /// Span attributes
    pub attributes: serde_json::Value,
}

impl CnsSpan {
    /// Create a new CNS span.
    pub fn new(
        name: &str,
        pod_id: &PodID,
        agent_name: &str,
        attributes: serde_json::Value,
    ) -> Self {
        Self {
            name: name.to_string(),
            timestamp: Utc::now(),
            pod_id: pod_id.to_string(),
            agent_name: agent_name.to_string(),
            attributes,
        }
    }

    /// Convert to NuEvent-compatible JSON for the CNS bridge.
    ///
    /// Maps Russell's flat span to a tagged schema:
    /// - `span` → `{ category, path }` extracted from the span name
    /// - `observer_webid` → pod_id
    /// - `phase` → "Observe" (Russell is always observing)
    /// - `observation` → attributes
    pub fn to_nu_event(&self) -> serde_json::Value {
        let (category, path) = self.parse_span_name();

        serde_json::json!({
            "id": format!("russell-{}", self.pod_id),
            "timestamp": self.timestamp.to_rfc3339(),
            "observer_webid": self.pod_id,
            "span": {
                "category": category,
                "path": path,
            },
            "phase": "Observe",
            "observation": self.attributes,
            "visibility": "private",
        })
    }

    /// Parse span name into (category, path).
    ///
    /// E.g., "cns.russell.activated" → ("AgentPod", "russell/activated")
    fn parse_span_name(&self) -> (String, String) {
        let parts: Vec<&str> = self.name.splitn(4, '.').collect();
        match parts.as_slice() {
            ["cns", agent, domain, action] => {
                let category = capitalize_first(domain);
                (category, format!("{agent}/{domain}/{action}"))
            }
            ["cns", agent, action] => ("AgentPod".into(), format!("{agent}/{action}")),
            _ => ("AgentPod".into(), self.name.clone()),
        }
    }
}

/// Capitalize the first character of a string.
fn capitalize_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

/// CNS span emitter — sends spans to the configured CNS endpoint.
#[derive(Clone)]
pub struct CnsEmitter {
    /// Pod ID
    pod_id: PodID,
    /// Agent name
    agent_name: String,
    /// CNS endpoint (if configured)
    cns_endpoint: Option<String>,
    /// HTTP client for CNS emission
    http_client: Option<reqwest::Client>,
}

impl CnsEmitter {
    /// Create a new CNS emitter.
    pub fn new(pod_id: &PodID, persona: &AgentPersona) -> Self {
        let cns_endpoint = std::env::var(ENV_CNS_ENDPOINT).ok();
        let http_client = cns_endpoint
            .as_ref()
            .and_then(|_| reqwest::Client::builder().build().ok());

        Self {
            pod_id: pod_id.clone(),
            agent_name: persona.name().to_string(),
            cns_endpoint,
            http_client,
        }
    }

    /// Emit a CNS span.
    fn emit(&self, span: CnsSpan) {
        tracing::debug!(span = %span.name, "Emitting CNS span");

        // If CNS endpoint configured and HTTP client available, send via HTTP
        if let (Some(endpoint), Some(client)) = (&self.cns_endpoint, &self.http_client) {
            let span_clone = span.clone();
            let endpoint_clone = endpoint.clone();
            let client_clone = client.clone();

            tokio::spawn(async move {
                let _ = send_to_cns(&client_clone, &endpoint_clone, span_clone).await;
            });
            tracing::info!("CNS span emitted: {} → {}", span.name, endpoint);
        } else {
            // No CNS endpoint — log locally (graceful degradation per JR-2)
            tracing::info!("CNS span (local only): {}", span.name);
        }
    }

    /// Emit pod populated span.
    pub fn emit_populated(&self) {
        let span = CnsSpan::new(
            "cns.russell.populated",
            &self.pod_id,
            &self.agent_name,
            serde_json::json!({
                "state": "populated",
                "persona_version": env!("CARGO_PKG_VERSION")
            }),
        );
        self.emit(span);
    }

    /// Emit pod registered span.
    pub fn emit_registered(&self) {
        let span = CnsSpan::new(
            "cns.russell.registered",
            &self.pod_id,
            &self.agent_name,
            serde_json::json!({
                "state": "registered",
                "hcp_runtime": "russell"
            }),
        );
        self.emit(span);
    }

    /// Emit pod activated span.
    pub fn emit_activated(&self) {
        let span = CnsSpan::new(
            "cns.russell.activated",
            &self.pod_id,
            &self.agent_name,
            serde_json::json!({
                "state": "activated",
                "sentinel": "running",
                "acp_server": "running"
            }),
        );
        self.emit(span);
    }

    /// Emit pod deactivated span.
    pub fn emit_deactivated(&self) {
        let span = CnsSpan::new(
            "cns.russell.deactivated",
            &self.pod_id,
            &self.agent_name,
            serde_json::json!({
                "state": "deactivated",
                "reason": "user_requested"
            }),
        );
        self.emit(span);
    }

    /// Emit probe executed span.
    pub fn emit_probe_executed(&self, probe_id: &str, skill_id: &str) {
        let span = CnsSpan::new(
            "cns.russell.probe.executed",
            &self.pod_id,
            &self.agent_name,
            serde_json::json!({
                "probe_id": probe_id,
                "skill_id": skill_id
            }),
        );
        self.emit(span);
    }

    /// Emit skill dispatched span.
    pub fn emit_skill_dispatched(&self, skill_id: &str, action: &str) {
        let span = CnsSpan::new(
            "cns.russell.skill.dispatch",
            &self.pod_id,
            &self.agent_name,
            serde_json::json!({
                "skill_id": skill_id,
                "action": action
            }),
        );
        self.emit(span);
    }

    /// Emit LLM escalation span.
    pub fn emit_llm_escalation(&self, model: &str, latency_ms: u64) {
        let span = CnsSpan::new(
            "cns.russell.llm.escalation",
            &self.pod_id,
            &self.agent_name,
            serde_json::json!({
                "model": model,
                "latency_ms": latency_ms
            }),
        );
        self.emit(span);
    }
}

/// Logging CNS adapter — emits spans as structured log lines, no network I/O.
pub struct LoggingCnsAdapter {
    pod_id: PodID,
}

impl LoggingCnsAdapter {
    /// Create a new logging CNS adapter.
    pub fn new(pod_id: &PodID, _persona: &AgentPersona) -> Self {
        Self {
            pod_id: pod_id.clone(),
        }
    }
}

impl CnsPort for LoggingCnsAdapter {
    fn emit_populated(&self) {
        tracing::info!(pod_id = %self.pod_id, "CNS: pod populated");
    }

    fn emit_registered(&self) {
        tracing::info!(pod_id = %self.pod_id, "CNS: pod registered");
    }

    fn emit_activated(&self) {
        tracing::info!(pod_id = %self.pod_id, "CNS: pod activated");
    }

    fn emit_deactivated(&self) {
        tracing::info!(pod_id = %self.pod_id, "CNS: pod deactivated");
    }

    fn emit_probe_executed(&self, probe_id: &str, skill_id: &str) {
        tracing::info!(pod_id = %self.pod_id, probe_id, skill_id, "CNS: probe executed");
    }

    fn emit_skill_dispatched(&self, skill_id: &str, action: &str) {
        tracing::info!(pod_id = %self.pod_id, skill_id, action, "CNS: skill dispatched");
    }

    fn emit_llm_escalation(&self, model: &str, latency_ms: u64) {
        tracing::info!(pod_id = %self.pod_id, model, latency_ms, "CNS: LLM escalation");
    }
}

/// No-op CNS adapter — discards all spans. Useful for testing and benchmarking.
pub struct NoopCnsAdapter;

impl CnsPort for NoopCnsAdapter {
    fn emit_populated(&self) {}
    fn emit_registered(&self) {}
    fn emit_activated(&self) {}
    fn emit_deactivated(&self) {}
    fn emit_probe_executed(&self, _probe_id: &str, _skill_id: &str) {}
    fn emit_skill_dispatched(&self, _skill_id: &str, _action: &str) {}
    fn emit_llm_escalation(&self, _model: &str, _latency_ms: u64) {}
}

impl CnsPort for CnsEmitter {
    fn emit_populated(&self) {
        self.emit_populated();
    }

    fn emit_registered(&self) {
        self.emit_registered();
    }

    fn emit_activated(&self) {
        self.emit_activated();
    }

    fn emit_deactivated(&self) {
        self.emit_deactivated();
    }

    fn emit_probe_executed(&self, probe_id: &str, skill_id: &str) {
        self.emit_probe_executed(probe_id, skill_id);
    }

    fn emit_skill_dispatched(&self, skill_id: &str, action: &str) {
        self.emit_skill_dispatched(skill_id, action);
    }

    fn emit_llm_escalation(&self, model: &str, latency_ms: u64) {
        self.emit_llm_escalation(model, latency_ms);
    }
}

/// Send CNS span to the configured endpoint.
async fn send_to_cns(
    client: &reqwest::Client,
    endpoint: &str,
    span: CnsSpan,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let nu_event = span.to_nu_event();
    let response = client
        .post(endpoint)
        .json(&nu_event)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await?;

    if response.status().is_success() {
        tracing::debug!("CNS span accepted");
        Ok(())
    } else {
        tracing::warn!("CNS span rejected: {}", response.status());
        Err("CNS rejected span".into())
    }
}

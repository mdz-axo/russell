// SPDX-License-Identifier: MIT OR Apache-2.0
//! CNS span emission for Russell agent.

use crate::pod::PodID;
use crate::persona::AgentPersona;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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
    pub fn new(name: &str, pod_id: &PodID, agent_name: &str, attributes: serde_json::Value) -> Self {
        Self {
            name: name.to_string(),
            timestamp: Utc::now(),
            pod_id: pod_id.to_string(),
            agent_name: agent_name.to_string(),
            attributes,
        }
    }
}

/// CNS span emitter — sends spans to hKask CNS.
pub struct CnsEmitter {
    /// Pod ID
    pod_id: PodID,
    /// Agent name
    agent_name: String,
    /// CNS endpoint (if configured)
    cns_endpoint: Option<String>,
}

impl CnsEmitter {
    /// Create a new CNS emitter.
    pub fn new(pod_id: &PodID, persona: &AgentPersona) -> Self {
        Self {
            pod_id: pod_id.clone(),
            agent_name: persona.name().to_string(),
            cns_endpoint: std::env::var("HKASK_CNS_ENDPOINT").ok(),
        }
    }
    
    /// Emit a CNS span.
    fn emit(&self, span: CnsSpan) {
        tracing::debug!(span = %span.name, "Emitting CNS span");
        
        // If CNS endpoint configured, send via HTTP
        if let Some(endpoint) = &self.cns_endpoint {
            // tokio::spawn(send_to_cns(endpoint, span));
            // For now, just log
            tracing::info!("CNS span: {} (endpoint: {})", span.name, endpoint);
        } else {
            // No CNS endpoint — log locally (graceful degradation)
            tracing::info!("CNS span: {}", span.name);
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
                "persona_version": "0.20.0"
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
                "hcp_runtime": "hKask"
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

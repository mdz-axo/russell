// SPDX-License-Identifier: MIT OR Apache-2.0
//! Reflex arcs — automatic responses to proprioception breaches.
//!
//! Phase 2A: Detection-only. Reflex arcs recommend actions but do not execute.
//! Phase 3+: Corrective arcs will execute automatic remediation.
//!
//! See [ADR-0021](../../../docs/adr/0021-proprioception-phase2-reflex-arcs.md).

use crate::ProprioResult;
use russell_core::Result;
use russell_core::event::{Event, Scope, Severity as EventSeverity};
use russell_core::journal::JournalWriter;

/// Reflex arc action recommendation.
#[derive(Debug, Clone)]
pub struct ReflexAction {
    /// Action ID (e.g., "restart-sentinel")
    pub action_id: &'static str,
    /// Human-readable description
    pub description: &'static str,
    /// Risk level for execution
    pub risk: RiskLevel,
    /// Trigger condition that fired this action
    pub trigger: &'static str,
}

/// Risk level for reflex actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskLevel {
    /// Safe to auto-execute (no state mutation)
    Low,
    /// Requires operator consent
    Medium,
    /// Requires explicit approval + rollback plan
    High,
}

/// Reflex arc engine — maps breaches to recommended actions.
pub struct ReflexArc {
    /// Pending actions to recommend
    actions: Vec<ReflexAction>,
}

impl ReflexArc {
    /// Create a new reflex arc engine.
    pub fn new() -> Self {
        Self {
            actions: Vec::new(),
        }
    }

    /// Evaluate proprioception result and queue reflex actions.
    pub fn evaluate(&mut self, result: &ProprioResult) {
        // Sentinel age > 30 min → restart sentinel
        if let Some(age_s) = result.age_s {
            if age_s > 1800 {
                self.actions.push(ReflexAction {
                    action_id: "restart-sentinel",
                    description: "Restart sentinel timer (age > 30 min)",
                    risk: RiskLevel::Low,
                    trigger: "sentinel_last_run_age_s > 1800s",
                });
            }
        }

        // Journal stall > 5 min → flush journal
        if let Some(stall_s) = result.journal_stall_s {
            if stall_s > 300 {
                self.actions.push(ReflexAction {
                    action_id: "flush-journal",
                    description: "Force journal flush (stall > 5 min)",
                    risk: RiskLevel::Low,
                    trigger: "journal_writer_stall_s > 300s",
                });
            }
        }

        // LLM p95 > 20s → recommend fallback to offline mode
        if let Some(llm_ms) = result.llm_p95_latency_ms {
            if llm_ms > 20_000.0 {
                self.actions.push(ReflexAction {
                    action_id: "llm-fallback",
                    description: "Switch to offline fallback (LLM p95 > 20s)",
                    risk: RiskLevel::Medium,
                    trigger: "llm_p95_latency_ms > 20000ms",
                });
            }
        }

        // Timer drift > 5 min → restart timer
        if let Some(drift_s) = result.timer_drift_s {
            if drift_s > 300 {
                self.actions.push(ReflexAction {
                    action_id: "restart-timer",
                    description: "Restart systemd timer (drift > 5 min)",
                    risk: RiskLevel::Medium,
                    trigger: "timer_drift_s > 300s",
                });
            }
        }

        // Help error rate > 50% → disable LLM help temporarily
        if let Some(error_rate) = result.help_error_rate_pct {
            if error_rate > 50.0 {
                self.actions.push(ReflexAction {
                    action_id: "disable-llm-help",
                    description: "Temporarily disable LLM help (error rate > 50%)",
                    risk: RiskLevel::High,
                    trigger: "help_error_rate_pct > 50%",
                });
            }
        }
    }

    /// Get queued actions.
    pub fn actions(&self) -> &[ReflexAction] {
        &self.actions
    }

    /// Clear queued actions.
    pub fn clear(&mut self) {
        self.actions.clear();
    }

    /// Log reflex actions to journal (Phase 2A: detection-only).
    pub fn log_actions(&self, writer: &JournalWriter) -> Result<()> {
        for action in &self.actions {
            let mut ev = Event::new("reflex_arc_action", EventSeverity::Warn);
            ev.scope = Scope::Self_;
            ev.tier = Some("proprio".into());
            ev.module = Some("reflex-arc".into());
            ev.summary = Some(format!("Recommended: {}", action.description));
            ev.outputs
                .insert("action_id".into(), action.action_id.into());
            ev.outputs
                .insert("risk".into(), format!("{:?}", action.risk).into());
            ev.outputs.insert("trigger".into(), action.trigger.into());
            writer.append(&ev)?;
        }
        Ok(())
    }
}

impl Default for ReflexArc {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Severity;

    #[test]
    fn test_reflex_arc_sentinel_age() {
        let mut arc = ReflexArc::new();
        let result = ProprioResult {
            age_s: Some(2000),
            severity: Severity::Alert,
            event_emitted: true,
            journal_stall_s: None,
            journal_stall_severity: Severity::Info,
            llm_p95_latency_ms: None,
            llm_p95_severity: Severity::Info,
            timer_drift_s: None,
            timer_drift_severity: Severity::Info,
            help_error_rate_pct: None,
            help_error_rate_severity: Severity::Info,
            hkask_mcp_reachable_ms: None,
            hkask_mcp_reachable_severity: Severity::Info,
            remote_discovery_latency_s: None,
            remote_discovery_latency_severity: Severity::Info,
            journal_chain_intact: None,
        };

        arc.evaluate(&result);
        assert_eq!(arc.actions().len(), 1);
        assert_eq!(arc.actions()[0].action_id, "restart-sentinel");
    }

    #[test]
    fn test_reflex_arc_no_breaches() {
        let mut arc = ReflexArc::new();
        let result = ProprioResult {
            age_s: Some(100),
            severity: Severity::Info,
            event_emitted: false,
            journal_stall_s: None,
            journal_stall_severity: Severity::Info,
            llm_p95_latency_ms: None,
            llm_p95_severity: Severity::Info,
            timer_drift_s: None,
            timer_drift_severity: Severity::Info,
            help_error_rate_pct: None,
            help_error_rate_severity: Severity::Info,
            hkask_mcp_reachable_ms: None,
            hkask_mcp_reachable_severity: Severity::Info,
            remote_discovery_latency_s: None,
            remote_discovery_latency_severity: Severity::Info,
            journal_chain_intact: None,
        };

        arc.evaluate(&result);
        assert_eq!(arc.actions().len(), 0);
    }
}

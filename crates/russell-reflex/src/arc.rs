// SPDX-License-Identifier: MIT OR Apache-2.0
//! Reflex arc engine — maps breaches to recommended actions.

use crate::action::{
    ReflexAction, ACTION_DISABLE_LLM_HELP, ACTION_FLUSH_JOURNAL, ACTION_LLM_FALLBACK,
    ACTION_RESTART_SENTINEL, ACTION_RESTART_TIMER,
};
use crate::risk::RiskLevel;
use russell_proprio::ProprioResult;

/// Reflex arc engine — maps proprioception breaches to recommended actions.
///
/// ## Phase 2A: Detection-Only
///
/// This engine only recommends actions. It does not execute them.
/// Execution is deferred to Phase 3+ when operator pre-approval workflows
/// are implemented.
pub struct ReflexArc {
    /// Pending actions to recommend.
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
                self.actions.push(ReflexAction::new(
                    ACTION_RESTART_SENTINEL,
                    "Restart sentinel timer (age > 30 min)",
                    RiskLevel::Low,
                    "sentinel_last_run_age_s > 1800s",
                ));
            }
        }

        // Journal stall > 5 min → flush journal
        if let Some(stall_s) = result.journal_stall_s {
            if stall_s > 300 {
                self.actions.push(ReflexAction::new(
                    ACTION_FLUSH_JOURNAL,
                    "Force journal flush (stall > 5 min)",
                    RiskLevel::Low,
                    "journal_writer_stall_s > 300s",
                ));
            }
        }

        // LLM p95 > 20s → recommend fallback to offline mode
        if let Some(llm_ms) = result.llm_p95_latency_ms {
            if llm_ms > 20_000.0 {
                self.actions.push(ReflexAction::new(
                    ACTION_LLM_FALLBACK,
                    "Switch to offline fallback (LLM p95 > 20s)",
                    RiskLevel::Medium,
                    "llm_p95_latency_ms > 20000ms",
                ));
            }
        }

        // Timer drift > 5 min → restart timer
        if let Some(drift_s) = result.timer_drift_s {
            if drift_s > 300 {
                self.actions.push(ReflexAction::new(
                    ACTION_RESTART_TIMER,
                    "Restart systemd timer (drift > 5 min)",
                    RiskLevel::Medium,
                    "timer_drift_s > 300s",
                ));
            }
        }

        // Help error rate > 50% → disable LLM help temporarily
        if let Some(error_rate) = result.help_error_rate_pct {
            if error_rate > 50.0 {
                self.actions.push(ReflexAction::new(
                    ACTION_DISABLE_LLM_HELP,
                    "Temporarily disable LLM help (error rate > 50%)",
                    RiskLevel::High,
                    "help_error_rate_pct > 50%",
                ));
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
    pub fn log_actions(&self, writer: &russell_core::journal::JournalWriter) -> russell_core::Result<()> {
        for action in &self.actions {
            let mut ev = russell_core::event::Event::new("reflex_arc_action", russell_core::event::Severity::Warn);
            ev.scope = russell_core::event::Scope::Self_;
            ev.tier = Some("proprio".into());
            ev.module = Some("reflex-arc".into());
            ev.summary = Some(format!("Recommended: {}", action.description));
            ev.outputs.insert("action_id".into(), action.action_id.into());
            ev.outputs.insert("risk".into(), format!("{:?}", action.risk).into());
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
    use russell_core::event::Severity;

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
        assert_eq!(arc.actions()[0].action_id, ACTION_RESTART_SENTINEL);
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

    #[test]
    fn test_reflex_arc_multiple_breaches() {
        let mut arc = ReflexArc::new();
        let result = ProprioResult {
            age_s: Some(2000),
            severity: Severity::Alert,
            event_emitted: true,
            journal_stall_s: Some(400),
            journal_stall_severity: Severity::Alert,
            llm_p95_latency_ms: Some(25_000.0),
            llm_p95_severity: Severity::Alert,
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
        assert_eq!(arc.actions().len(), 3);
        assert_eq!(arc.actions()[0].action_id, ACTION_RESTART_SENTINEL);
        assert_eq!(arc.actions()[1].action_id, ACTION_FLUSH_JOURNAL);
        assert_eq!(arc.actions()[2].action_id, ACTION_LLM_FALLBACK);
    }
}

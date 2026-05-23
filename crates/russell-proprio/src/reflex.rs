// SPDX-License-Identifier: MIT OR Apache-2.0
//! Proprioception reflex arcs — maps self-vital breaches to recommended actions.
//!
//! **Phase 2A (Current):** Detection-only. Reflex arcs recommend actions but do not execute.
//! This respects JR-2 (observe > recommend > act) while building the foundation for
//! automatic remediation.
//!
//! Consolidated from the former `russell-reflex` crate per C7 (when implementations
//! diverge, one must yield). The proprioception-specific reflex engine belongs here
//! since it operates exclusively on [`ProprioResult`].

use russell_core::risk::RiskBand;

use crate::ProprioResult;

/// Action ID: restart sentinel timer.
pub const ACTION_RESTART_SENTINEL: &str = "restart-sentinel";
/// Action ID: force journal flush.
pub const ACTION_FLUSH_JOURNAL: &str = "flush-journal";
/// Action ID: switch to offline LLM fallback.
pub const ACTION_LLM_FALLBACK: &str = "llm-fallback";
/// Action ID: restart systemd timer.
pub const ACTION_RESTART_TIMER: &str = "restart-timer";
/// Action ID: temporarily disable LLM help.
pub const ACTION_DISABLE_LLM_HELP: &str = "disable-llm-help";

/// Reflex arc action recommendation.
#[derive(Debug, Clone)]
pub struct ReflexAction {
    /// Action ID (e.g., "restart-sentinel").
    pub action_id: &'static str,
    /// Human-readable description.
    pub description: &'static str,
    /// Risk level for execution.
    pub risk: RiskBand,
    /// Trigger condition that fired this action.
    pub trigger: &'static str,
}

impl ReflexAction {
    /// Create a new reflex action.
    pub const fn new(
        action_id: &'static str,
        description: &'static str,
        risk: RiskBand,
        trigger: &'static str,
    ) -> Self {
        Self {
            action_id,
            description,
            risk,
            trigger,
        }
    }
}

/// Proprioception reflex arc engine — maps self-vital breaches to recommended actions.
///
/// ## Phase 2A: Detection-Only
///
/// This engine only recommends actions. It does not execute them.
/// Execution is deferred to Phase 3+ when operator pre-approval workflows
/// are implemented.
pub struct ProprioReflex {
    /// Pending actions to recommend.
    actions: Vec<ReflexAction>,
}

impl ProprioReflex {
    /// Create a new reflex arc engine.
    pub fn new() -> Self {
        Self {
            actions: Vec::new(),
        }
    }

    /// Evaluate proprioception result and queue reflex actions.
    pub fn evaluate(&mut self, result: &ProprioResult) {
        if result.age_s.is_some_and(|v| v > 1800) {
            self.actions.push(ReflexAction::new(
                ACTION_RESTART_SENTINEL,
                "Restart sentinel timer (age > 30 min)",
                RiskBand::Low,
                "sentinel_last_run_age_s > 1800s",
            ));
        }

        if result.journal_stall_s.is_some_and(|v| v > 300) {
            self.actions.push(ReflexAction::new(
                ACTION_FLUSH_JOURNAL,
                "Force journal flush (stall > 5 min)",
                RiskBand::Low,
                "journal_writer_stall_s > 300s",
            ));
        }

        if result.llm_p95_latency_ms.is_some_and(|v| v > 20_000.0) {
            self.actions.push(ReflexAction::new(
                ACTION_LLM_FALLBACK,
                "Switch to offline fallback (LLM p95 > 20s)",
                RiskBand::Medium,
                "llm_p95_latency_ms > 20000ms",
            ));
        }

        if result.timer_drift_s.is_some_and(|v| v > 300) {
            self.actions.push(ReflexAction::new(
                ACTION_RESTART_TIMER,
                "Restart systemd timer (drift > 5 min)",
                RiskBand::Medium,
                "timer_drift_s > 300s",
            ));
        }

        if result.help_error_rate_pct.is_some_and(|v| v > 50.0) {
            self.actions.push(ReflexAction::new(
                ACTION_DISABLE_LLM_HELP,
                "Temporarily disable LLM help (error rate > 50%)",
                RiskBand::High,
                "help_error_rate_pct > 50%",
            ));
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
    pub fn log_actions(
        &self,
        writer: &russell_core::journal::JournalWriter,
    ) -> russell_core::Result<()> {
        for action in &self.actions {
            let mut ev = russell_core::event::Event::new(
                "reflex_arc_action",
                russell_core::event::Severity::Warn,
            );
            ev.scope = russell_core::event::Scope::Self_;
            ev.tier = Some("proprio".into());
            ev.module = Some("reflex-arc".into());
            ev.summary = Some(format!("Recommended: {}", action.description));
            ev.outputs
                .insert("action_id".into(), action.action_id.into());
            ev.outputs
                .insert("risk".into(), action.risk.as_str().into());
            ev.outputs.insert("trigger".into(), action.trigger.into());
            writer.append(&ev)?;
        }
        Ok(())
    }
}

impl Default for ProprioReflex {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use russell_core::event::Severity;

    fn make_result(age_s: Option<i64>, stall: Option<i64>, llm: Option<f64>) -> ProprioResult {
        ProprioResult {
            age_s,
            severity: Severity::Info,
            event_emitted: false,
            journal_stall_s: stall,
            journal_stall_severity: Severity::Info,
            llm_p95_latency_ms: llm,
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
        }
    }

    #[test]
    fn reflex_sentinel_age() {
        let mut arc = ProprioReflex::new();
        arc.evaluate(&make_result(Some(2000), None, None));
        assert_eq!(arc.actions().len(), 1);
        assert_eq!(arc.actions()[0].action_id, ACTION_RESTART_SENTINEL);
    }

    #[test]
    fn reflex_no_breaches() {
        let mut arc = ProprioReflex::new();
        arc.evaluate(&make_result(Some(100), None, None));
        assert_eq!(arc.actions().len(), 0);
    }

    #[test]
    fn reflex_multiple_breaches() {
        let mut arc = ProprioReflex::new();
        arc.evaluate(&make_result(Some(2000), Some(400), Some(25_000.0)));
        assert_eq!(arc.actions().len(), 3);
        assert_eq!(arc.actions()[0].action_id, ACTION_RESTART_SENTINEL);
        assert_eq!(arc.actions()[1].action_id, ACTION_FLUSH_JOURNAL);
        assert_eq!(arc.actions()[2].action_id, ACTION_LLM_FALLBACK);
    }
}

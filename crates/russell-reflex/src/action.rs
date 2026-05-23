// SPDX-License-Identifier: MIT OR Apache-2.0
//! Reflex action — recommended automatic response.

use crate::risk::RiskLevel;

/// Reflex action: restart sentinel timer.
pub const ACTION_RESTART_SENTINEL: &str = "restart-sentinel";
/// Reflex action: force journal flush.
pub const ACTION_FLUSH_JOURNAL: &str = "flush-journal";
/// Reflex action: switch to offline LLM fallback.
pub const ACTION_LLM_FALLBACK: &str = "llm-fallback";
/// Reflex action: restart systemd timer.
pub const ACTION_RESTART_TIMER: &str = "restart-timer";
/// Reflex action: temporarily disable LLM help.
pub const ACTION_DISABLE_LLM_HELP: &str = "disable-llm-help";

/// Reflex arc action recommendation.
#[derive(Debug, Clone)]
pub struct ReflexAction {
    /// Action ID (e.g., "restart-sentinel").
    pub action_id: &'static str,
    /// Human-readable description.
    pub description: &'static str,
    /// Risk level for execution.
    pub risk: RiskLevel,
    /// Trigger condition that fired this action.
    pub trigger: &'static str,
}

impl ReflexAction {
    /// Create a new reflex action.
    pub const fn new(
        action_id: &'static str,
        description: &'static str,
        risk: RiskLevel,
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

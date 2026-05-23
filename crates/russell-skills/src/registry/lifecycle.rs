// SPDX-License-Identifier: MIT OR Apache-2.0
//! Skill lifecycle — state machine, transitions, and journal events.
//!
//! The lifecycle state machine per ADR-0024:
//! ```text
//! discovered → evaluated → installed → active → stale_warning → deprecated → retired
//! ```

use russell_core::event::{Event, Severity};
use russell_core::journal::JournalWriter;
use serde::{Deserialize, Serialize};

/// Lifecycle states per ADR-0024.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleStatus {
    /// Found via search, not yet inspected.
    Discovered,
    /// Manifest reviewed, safety scanned.
    Evaluated,
    /// On disk, poka-yoke passed.
    Installed,
    /// Loaded by harness, used in sessions.
    Active,
    /// Authored > 6mo or valid_until passed.
    StaleWarning,
    /// Superseded or no longer relevant.
    Deprecated,
    /// Removed from skills directory.
    Retired,
}

/// Error returned when an invalid lifecycle transition is attempted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidTransition {
    /// The state we're transitioning from.
    pub from: LifecycleStatus,
    /// The state we attempted to transition to.
    pub to: LifecycleStatus,
}

impl std::fmt::Display for InvalidTransition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid lifecycle transition: {} → {}",
            self.from.as_str(),
            self.to.as_str()
        )
    }
}

impl std::error::Error for InvalidTransition {}

impl LifecycleStatus {
    /// Whether skills in this state should be loaded by the harness.
    #[must_use]
    pub fn is_loadable(self) -> bool {
        matches!(
            self,
            LifecycleStatus::Installed | LifecycleStatus::Active | LifecycleStatus::StaleWarning
        )
    }

    /// Human-readable label.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            LifecycleStatus::Discovered => "discovered",
            LifecycleStatus::Evaluated => "evaluated",
            LifecycleStatus::Installed => "installed",
            LifecycleStatus::Active => "active",
            LifecycleStatus::StaleWarning => "stale_warning",
            LifecycleStatus::Deprecated => "deprecated",
            LifecycleStatus::Retired => "retired",
        }
    }

    /// Validate whether a transition from `self` to `to` is permitted.
    ///
    /// Valid transitions per ADR-0024:
    /// - Forward: discovered → evaluated → installed → active → stale_warning → deprecated → retired
    /// - Backward: deprecated → installed (restore)
    /// - Re-evaluation: any non-terminal state → evaluated
    ///
    /// # Errors
    ///
    /// Returns [`InvalidTransition`] if the transition is not permitted.
    pub fn validate_transition(self, to: LifecycleStatus) -> Result<(), InvalidTransition> {
        let valid = match (self, to) {
            // Forward transitions
            (LifecycleStatus::Discovered, LifecycleStatus::Evaluated) => true,
            (LifecycleStatus::Evaluated, LifecycleStatus::Installed) => true,
            (LifecycleStatus::Installed, LifecycleStatus::Active) => true,
            (LifecycleStatus::Active, LifecycleStatus::StaleWarning) => true,
            (LifecycleStatus::StaleWarning, LifecycleStatus::Deprecated) => true,
            (LifecycleStatus::Deprecated, LifecycleStatus::Retired) => true,
            // Backward: restore from deprecated
            (LifecycleStatus::Deprecated, LifecycleStatus::Installed) => true,
            // Re-evaluation: any non-terminal state can be re-evaluated
            (_, LifecycleStatus::Evaluated) if self != LifecycleStatus::Retired => true,
            // Same state is always a no-op (idempotent)
            (a, b) if a == b => true,
            _ => false,
        };

        if valid {
            Ok(())
        } else {
            Err(InvalidTransition { from: self, to })
        }
    }
}

/// Write a lifecycle transition event to the journal.
///
/// Produces a `harness.event.v1` record with action
/// `"skill.lifecycle.transition"` and severity `Info`, satisfying
/// JR-7 (persistence is auditable).
pub fn journal_transition(
    journal: &JournalWriter,
    skill_id: &str,
    from_status: Option<LifecycleStatus>,
    to_status: LifecycleStatus,
    reason: Option<&str>,
) {
    let from_str = from_status.map_or("none", |s| s.as_str());
    let mut ev = Event::new("skill.lifecycle.transition", Severity::Info);
    ev.tier = Some("skill".into());
    ev.module = Some(format!("skill/{skill_id}"));
    ev.summary = Some(format!("{skill_id}: {from_str} → {}", to_status.as_str()));
    ev.outputs.insert("skill_id".into(), skill_id.into());
    ev.outputs.insert("from_status".into(), from_str.into());
    ev.outputs
        .insert("to_status".into(), to_status.as_str().into());
    if let Some(r) = reason {
        ev.outputs.insert("reason".into(), r.into());
    }
    if let Err(e) = journal.append(&ev) {
        tracing::warn!(skill = %skill_id, error = %e, "failed to journal lifecycle transition");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forward_transitions_valid() {
        assert!(
            LifecycleStatus::Discovered
                .validate_transition(LifecycleStatus::Evaluated)
                .is_ok()
        );
        assert!(
            LifecycleStatus::Evaluated
                .validate_transition(LifecycleStatus::Installed)
                .is_ok()
        );
        assert!(
            LifecycleStatus::Installed
                .validate_transition(LifecycleStatus::Active)
                .is_ok()
        );
        assert!(
            LifecycleStatus::Active
                .validate_transition(LifecycleStatus::StaleWarning)
                .is_ok()
        );
        assert!(
            LifecycleStatus::StaleWarning
                .validate_transition(LifecycleStatus::Deprecated)
                .is_ok()
        );
        assert!(
            LifecycleStatus::Deprecated
                .validate_transition(LifecycleStatus::Retired)
                .is_ok()
        );
    }

    #[test]
    fn backward_restore_valid() {
        assert!(
            LifecycleStatus::Deprecated
                .validate_transition(LifecycleStatus::Installed)
                .is_ok()
        );
    }

    #[test]
    fn re_evaluation_valid() {
        assert!(
            LifecycleStatus::Discovered
                .validate_transition(LifecycleStatus::Evaluated)
                .is_ok()
        );
        assert!(
            LifecycleStatus::Installed
                .validate_transition(LifecycleStatus::Evaluated)
                .is_ok()
        );
        assert!(
            LifecycleStatus::Active
                .validate_transition(LifecycleStatus::Evaluated)
                .is_ok()
        );
        assert!(
            LifecycleStatus::StaleWarning
                .validate_transition(LifecycleStatus::Evaluated)
                .is_ok()
        );
        assert!(
            LifecycleStatus::Deprecated
                .validate_transition(LifecycleStatus::Evaluated)
                .is_ok()
        );
    }

    #[test]
    fn retired_cannot_re_evaluate() {
        assert!(
            LifecycleStatus::Retired
                .validate_transition(LifecycleStatus::Evaluated)
                .is_err()
        );
    }

    #[test]
    fn same_state_is_noop() {
        for state in [
            LifecycleStatus::Discovered,
            LifecycleStatus::Evaluated,
            LifecycleStatus::Installed,
            LifecycleStatus::Active,
            LifecycleStatus::StaleWarning,
            LifecycleStatus::Deprecated,
            LifecycleStatus::Retired,
        ] {
            assert!(state.validate_transition(state).is_ok());
        }
    }

    #[test]
    fn skip_forward_invalid() {
        assert!(
            LifecycleStatus::Discovered
                .validate_transition(LifecycleStatus::Installed)
                .is_err()
        );
        assert!(
            LifecycleStatus::Discovered
                .validate_transition(LifecycleStatus::Active)
                .is_err()
        );
        assert!(
            LifecycleStatus::Evaluated
                .validate_transition(LifecycleStatus::Active)
                .is_err()
        );
    }

    #[test]
    fn backward_invalid() {
        assert!(
            LifecycleStatus::Active
                .validate_transition(LifecycleStatus::Installed)
                .is_err()
        );
        assert!(
            LifecycleStatus::Installed
                .validate_transition(LifecycleStatus::Discovered)
                .is_err()
        );
        assert!(
            LifecycleStatus::Retired
                .validate_transition(LifecycleStatus::Installed)
                .is_err()
        );
    }

    #[test]
    fn invalid_transition_error_contains_states() {
        let err = LifecycleStatus::Discovered
            .validate_transition(LifecycleStatus::Retired)
            .unwrap_err();
        assert_eq!(err.from, LifecycleStatus::Discovered);
        assert_eq!(err.to, LifecycleStatus::Retired);
        assert!(err.to_string().contains("discovered"));
        assert!(err.to_string().contains("retired"));
    }

    #[test]
    fn is_loadable_states() {
        assert!(!LifecycleStatus::Discovered.is_loadable());
        assert!(!LifecycleStatus::Evaluated.is_loadable());
        assert!(LifecycleStatus::Installed.is_loadable());
        assert!(LifecycleStatus::Active.is_loadable());
        assert!(LifecycleStatus::StaleWarning.is_loadable());
        assert!(!LifecycleStatus::Deprecated.is_loadable());
        assert!(!LifecycleStatus::Retired.is_loadable());
    }
}

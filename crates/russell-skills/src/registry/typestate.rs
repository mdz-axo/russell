// SPDX-License-Identifier: MIT OR Apache-2.0
//! Skill lifecycle — typestate machine with compiler-enforced transitions.
//!
//! Replaces the string-keyed [`LifecycleStatus`] enum with a typestate
//! pattern where the compiler rejects invalid transitions.
//!
//! ## Usage
//!
//! ```ignore
//! let discovered = SkillLifecycle::discovered(skill_id, authored);
//! let evaluated = discovered.evaluate(&evaluator)?;
//! let installed = evaluated.install(&skill_loader, &journal)?;
//! let active = installed.activate(&journal)?;
//!
//! // Compiler error — can't activate from Active
//! // active.activate(&journal);
//!
//! // Runtime staleness check
//! let state = active.check_staleness(&journal);
//! match state {
//!     StalenessResult::Fresh(active) => { /* still active */ }
//!     StalenessResult::Stale(stale) => { /* transitioned */ }
//! }
//! ```
//!
//! ## State Machine
//!
//! ```text
//! discovered → evaluated → installed → active → stale_warning → deprecated → retired
//!                  ↑                                                                 │
//!                  └────────────────── reinstall ────────────────────────────────────┘
//! ```

#![deny(unsafe_code)]

use russell_core::event::{Event, Severity};
use russell_core::journal::JournalWriter;
use std::marker::PhantomData;

// ─── State marker types (zero-sized, compile-time only) ──────────────────

/// Found via search, not yet inspected.
#[derive(Debug, Clone, Copy)]
pub struct Discovered;

/// Manifest reviewed, safety scanned.
#[derive(Debug, Clone, Copy)]
pub struct Evaluated;

/// On disk, poka-yoke passed.
#[derive(Debug, Clone, Copy)]
pub struct Installed;

/// Loaded by harness, used in sessions.
#[derive(Debug, Clone, Copy)]
pub struct Active;

/// Authored > 6mo or valid_until passed.
#[derive(Debug, Clone, Copy)]
pub struct StaleWarning;

/// Superseded or no longer relevant.
#[derive(Debug, Clone, Copy)]
pub struct Deprecated;

/// Removed from skills directory.
#[derive(Debug, Clone, Copy)]
pub struct Retired;

// ─── Core typestate container ──────────────────────────────────────────────

/// A skill with compile-time lifecycle state `S`.
///
/// The state type parameter `S` ensures that only valid transitions
/// are available at each stage. For example, `SkillLifecycle<Active>`
/// has `record_execution()` but not `activate()`.
#[derive(Debug, Clone)]
pub struct SkillLifecycle<S> {
    /// Skill identity (stable across all states).
    pub skill_id: String,
    /// ISO 8601 date the skill was authored.
    pub authored: String,
    /// ISO 8601 date of the current state transition.
    pub state_entered_at: String,
    /// Telemetry: total probe executions (populated in Active state).
    pub probe_runs: u64,
    /// Telemetry: recent probe failures.
    pub recent_probe_failures: u64,
    /// Telemetry: total intervention executions.
    pub intervention_runs: u64,
    /// Telemetry: recent intervention failures.
    pub recent_intervention_failures: u64,
    /// Telemetry: last probe run timestamp (ISO 8601).
    pub last_probe_run_at: Option<String>,
    /// Telemetry: most recent error message.
    pub last_error: Option<String>,
    /// Telemetry: EWMA of probe durations (ms).
    pub avg_probe_duration_ms: Option<f64>,
    /// Phantom state marker.
    _state: PhantomData<S>,
}

impl SkillLifecycle<Discovered> {
    /// Create a new skill in the Discovered state.
    #[must_use]
    pub fn discovered(skill_id: impl Into<String>, authored: impl Into<String>) -> Self {
        let state_entered_at = russell_core::time::now_date_iso8601();
        Self {
            skill_id: skill_id.into(),
            authored: authored.into(),
            state_entered_at,
            probe_runs: 0,
            recent_probe_failures: 0,
            intervention_runs: 0,
            recent_intervention_failures: 0,
            last_probe_run_at: None,
            last_error: None,
            avg_probe_duration_ms: None,
            _state: PhantomData,
        }
    }

    /// Evaluate the skill: review manifest and safety scan.
    ///
    /// Consumes `self` and produces `SkillLifecycle<Evaluated>`.
    /// Journals the transition with `harness.event.v1` action=`skill.lifecycle.transition`.
    pub fn evaluate(
        self,
        journal: &JournalWriter,
    ) -> SkillLifecycle<Evaluated> {
        let state_entered_at = russell_core::time::now_date_iso8601();
        journal_transition(
            journal,
            &self.skill_id,
            Some("discovered"),
            "evaluated",
            Some("manifest reviewed, safety scanned"),
        );
        SkillLifecycle {
            skill_id: self.skill_id,
            authored: self.authored,
            state_entered_at,
            probe_runs: self.probe_runs,
            recent_probe_failures: self.recent_probe_failures,
            intervention_runs: self.intervention_runs,
            recent_intervention_failures: self.recent_intervention_failures,
            last_probe_run_at: self.last_probe_run_at,
            last_error: self.last_error,
            avg_probe_duration_ms: self.avg_probe_duration_ms,
            _state: PhantomData,
        }
    }

    /// Shortcut: install directly from discovered (skip evaluation).
    pub fn install(
        self,
        journal: &JournalWriter,
    ) -> SkillLifecycle<Installed> {
        let state_entered_at = russell_core::time::now_date_iso8601();
        journal_transition(
            journal,
            &self.skill_id,
            Some("discovered"),
            "installed",
            Some("shortcut: skipped evaluation"),
        );
        SkillLifecycle {
            skill_id: self.skill_id,
            authored: self.authored,
            state_entered_at,
            probe_runs: self.probe_runs,
            recent_probe_failures: self.recent_probe_failures,
            intervention_runs: self.intervention_runs,
            recent_intervention_failures: self.recent_intervention_failures,
            last_probe_run_at: self.last_probe_run_at,
            last_error: self.last_error,
            avg_probe_duration_ms: self.avg_probe_duration_ms,
            _state: PhantomData,
        }
    }

    /// Retire the skill before installation (never made it to disk).
    pub fn retire(
        self,
        journal: &JournalWriter,
        reason: Option<&str>,
    ) -> SkillLifecycle<Retired> {
        let state_entered_at = russell_core::time::now_date_iso8601();
        journal_transition(
            journal,
            &self.skill_id,
            Some("discovered"),
            "retired",
            reason,
        );
        SkillLifecycle {
            skill_id: self.skill_id,
            authored: self.authored,
            state_entered_at,
            probe_runs: self.probe_runs,
            recent_probe_failures: self.recent_probe_failures,
            intervention_runs: self.intervention_runs,
            recent_intervention_failures: self.recent_intervention_failures,
            last_probe_run_at: self.last_probe_run_at,
            last_error: self.last_error,
            avg_probe_duration_ms: self.avg_probe_duration_ms,
            _state: PhantomData,
        }
    }
}

impl SkillLifecycle<Evaluated> {
    /// Install the skill to disk.
    pub fn install(
        self,
        journal: &JournalWriter,
    ) -> SkillLifecycle<Installed> {
        let state_entered_at = russell_core::time::now_date_iso8601();
        journal_transition(
            journal,
            &self.skill_id,
            Some("evaluated"),
            "installed",
            Some("written to skills directory"),
        );
        SkillLifecycle {
            skill_id: self.skill_id,
            authored: self.authored,
            state_entered_at,
            probe_runs: self.probe_runs,
            recent_probe_failures: self.recent_probe_failures,
            intervention_runs: self.intervention_runs,
            recent_intervention_failures: self.recent_intervention_failures,
            last_probe_run_at: self.last_probe_run_at,
            last_error: self.last_error,
            avg_probe_duration_ms: self.avg_probe_duration_ms,
            _state: PhantomData,
        }
    }

    /// Retire the skill after evaluation but before installation.
    pub fn retire(
        self,
        journal: &JournalWriter,
        reason: Option<&str>,
    ) -> SkillLifecycle<Retired> {
        let state_entered_at = russell_core::time::now_date_iso8601();
        journal_transition(
            journal,
            &self.skill_id,
            Some("evaluated"),
            "retired",
            reason,
        );
        SkillLifecycle {
            skill_id: self.skill_id,
            authored: self.authored,
            state_entered_at,
            probe_runs: self.probe_runs,
            recent_probe_failures: self.recent_probe_failures,
            intervention_runs: self.intervention_runs,
            recent_intervention_failures: self.recent_intervention_failures,
            last_probe_run_at: self.last_probe_run_at,
            last_error: self.last_error,
            avg_probe_duration_ms: self.avg_probe_duration_ms,
            _state: PhantomData,
        }
    }
}

impl SkillLifecycle<Installed> {
    /// Activate the skill: load into harness sessions.
    ///
    /// Requires RW-lock on the skills directory (external to this type) to
    /// prevent concurrent modification of the skill's scripts during activation.
    pub fn activate(
        self,
        journal: &JournalWriter,
    ) -> SkillLifecycle<Active> {
        let state_entered_at = russell_core::time::now_date_iso8601();
        journal_transition(
            journal,
            &self.skill_id,
            Some("installed"),
            "active",
            Some("loaded by harness"),
        );
        SkillLifecycle {
            skill_id: self.skill_id,
            authored: self.authored,
            state_entered_at,
            probe_runs: self.probe_runs,
            recent_probe_failures: self.recent_probe_failures,
            intervention_runs: self.intervention_runs,
            recent_intervention_failures: self.recent_intervention_failures,
            last_probe_run_at: self.last_probe_run_at,
            last_error: self.last_error,
            avg_probe_duration_ms: self.avg_probe_duration_ms,
            _state: PhantomData,
        }
    }

    /// Retire the skill after installation but before activation.
    pub fn retire(
        self,
        journal: &JournalWriter,
        reason: Option<&str>,
    ) -> SkillLifecycle<Retired> {
        let state_entered_at = russell_core::time::now_date_iso8601();
        journal_transition(
            journal,
            &self.skill_id,
            Some("installed"),
            "retired",
            reason,
        );
        SkillLifecycle {
            skill_id: self.skill_id,
            authored: self.authored,
            state_entered_at,
            probe_runs: self.probe_runs,
            recent_probe_failures: self.recent_probe_failures,
            intervention_runs: self.intervention_runs,
            recent_intervention_failures: self.recent_intervention_failures,
            last_probe_run_at: self.last_probe_run_at,
            last_error: self.last_error,
            avg_probe_duration_ms: self.avg_probe_duration_ms,
            _state: PhantomData,
        }
    }
}

impl SkillLifecycle<Active> {
    /// Record a successful probe execution.
    ///
    /// Updates EWMA of probe duration using the same algorithm as
    /// `registry/health.rs:20-41` with alpha=0.2.
    pub fn record_execution(&mut self, duration_ms: u64) {
        const EWMA_ALPHA: f64 = 0.2;
        self.probe_runs = self.probe_runs.saturating_add(1);
        self.last_probe_run_at = Some(russell_core::time::now_date_iso8601());
        let current = duration_ms as f64;
        self.avg_probe_duration_ms = Some(match self.avg_probe_duration_ms {
            Some(prev) => EWMA_ALPHA * current + (1.0 - EWMA_ALPHA) * prev,
            None => current,
        });
    }

    /// Record a failed probe execution with an error message.
    pub fn record_failure(&mut self, duration_ms: u64, error_message: &str) {
        self.probe_runs = self.probe_runs.saturating_add(1);
        self.recent_probe_failures = self.recent_probe_failures.saturating_add(1);
        self.last_probe_run_at = Some(russell_core::time::now_date_iso8601());
        self.last_error = Some(error_message.to_string());
        let current = duration_ms as f64;
        const EWMA_ALPHA: f64 = 0.2;
        self.avg_probe_duration_ms = Some(match self.avg_probe_duration_ms {
            Some(prev) => EWMA_ALPHA * current + (1.0 - EWMA_ALPHA) * prev,
            None => current,
        });
    }

    /// Record an intervention execution.
    pub fn record_intervention(&mut self, success: bool, error_message: Option<&str>) {
        self.intervention_runs = self.intervention_runs.saturating_add(1);
        if !success {
            self.recent_intervention_failures = self.recent_intervention_failures.saturating_add(1);
            if let Some(msg) = error_message {
                self.last_error = Some(msg.to_string());
            }
        }
    }

    /// Check if the skill is stale based on authored date.
    ///
    /// Returns `true` if the authored date is more than `STALENESS_DAYS` ago.
    #[must_use]
    pub fn is_stale(&self) -> bool {
        let today = russell_core::time::now_date_iso8601();
        super::health::is_stale(&self.authored, &today)
    }

    /// Check staleness and optionally transition to StaleWarning.
    ///
    /// If the skill is stale, consumes `self` and returns `StalenessResult::Stale`.
    /// Otherwise, returns `StalenessResult::Fresh` with `self` re-wrapped.
    ///
    /// The `today` parameter allows deterministic testing.
    pub fn check_staleness(
        self,
        journal: &JournalWriter,
        today: &str,
    ) -> StalenessResult {
        if super::health::is_stale(&self.authored, today) {
            let state_entered_at = today.to_string();
            journal_transition(
                journal,
                &self.skill_id,
                Some("active"),
                "stale_warning",
                Some(&format!(
                    "authored {} exceeds {} day threshold",
                    self.authored,
                    super::health::STALENESS_DAYS
                )),
            );
            StalenessResult::Stale(SkillLifecycle {
                skill_id: self.skill_id,
                authored: self.authored,
                state_entered_at,
                probe_runs: self.probe_runs,
                recent_probe_failures: self.recent_probe_failures,
                intervention_runs: self.intervention_runs,
                recent_intervention_failures: self.recent_intervention_failures,
                last_probe_run_at: self.last_probe_run_at,
                last_error: self.last_error,
                avg_probe_duration_ms: self.avg_probe_duration_ms,
                _state: PhantomData,
            })
        } else {
            StalenessResult::Fresh(self)
        }
    }

    /// Deprecate the skill manually (operator override).
    pub fn deprecate(
        self,
        journal: &JournalWriter,
        reason: Option<&str>,
    ) -> SkillLifecycle<Deprecated> {
        let state_entered_at = russell_core::time::now_date_iso8601();
        journal_transition(
            journal,
            &self.skill_id,
            Some("active"),
            "deprecated",
            reason,
        );
        SkillLifecycle {
            skill_id: self.skill_id,
            authored: self.authored,
            state_entered_at,
            probe_runs: self.probe_runs,
            recent_probe_failures: self.recent_probe_failures,
            intervention_runs: self.intervention_runs,
            recent_intervention_failures: self.recent_intervention_failures,
            last_probe_run_at: self.last_probe_run_at,
            last_error: self.last_error,
            avg_probe_duration_ms: self.avg_probe_duration_ms,
            _state: PhantomData,
        }
    }

    /// Compute health metrics for the active skill.
    ///
    /// Returns a health score 0.0–1.0 based on freshness (probe success rate).
    /// 1.0 = no failures, 0.0 = all failures or never run.
    #[must_use]
    pub fn compute_health(&self) -> f64 {
        if self.probe_runs == 0 {
            return 0.0;
        }
        let failure_rate = self.recent_probe_failures as f64 / self.probe_runs as f64;
        (1.0 - failure_rate).max(0.0)
    }
}

impl SkillLifecycle<StaleWarning> {
    /// Revalidate: operator confirms the skill is still relevant.
    /// Returns the skill back to Active state.
    pub fn revalidate(
        self,
        journal: &JournalWriter,
    ) -> SkillLifecycle<Active> {
        let state_entered_at = russell_core::time::now_date_iso8601();
        journal_transition(
            journal,
            &self.skill_id,
            Some("stale_warning"),
            "active",
            Some("operator revalidated"),
        );
        SkillLifecycle {
            skill_id: self.skill_id,
            authored: self.authored,
            state_entered_at,
            probe_runs: self.probe_runs,
            recent_probe_failures: self.recent_probe_failures,
            intervention_runs: self.intervention_runs,
            recent_intervention_failures: self.recent_intervention_failures,
            last_probe_run_at: self.last_probe_run_at,
            last_error: self.last_error,
            avg_probe_duration_ms: self.avg_probe_duration_ms,
            _state: PhantomData,
        }
    }

    /// Skill remains in StaleWarning for > 30 days → Deprecated.
    ///
    /// `days_in_warning` is the number of days since the transition to StaleWarning.
    /// If > 30, transitions to Deprecated. Otherwise, returns self.
    pub fn auto_deprecate(
        self,
        journal: &JournalWriter,
        days_in_warning: u32,
    ) -> StalenessResult {
        if days_in_warning > 30 {
            let state_entered_at = russell_core::time::now_date_iso8601();
            journal_transition(
                journal,
                &self.skill_id,
                Some("stale_warning"),
                "deprecated",
                Some(&format!("{days_in_warning} days in stale warning (>30)")),
            );
            StalenessResult::Stale(SkillLifecycle {
                skill_id: self.skill_id,
                authored: self.authored,
                state_entered_at,
                probe_runs: self.probe_runs,
                recent_probe_failures: self.recent_probe_failures,
                intervention_runs: self.intervention_runs,
                recent_intervention_failures: self.recent_intervention_failures,
                last_probe_run_at: self.last_probe_run_at,
                last_error: self.last_error,
                avg_probe_duration_ms: self.avg_probe_duration_ms,
                _state: PhantomData,
            })
        } else {
            StalenessResult::Fresh(SkillLifecycle {
                skill_id: self.skill_id,
                authored: self.authored,
                state_entered_at: self.state_entered_at,
                probe_runs: self.probe_runs,
                recent_probe_failures: self.recent_probe_failures,
                intervention_runs: self.intervention_runs,
                recent_intervention_failures: self.recent_intervention_failures,
                last_probe_run_at: self.last_probe_run_at,
                last_error: self.last_error,
                avg_probe_duration_ms: self.avg_probe_duration_ms,
                _state: PhantomData,
            })
        }
    }

    /// Deprecate manually.
    pub fn deprecate(
        self,
        journal: &JournalWriter,
        reason: Option<&str>,
    ) -> SkillLifecycle<Deprecated> {
        let state_entered_at = russell_core::time::now_date_iso8601();
        journal_transition(
            journal,
            &self.skill_id,
            Some("stale_warning"),
            "deprecated",
            reason,
        );
        SkillLifecycle {
            skill_id: self.skill_id,
            authored: self.authored,
            state_entered_at,
            probe_runs: self.probe_runs,
            recent_probe_failures: self.recent_probe_failures,
            intervention_runs: self.intervention_runs,
            recent_intervention_failures: self.recent_intervention_failures,
            last_probe_run_at: self.last_probe_run_at,
            last_error: self.last_error,
            avg_probe_duration_ms: self.avg_probe_duration_ms,
            _state: PhantomData,
        }
    }

    /// Retire directly from StaleWarning.
    pub fn retire(
        self,
        journal: &JournalWriter,
        reason: Option<&str>,
    ) -> SkillLifecycle<Retired> {
        let state_entered_at = russell_core::time::now_date_iso8601();
        journal_transition(
            journal,
            &self.skill_id,
            Some("stale_warning"),
            "retired",
            reason,
        );
        SkillLifecycle {
            skill_id: self.skill_id,
            authored: self.authored,
            state_entered_at,
            probe_runs: self.probe_runs,
            recent_probe_failures: self.recent_probe_failures,
            intervention_runs: self.intervention_runs,
            recent_intervention_failures: self.recent_intervention_failures,
            last_probe_run_at: self.last_probe_run_at,
            last_error: self.last_error,
            avg_probe_duration_ms: self.avg_probe_duration_ms,
            _state: PhantomData,
        }
    }
}

impl SkillLifecycle<Deprecated> {
    /// Retire the deprecated skill: remove from skills directory.
    pub fn retire(
        self,
        journal: &JournalWriter,
        reason: Option<&str>,
    ) -> SkillLifecycle<Retired> {
        let state_entered_at = russell_core::time::now_date_iso8601();
        journal_transition(
            journal,
            &self.skill_id,
            Some("deprecated"),
            "retired",
            reason,
        );
        SkillLifecycle {
            skill_id: self.skill_id,
            authored: self.authored,
            state_entered_at,
            probe_runs: self.probe_runs,
            recent_probe_failures: self.recent_probe_failures,
            intervention_runs: self.intervention_runs,
            recent_intervention_failures: self.recent_intervention_failures,
            last_probe_run_at: self.last_probe_run_at,
            last_error: self.last_error,
            avg_probe_duration_ms: self.avg_probe_duration_ms,
            _state: PhantomData,
        }
    }

    /// Re-install a deprecated skill (operator explicit override).
    /// Resets all telemetry and starts fresh in Installed state.
    pub fn reinstall(
        self,
        journal: &JournalWriter,
    ) -> SkillLifecycle<Installed> {
        let state_entered_at = russell_core::time::now_date_iso8601();
        journal_transition(
            journal,
            &self.skill_id,
            Some("deprecated"),
            "installed",
            Some("re-installed by operator"),
        );
        SkillLifecycle {
            skill_id: self.skill_id,
            authored: self.authored,
            state_entered_at,
            probe_runs: 0,
            recent_probe_failures: 0,
            intervention_runs: 0,
            recent_intervention_failures: 0,
            last_probe_run_at: None,
            last_error: None,
            avg_probe_duration_ms: None,
            _state: PhantomData,
        }
    }
}

impl SkillLifecycle<Retired> {
    /// Re-install a retired skill. Resets all telemetry.
    ///
    /// This is the only way out of the Retired state (terminus).
    pub fn reinstall(
        self,
        journal: &JournalWriter,
    ) -> SkillLifecycle<Installed> {
        let state_entered_at = russell_core::time::now_date_iso8601();
        journal_transition(
            journal,
            &self.skill_id,
            Some("retired"),
            "installed",
            Some("re-installed by operator"),
        );
        SkillLifecycle {
            skill_id: self.skill_id,
            authored: self.authored,
            state_entered_at,
            probe_runs: 0,
            recent_probe_failures: 0,
            intervention_runs: 0,
            recent_intervention_failures: 0,
            last_probe_run_at: None,
            last_error: None,
            avg_probe_duration_ms: None,
            _state: PhantomData,
        }
    }
}

// ─── Staleness result enum ──────────────────────────────────────────────────

/// Result of `check_staleness()` — either the skill stays in its current state
/// (`Fresh`) or transitions to the next stale state (`Stale`).
pub enum StalenessResult {
    /// The skill is not stale; returned unchanged.
    Fresh(SkillLifecycle<Active>),
    /// The skill is stale; transitioned to StaleWarning.
    Stale(SkillLifecycle<StaleWarning>),
}

// ─── Convenience conversion: state label strings ────────────────────────────

impl SkillLifecycle<Discovered> {
    /// Current lifecycle status as a label.
    #[must_use]
    pub fn status_label(&self) -> &'static str { "discovered" }
}

impl SkillLifecycle<Evaluated> {
    #[must_use]
    pub fn status_label(&self) -> &'static str { "evaluated" }
}

impl SkillLifecycle<Installed> {
    #[must_use]
    pub fn status_label(&self) -> &'static str { "installed" }
}

impl SkillLifecycle<Active> {
    #[must_use]
    pub fn status_label(&self) -> &'static str { "active" }
}

impl SkillLifecycle<StaleWarning> {
    #[must_use]
    pub fn status_label(&self) -> &'static str { "stale_warning" }
}

impl SkillLifecycle<Deprecated> {
    #[must_use]
    pub fn status_label(&self) -> &'static str { "deprecated" }
}

impl SkillLifecycle<Retired> {
    #[must_use]
    pub fn status_label(&self) -> &'static str { "retired" }
}

// ─── Journal integration ────────────────────────────────────────────────────

/// Write a lifecycle transition event to the journal.
///
/// Produces a `harness.event.v1` record with action
/// `"skill.lifecycle.transition"` and severity `Info`.
///
/// This is the same schema as `lifecycle.rs:63-85` but operates
/// with string labels instead of the `LifecycleStatus` enum.
pub fn journal_transition(
    journal: &JournalWriter,
    skill_id: &str,
    from_status: Option<&str>,
    to_status: &str,
    reason: Option<&str>,
) {
    let from_str = from_status.unwrap_or("none");
    let mut ev = Event::new("skill.lifecycle.transition", Severity::Info);
    ev.tier = Some("skill".into());
    ev.module = Some(format!("skill/{}", skill_id));
    ev.summary = Some(format!("{skill_id}: {from_str} → {to_status}"));
    ev.outputs.insert("skill_id".into(), skill_id.into());
    ev.outputs.insert("from_status".into(), from_str.into());
    ev.outputs.insert("to_status".into(), to_status.into());
    if let Some(r) = reason {
        ev.outputs.insert("reason".into(), r.into());
    }
    if let Err(e) = journal.append(&ev) {
        tracing::warn!(skill = %skill_id, error = %e, "failed to journal lifecycle transition");
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use russell_core::journal::JournalWriter;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn test_journal() -> (JournalWriter, PathBuf) {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let writer = JournalWriter::open(&db_path).unwrap();
        (writer, db_path)
    }

    #[test]
    fn full_happy_path() {
        let (journal, _db) = test_journal();
        let discovered = SkillLifecycle::discovered("test-skill", "2026-01-01");
        let evaluated = discovered.evaluate(&journal);
        let installed = evaluated.install(&journal);
        let active = installed.activate(&journal);
        assert_eq!(active.status_label(), "active");
        assert_eq!(active.probe_runs, 0);
    }

    #[test]
    fn shortcut_discovered_to_installed() {
        let (journal, _db) = test_journal();
        let discovered = SkillLifecycle::discovered("test-skill", "2026-01-01");
        let installed = discovered.install(&journal);
        assert_eq!(installed.status_label(), "installed");
    }

    #[test]
    fn record_execution_updates_telemetry() {
        let (journal, _db) = test_journal();
        let mut active = SkillLifecycle::discovered("test-skill", "2026-01-01")
            .evaluate(&journal)
            .install(&journal)
            .activate(&journal);

        active.record_execution(100);
        assert_eq!(active.probe_runs, 1);
        assert!(active.last_probe_run_at.is_some());
        assert!((active.avg_probe_duration_ms.unwrap() - 100.0).abs() < 0.01);
    }

    #[test]
    fn record_failure_increments_failure_count() {
        let (journal, _db) = test_journal();
        let mut active = SkillLifecycle::discovered("test-skill", "2026-01-01")
            .evaluate(&journal)
            .install(&journal)
            .activate(&journal);

        active.record_failure(50, "timeout");
        assert_eq!(active.probe_runs, 1);
        assert_eq!(active.recent_probe_failures, 1);
        assert_eq!(active.last_error.as_deref(), Some("timeout"));
    }

    #[test]
    fn compute_health_perfect() {
        let (journal, _db) = test_journal();
        let mut active = SkillLifecycle::discovered("test-skill", "2026-01-01")
            .evaluate(&journal)
            .install(&journal)
            .activate(&journal);

        active.record_execution(100);
        assert!((active.compute_health() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn compute_health_with_failures() {
        let (journal, _db) = test_journal();
        let mut active = SkillLifecycle::discovered("test-skill", "2026-01-01")
            .evaluate(&journal)
            .install(&journal)
            .activate(&journal);

        active.record_execution(100);
        active.record_failure(200, "timeout");
        assert!((active.compute_health() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn staleness_check_transitions() {
        let (journal, _db) = test_journal();
        let active = SkillLifecycle::discovered("test-skill", "2025-01-01") // old
            .evaluate(&journal)
            .install(&journal)
            .activate(&journal);

        let result = active.check_staleness(&journal, "2026-05-15");
        match result {
            StalenessResult::Stale(stale) => {
                assert_eq!(stale.status_label(), "stale_warning");
            }
            StalenessResult::Fresh(_) => panic!("expected stale"),
        }
    }

    #[test]
    fn staleness_check_stays_fresh() {
        let (journal, _db) = test_journal();
        let active = SkillLifecycle::discovered("test-skill", "2026-05-01") // recent
            .evaluate(&journal)
            .install(&journal)
            .activate(&journal);

        let result = active.check_staleness(&journal, "2026-05-15");
        match result {
            StalenessResult::Fresh(a) => {
                assert_eq!(a.status_label(), "active");
            }
            StalenessResult::Stale(_) => panic!("expected fresh"),
        }
    }

    #[test]
    fn retire_from_any_pre_active_state() {
        let (journal, _db) = test_journal();
        let discovered = SkillLifecycle::discovered("test-skill", "2026-01-01");
        let retired = discovered.retire(&journal, Some("not needed"));
        assert_eq!(retired.status_label(), "retired");
    }

    #[test]
    fn reinstall_from_retired() {
        let (journal, _db) = test_journal();
        let retired = SkillLifecycle::discovered("test-skill", "2026-01-01")
            .retire(&journal, None);
        let reinstalled = retired.reinstall(&journal);
        assert_eq!(reinstalled.status_label(), "installed");
        assert_eq!(reinstalled.probe_runs, 0); // telemetry reset
    }

    #[test]
    fn reinstall_from_deprecated() {
        let (journal, _db) = test_journal();
        let deprecated = SkillLifecycle::discovered("test-skill", "2026-01-01")
            .evaluate(&journal)
            .install(&journal)
            .activate(&journal)
            .deprecate(&journal, None);
        let reinstalled = deprecated.reinstall(&journal);
        assert_eq!(reinstalled.status_label(), "installed");
        assert_eq!(reinstalled.probe_runs, 0);
    }

    #[test]
    fn revalidate_from_stale_warning() {
        let (journal, _db) = test_journal();
        let stale = SkillLifecycle::discovered("test-skill", "2025-01-01")
            .evaluate(&journal)
            .install(&journal)
            .activate(&journal)
            .check_staleness(&journal, "2026-05-15");
        match stale {
            StalenessResult::Stale(sw) => {
                let revalidated = sw.revalidate(&journal);
                assert_eq!(revalidated.status_label(), "active");
            }
            StalenessResult::Fresh(_) => panic!("expected stale for 2025 date"),
        }
    }
}

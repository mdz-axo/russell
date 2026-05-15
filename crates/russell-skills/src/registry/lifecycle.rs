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

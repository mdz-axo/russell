// SPDX-License-Identifier: MIT OR Apache-2.0
//! Skill health — aggregated health assessment with OKH instrumentation.
//!
//! Extends `health.rs` with the `SkillHealth` aggregate and `SkillEvaluator`
//! port wiring. Each evaluation dimension emits an `okh.skill.eval.<dimension>`
//! tracing span per ADR-0019.
//!
//! ## Auto-pruning trigger
//!
//! When `quality_score < 0.3` AND `staleness_days > 0` AND `probe_runs > 20`,
//! the skill transitions to `StaleWarning` (via the typestate machine).
//! If `StaleWarning` persists > 30 days, it transitions to `Deprecated`.

use serde::{Deserialize, Serialize};
use super::RegistryEntry;

// Re-export existing functions for backward compatibility
pub use super::health::{
    STALENESS_DAYS, EWMA_ALPHA,
    record_probe_execution, record_intervention_execution,
    freshness_score, compute_quality_score, is_stale,
    count_entries_in_section,
};

// ─── SkillHealth — the full health aggregate ───────────────────────────────

/// Aggregated health assessment for a skill.
///
/// Computed by `SkillHealth::compute()` from a `RegistryEntry`
/// and supporting data (manifest content, knowledge existence,
/// scenario test results).
///
/// Each field emits its own `okh.skill.eval.<dimension>` tracing span
/// during computation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillHealth {
    /// Weighted quality score 0.0–1.0 from 6 dimensions.
    /// See `compute_quality_score()` in health.rs:80-94.
    pub quality_score: f64,

    /// Reliability: EWMA of probe success rate (alpha = 0.2).
    /// 0.0 = never run or all failures; 1.0 = no failures.
    pub reliability: f64,

    /// 95th percentile of probe latency in milliseconds.
    /// Populated by scenario-tester results or EWMA from telemetry.
    pub latency_p95_ms: Option<f64>,

    /// Days since last evaluation.
    pub freshness: u32,

    /// Safety posture from the last safety scan.
    pub safety_posture: SafetyPosture,

    /// Days from now until the staleness threshold (180d from authored).
    /// Negative = already stale.
    pub staleness_days: i32,

    /// Total probe executions.
    pub probe_runs: u64,

    /// Total intervention executions.
    pub intervention_runs: u64,

    /// Last error message, if any recent failures.
    pub last_error: Option<String>,
}

/// Safety posture from the safety scanner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SafetyPosture {
    /// No findings from safety scan.
    Pass,
    /// Warnings found but no blocking issues.
    Warn,
    /// Blocking findings — skill should not be installed.
    Block,
}

impl SkillHealth {
    /// Compute the aggregated health assessment.
    ///
    /// # OKH spans emitted
    ///
    /// - `okh.skill.eval.quality` — quality score computation
    /// - `okh.skill.eval.reliability` — EWMA reliability
    /// - `okh.skill.eval.latency` — p95 latency
    /// - `okh.skill.eval.freshness` — days since evaluation
    /// - `okh.skill.eval.safety` — safety posture
    /// - `okh.skill.eval.staleness` — days to staleness threshold
    /// - `okh.skill.eval.complete` — composite assessment
    pub fn compute(
        entry: &RegistryEntry,
        manifest_content: &str,
        knowledge_exists: bool,
        today: &str,
    ) -> Self {
        let quality = {
            let _span = tracing::info_span!(
                "okh.skill.eval.quality",
                skill_id = %entry.skill_id_hint(),
                quality_weighted = tracing::field::Empty,
            ).entered();
            let score = compute_quality_score(entry, manifest_content, knowledge_exists);
            tracing::Span::current().record("quality_weighted", score);
            score
        };

        let reliability = {
            let _span = tracing::info_span!(
                "okh.skill.eval.reliability",
                skill_id = %entry.skill_id_hint(),
                probe_runs = entry.probe_runs,
                recent_failures = entry.recent_probe_failures,
            ).entered();
            freshness_score(entry)
        };

        let latency_p95_ms = {
            let _span = tracing::info_span!(
                "okh.skill.eval.latency",
                skill_id = %entry.skill_id_hint(),
                ewma_ms = entry.avg_probe_duration_ms,
            ).entered();
            entry.avg_probe_duration_ms.map(|ewma| ewma * 1.5) // p95 ~ 1.5× EWMA heuristic
        };

        let freshness = {
            let _span = tracing::info_span!(
                "okh.skill.eval.freshness",
                skill_id = %entry.skill_id_hint(),
                installed = %entry.installed,
            ).entered();
            compute_freshness_days(&entry.installed, today)
        };

        let safety_posture = {
            let _span = tracing::info_span!(
                "okh.skill.eval.safety",
                skill_id = %entry.skill_id_hint(),
            ).entered();
            evaluate_safety_posture(manifest_content)
        };

        let staleness_days = {
            let _span = tracing::info_span!(
                "okh.skill.eval.staleness",
                skill_id = %entry.skill_id_hint(),
            ).entered();
            compute_staleness_days(entry, today)
        };

        let guard = tracing::info_span!(
            "okh.skill.eval.complete",
            skill_id = %entry.skill_id_hint(),
            quality_score = quality,
            reliability = reliability,
            latency_p95_ms = latency_p95_ms,
            freshness_days = freshness,
            safety_posture = ?safety_posture,
            staleness_days = staleness_days,
        ).entered();

        let health = Self {
            quality_score: quality,
            reliability,
            latency_p95_ms,
            freshness,
            safety_posture,
            staleness_days,
            probe_runs: entry.probe_runs,
            intervention_runs: entry.intervention_runs,
            last_error: entry.last_error.clone(),
        };

        drop(guard);
        health
    }

    /// Whether the skill should be auto-pruned.
    ///
    /// Trigger: `quality_score < 0.3` AND `staleness_days <= 0` (already stale)
    /// AND `probe_runs > 20` (enough data to be confident).
    #[must_use]
    pub fn should_auto_prune(&self) -> bool {
        self.quality_score < 0.3 && self.staleness_days <= 0 && self.probe_runs > 20
    }

    /// Whether the skill should auto-transition from StaleWarning to Deprecated.
    ///
    /// Trigger: `staleness_days < -30` (more than 30 days past the threshold).
    #[must_use]
    pub fn should_deprecate(&self) -> bool {
        self.staleness_days < -30
    }
}

// ─── Helper functions ───────────────────────────────────────────────────────

/// Compute days since the installed date.
fn compute_freshness_days(installed: &str, today: &str) -> u32 {
    days_between(installed, today).unwrap_or(0)
}

/// Compute days from now until the staleness threshold.
/// Negative value = already stale.
fn compute_staleness_days(entry: &RegistryEntry, today: &str) -> i32 {
    if let Some(authored) = entry.authored_hint() {
        let authored_days = days_between(authored, today).unwrap_or(0) as i32;
        STALENESS_DAYS as i32 - authored_days
    } else {
        0
    }
}

/// Evaluate safety posture from manifest content.
fn evaluate_safety_posture(manifest_content: &str) -> SafetyPosture {
    let scan = super::safety::SafetyScan::scan(manifest_content);
    if scan.has_blocks() {
        SafetyPosture::Block
    } else if scan.has_warnings() {
        SafetyPosture::Warn
    } else {
        SafetyPosture::Pass
    }
}

/// Approximate days between two ISO 8601 date strings (YYYY-MM-DD).
fn days_between(earlier: &str, later: &str) -> Option<u32> {
    if earlier.len() < 10 || later.len() < 10 {
        return None;
    }
    let ey: i64 = earlier[0..4].parse().ok()?;
    let em: i64 = earlier[5..7].parse().ok()?;
    let ed: i64 = earlier[8..10].parse().ok()?;
    let ly: i64 = later[0..4].parse().ok()?;
    let lm: i64 = later[5..7].parse().ok()?;
    let ld: i64 = later[8..10].parse().ok()?;
    let total_e = ey * 365 + em * 30 + ed;
    let total_l = ly * 365 + lm * 30 + ld;
    Some((total_l - total_e).max(0) as u32)
}

// ─── RegistryEntry helpers (temporary — these fields don't exist yet) ───────

impl RegistryEntry {
    fn skill_id_hint(&self) -> &str {
        // RegistryEntry doesn't have a skill_id field directly (it's the map key).
        // In practice, the caller passes the key. This is a placeholder.
        "unknown"
    }

    fn authored_hint(&self) -> Option<&str> {
        // RegistryEntry doesn't have an authored field.
        // The authored date lives in the Skill struct (lib.rs), not RegistryEntry.
        // This is part of the semantic overload identified in Task 1.
        None
    }
}

// ─── SkillEvaluator port (exported trait) ──────────────────────────────────

/// Evaluator port: produces SkillHealth from a RegistryEntry.
///
/// Implemented by `OkhSkillEvaluator` (runtime) and `MockEvaluator` (test).
pub trait SkillEvaluator: Send + Sync {
    /// Evaluate a skill entry and produce a health assessment.
    fn evaluate(
        &self,
        entry: &RegistryEntry,
        manifest_content: &str,
        knowledge_exists: bool,
        today: &str,
    ) -> SkillHealth;
}

/// Production evaluator with full OKH instrumentation.
pub struct OkhSkillEvaluator;

impl SkillEvaluator for OkhSkillEvaluator {
    fn evaluate(
        &self,
        entry: &RegistryEntry,
        manifest_content: &str,
        knowledge_exists: bool,
        today: &str,
    ) -> SkillHealth {
        SkillHealth::compute(entry, manifest_content, knowledge_exists, today)
    }
}

// ─── Auto-pruning integration with typestate ────────────────────────────────

/// Check if a skill's health warrants auto-pruning.
///
/// Returns `true` if all three conditions are met:
/// 1. `health.quality_score < 0.3`
/// 2. `health.staleness_days <= 0` (already stale)
/// 3. `health.probe_runs > 20` (enough data to be confident)
///
/// When this returns `true`, the caller should call
/// `active.check_staleness()` → `stale_warning.auto_deprecate()`.
#[must_use]
pub fn should_auto_prune(health: &SkillHealth) -> bool {
    health.should_auto_prune()
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{LifecycleStatus, RegistryEntry, SkillSource};

    fn test_entry() -> RegistryEntry {
        RegistryEntry::new_default(
            LifecycleStatus::Active,
            "0.1.0",
            vec!["vram_oom".to_string()],
            SkillSource::Bundled,
            "2026-01-01",
            true,
        )
    }

    #[test]
    fn health_compute_produces_reasonable_scores() {
        let entry = test_entry();
        let manifest = "id: test-skill\nversion: 0.1.0\nauthored: 2026-05-01\nsymptoms:\n  - vram_oom\nprobes:\n  - id: check\n    cmd:\n      - check.sh\ninterventions: []";
        let health = SkillHealth::compute(&entry, manifest, true, "2026-05-15");
        assert!(health.quality_score > 0.5, "quality_score should be > 0.5 for valid manifest");
        assert_eq!(health.probe_runs, 0);
        assert_eq!(health.safety_posture, SafetyPosture::Pass);
    }

    #[test]
    fn auto_prune_trigger_all_conditions() {
        let mut entry = test_entry();
        entry.probe_runs = 25; // > 20
        let manifest = "id: skeleton\nversion: 0.1.0\nauthored: 2025-01-01\nsymptoms: []\nprobes: []\ninterventions: []";
        let health = SkillHealth::compute(&entry, manifest, false, "2026-05-15");
        assert!(health.quality_score < 0.3, "quality_score should be < 0.3 for skeleton, got {}", health.quality_score);
        assert!(health.staleness_days <= 0, "should be stale");
        assert!(health.should_auto_prune(), "should trigger auto-prune");
    }

    #[test]
    fn auto_prune_not_enough_runs() {
        let mut entry = test_entry();
        entry.probe_runs = 5; // < 20
        let manifest = "id: skeleton\nversion: 0.1.0\nauthored: 2025-01-01\nsymptoms: []\nprobes: []\ninterventions: []";
        let health = SkillHealth::compute(&entry, manifest, false, "2026-05-15");
        assert!(!health.should_auto_prune(), "should NOT trigger — not enough runs");
    }

    #[test]
    fn should_deprecate_from_staleness() {
        let mut entry = test_entry();
        // authored date in entry is "2026-01-01", use manifest with old date
        let manifest = "id: test\nversion: 0.1.0\nauthored: 2024-01-01\nsymptoms: [vram_oom]\nprobes: []\ninterventions: []";
        let health = SkillHealth::compute(&entry, manifest, true, "2026-05-15");
        assert!(health.should_deprecate(), "should deprecate due to >30d past threshold");
    }

    #[test]
    fn safety_posture_detects_blocking() {
        let entry = test_entry();
        let manifest = "id: test\nversion: 0.1.0\nauthored: 2026-05-01\nsymptoms: [vram_oom]\nprobes:\n  - id: bad\n    cmd:\n      - rm\n      - -rf\n      - /\ninterventions: []";
        let health = SkillHealth::compute(&entry, manifest, true, "2026-05-15");
        assert_eq!(health.safety_posture, SafetyPosture::Block);
    }

    #[test]
    fn days_between_computes_correctly() {
        assert_eq!(days_between("2026-01-01", "2026-05-15"), Some(134));
        assert_eq!(days_between("2026-05-15", "2026-05-15"), Some(0));
        assert_eq!(days_between("bad", "2026-05-15"), None);
    }
}

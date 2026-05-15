// SPDX-License-Identifier: MIT OR Apache-2.0
//! Skill health — telemetry recording, EWMA, quality scoring,
//! and staleness detection.
//!
//! This module encapsulates the measurement dimension of skills:
//! how well they run, how often they fail, and how fresh they are.

use super::RegistryEntry;

/// EWMA smoothing factor (alpha = 0.2 ≈ 5-sample effective window).
const EWMA_ALPHA: f64 = 0.2;

/// Staleness threshold in days.
pub const STALENESS_DAYS: i64 = 180;

/// Record a probe execution in a registry entry.
///
/// Increments `probe_runs` and, if failed, `recent_probe_failures`.
/// Updates `last_probe_run_at`, `last_error`, and `avg_probe_duration_ms` (EWMA).
pub fn record_probe_execution(
    entry: &mut RegistryEntry,
    success: bool,
    duration_ms: u64,
    error_message: Option<&str>,
    now_iso: &str,
) {
    entry.probe_runs = entry.probe_runs.saturating_add(1);
    entry.last_probe_run_at = Some(now_iso.to_string());
    if !success {
        entry.recent_probe_failures = entry.recent_probe_failures.saturating_add(1);
        if let Some(msg) = error_message {
            entry.last_error = Some(msg.to_string());
        }
    }
    // EWMA update.
    let current = duration_ms as f64;
    entry.avg_probe_duration_ms = Some(match entry.avg_probe_duration_ms {
        Some(prev) => EWMA_ALPHA * current + (1.0 - EWMA_ALPHA) * prev,
        None => current,
    });
}

/// Record an intervention execution in a registry entry.
pub fn record_intervention_execution(
    entry: &mut RegistryEntry,
    success: bool,
    error_message: Option<&str>,
) {
    entry.intervention_runs = entry.intervention_runs.saturating_add(1);
    if !success {
        entry.recent_intervention_failures = entry.recent_intervention_failures.saturating_add(1);
        if let Some(msg) = error_message {
            entry.last_error = Some(msg.to_string());
        }
    }
}

/// Freshness score: how recently and reliably the skill has run.
///
/// Returns 0.0 if the skill has never run, up to 1.0 for no failures.
#[must_use]
pub fn freshness_score(entry: &RegistryEntry) -> f64 {
    if entry.probe_runs == 0 {
        return 0.0;
    }
    let failure_rate = entry.recent_probe_failures as f64 / entry.probe_runs as f64;
    (1.0 - failure_rate).max(0.0)
}

/// Compute a quality score 0.0–1.0 for a skill entry.
///
/// Weights:
/// - Manifest completeness: 0.20
/// - Probe coverage: 0.25
/// - Intervention coverage: 0.20
/// - Rollback quality: 0.15
/// - Script quality: 0.10
/// - Documentation: 0.10
#[must_use]
pub fn compute_quality_score(
    entry: &RegistryEntry,
    manifest_content: &str,
    knowledge_exists: bool,
) -> f64 {
    let weights: [(f64, f64); 6] = [
        (0.20, score_manifest(manifest_content)),
        (0.25, score_probe_coverage(manifest_content)),
        (0.20, score_intervention_coverage(manifest_content)),
        (0.15, score_rollback_quality(manifest_content)),
        (0.10, score_script_quality(manifest_content)),
        (0.10, score_documentation(entry, knowledge_exists)),
    ];
    weights.iter().map(|(w, s)| w * s).sum()
}

/// Check if a skill's authored date makes it stale (> 180 days).
#[must_use]
pub fn is_stale(authored_date: &str, today: &str) -> bool {
    if authored_date.len() < 10 || today.len() < 10 {
        return false;
    }
    authored_date < staleness_threshold(today).as_str()
}

// ─── Internal scoring functions ───────────────────────────────────────────

fn score_manifest(content: &str) -> f64 {
    let required = ["id:", "version:", "authored:", "symptoms:"];
    let present = required
        .iter()
        .filter(|k| content.lines().any(|l| l.starts_with(*k)))
        .count();
    present as f64 / required.len() as f64
}

fn score_probe_coverage(content: &str) -> f64 {
    let count = count_entries_in_section(content, "probes:", "- id:");
    if count == 0 {
        return 0.0;
    }
    1.0
}

fn score_intervention_coverage(content: &str) -> f64 {
    let count = count_entries_in_section(content, "interventions:", "- id:");
    if count == 0 {
        return 0.5;
    }
    1.0
}

fn score_rollback_quality(content: &str) -> f64 {
    let sections = section_lines(content, "interventions:");
    if sections.is_empty() {
        return 0.3;
    }
    if sections
        .iter()
        .any(|l| l.contains("none_needed") || l.contains("reboot"))
    {
        return 0.8;
    }
    if sections.iter().any(|l| l.starts_with("    rollback:")) {
        return 1.0;
    }
    0.3
}

fn score_script_quality(content: &str) -> f64 {
    let lower = content.to_lowercase();
    let has_shebang = content.lines().any(|l| l.starts_with("#!/"));
    let has_set_e = lower.contains("set -e")
        || lower.contains("set -eu")
        || lower.contains("set -euo pipefail");
    let probe_cmds = count_entries_in_section(content, "probes:", "- cmd:");
    let intervention_cmds = count_entries_in_section(content, "interventions:", "- cmd:");
    let has_cmd = probe_cmds > 0 || intervention_cmds > 0;
    let checks = [has_shebang, has_set_e, has_cmd];
    checks.iter().filter(|&&c| c).count() as f64 / checks.len() as f64
}

fn score_documentation(entry: &RegistryEntry, knowledge_exists: bool) -> f64 {
    let mut score = 0.0;
    if knowledge_exists {
        score += 0.6;
    }
    if !entry.symptoms.is_empty() {
        score += 0.4;
    }
    score
}

/// Count entries matching `entry_pattern` within a named top-level YAML section.
pub(crate) fn count_entries_in_section(
    content: &str,
    section_header: &str,
    entry_pattern: &str,
) -> usize {
    let mut count = 0;
    let mut in_section = false;
    for line in content.lines() {
        if line.trim_start() == section_header {
            in_section = true;
            continue;
        }
        if in_section {
            if line.is_empty() {
                continue;
            }
            if !line.starts_with(' ') && !line.starts_with('\t') {
                break;
            }
            if line.trim_start().starts_with(entry_pattern) {
                count += 1;
            }
        }
    }
    count
}

/// Return lines belonging to a named top-level YAML section.
fn section_lines<'a>(content: &'a str, section_header: &str) -> Vec<&'a str> {
    let mut result = Vec::new();
    let mut in_section = false;
    for line in content.lines() {
        if line.trim_start() == section_header {
            in_section = true;
            continue;
        }
        if in_section {
            if line.is_empty() {
                continue;
            }
            if !line.starts_with(' ') && !line.starts_with('\t') {
                break;
            }
            result.push(line);
        }
    }
    result
}

/// Compute the staleness threshold (today minus 180 days) as an ISO 8601 string.
fn staleness_threshold(today: &str) -> String {
    let parts: Vec<&str> = today.split('-').collect();
    if parts.len() != 3 {
        return today.to_string();
    }
    let year: i64 = parts[0].parse().unwrap_or(0);
    let month: i64 = parts[1].parse().unwrap_or(0);
    let day: i64 = parts[2].parse().unwrap_or(0);

    // Convert to days-since-epoch, subtract 180, convert back.
    let total = year * 365 + month * 30 + day - STALENESS_DAYS;
    let ty = total / 365;
    let rem = total % 365;
    let tm = (rem / 30).clamp(1, 12);
    let td = (rem % 30).clamp(1, 28);
    format!("{ty:04}-{tm:02}-{td:02}")
}

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
            "2026-05-01",
            true,
        )
    }

    #[test]
    fn freshness_zero_for_no_runs() {
        let entry = test_entry();
        assert!((freshness_score(&entry)).abs() < f64::EPSILON);
    }

    #[test]
    fn freshness_perfect() {
        let mut entry = test_entry();
        entry.probe_runs = 100;
        entry.recent_probe_failures = 0;
        assert!((freshness_score(&entry) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn freshness_with_failures() {
        let mut entry = test_entry();
        entry.probe_runs = 100;
        entry.recent_probe_failures = 30;
        assert!((freshness_score(&entry) - 0.7).abs() < f64::EPSILON);
    }

    #[test]
    fn staleness_detection() {
        assert!(is_stale("2025-11-01", "2026-05-13"));
        assert!(!is_stale("2026-05-01", "2026-05-13"));
    }

    #[test]
    fn quality_score_full_skill() {
        let entry = test_entry();
        let manifest = "id: test-skill\nversion: 0.1.0\nauthored: 2026-05-01\nsymptoms:\n  - vram_oom\nprobes:\n  - id: probe-vram\n    cmd:\n      - check-vram.sh\ninterventions:\n  - id: restart\n    cmd:\n      - restart.sh\n    rollback: none_needed";
        let score = compute_quality_score(&entry, manifest, true);
        assert!(score > 0.7, "expected >0.7 got {score}");
    }

    #[test]
    fn quality_score_skeleton() {
        let entry = RegistryEntry::new_default(
            LifecycleStatus::Discovered,
            "0.1.0",
            vec![],
            SkillSource::Workshop,
            "2026-05-01",
            false,
        );
        let manifest = "id: skeleton\nversion: 0.1.0\nauthored: 2026-05-01\nsymptoms: []\nprobes: []\ninterventions: []";
        let score = compute_quality_score(&entry, manifest, false);
        assert!(score < 0.5, "expected <0.5 got {score}");
    }

    #[test]
    fn record_probe_updates_ewma() {
        let mut entry = test_entry();
        entry.probe_runs = 1;
        entry.avg_probe_duration_ms = Some(100.0);
        record_probe_execution(&mut entry, true, 200, None, "2026-05-15");
        let ewma = entry.avg_probe_duration_ms.unwrap();
        assert!(
            (ewma - 120.0).abs() < 0.01,
            "EWMA should be ~120, got {ewma}"
        );
    }
}

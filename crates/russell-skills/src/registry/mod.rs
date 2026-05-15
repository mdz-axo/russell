// SPDX-License-Identifier: MIT OR Apache-2.0
//! Skill registry — local cache, lifecycle tracking, and lookup.
//!
//! The registry is a derived YAML cache (rebuildable from manifests + journal).
//! It tracks lifecycle state, coverage, staleness, and provides
//! symptom → skill lookup for Jack's decision support.
//!
//! Persisted at `~/.local/share/harness/registry/local-cache.yaml`.
//! See ADR-0024.
//!
//! ## Module structure
//!
//! The registry is split into focused submodules:
//! - [`lifecycle`] — state machine, transitions, journal events
//! - [`safety`] — content scanning and installation gates
//! - [`health`] — telemetry (EWMA), quality scoring, staleness detection

pub mod health;
pub mod lifecycle;
pub mod safety;

// Re-export submodule types at registry level for backward compatibility.
pub use health::{compute_quality_score, freshness_score, is_stale};
pub use lifecycle::{journal_transition, LifecycleStatus};
pub use safety::{SafetyScan, ScanFinding, ScanSeverity};

use russell_core::journal::JournalWriter;
use russell_core::time::now_date_iso8601;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

// Re-export the `RegistryError` type so existing callers still find it here.
// (It remains defined in this file.)

/// The local registry cache — one entry per known skill.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RegistryCache {
    /// Map of skill_id → cache entry.
    pub skills: BTreeMap<String, RegistryEntry>,
}

/// One skill's entry in the cache.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryEntry {
    /// Lifecycle state.
    pub status: LifecycleStatus,
    /// Semver version from manifest.
    pub version: String,
    /// Date the skill was authored (ISO 8601).
    pub authored: String,
    /// Symptoms this skill addresses (from manifest).
    pub symptoms: Vec<String>,
    /// Where the skill came from.
    pub source: SkillSource,
    /// Gap 3: Execution trust tier — determines whether this skill
    /// needs consent gates, sandbox testing, or can auto-execute.
    /// Independent of `source` (a `Workshop` skill starts at T3, not T4).
    #[serde(default = "default_trust_tier")]
    pub trust_tier: TrustTier,
    /// ISO 8601 date of installation.
    pub installed: String,
    /// ISO 8601 date of last evaluation.
    #[serde(default)]
    pub last_evaluated: Option<String>,
    /// ISO 8601 date the skill validity expires, if set.
    #[serde(default)]
    pub valid_until: Option<String>,
    /// Quality score 0.0–1.0.
    #[serde(default)]
    pub coverage_score: Option<f64>,
    /// If deprecated: which skill superseded it.
    #[serde(default)]
    pub superseded_by: Option<String>,
    /// Operator's deprecation note.
    #[serde(default)]
    pub deprecation_reason: Option<String>,
    /// Number of times probes from this skill have run.
    #[serde(default)]
    pub probe_runs: u64,
    /// Number of recent probe failures.
    #[serde(default)]
    pub recent_probe_failures: u64,
    /// Number of times interventions from this skill have run.
    #[serde(default)]
    pub intervention_runs: u64,
    /// Number of recent intervention failures.
    #[serde(default)]
    pub recent_intervention_failures: u64,
    /// ISO 8601 timestamp of most recent probe run.
    #[serde(default)]
    pub last_probe_run_at: Option<String>,
    /// Error message from the most recent failure, if any.
    #[serde(default)]
    pub last_error: Option<String>,
    /// EWMA of probe run durations in milliseconds.
    #[serde(default)]
    pub avg_probe_duration_ms: Option<f64>,
    /// EWMA success rate (0.0–1.0). Updated on each probe/intervention
    /// execution. More recent outcomes weigh more heavily than historical.
    /// `None` means no executions have been recorded yet.
    #[serde(default)]
    pub ewma_success_rate: Option<f64>,
    /// Whether this is a bundled skill (resistant to pruning).
    #[serde(default)]
    pub bundled: bool,
}

impl RegistryEntry {
    /// Create a new entry with the given key fields and sensible defaults
    /// for all telemetry / lifecycle metadata.
    ///
    /// This eliminates the repeated 18-field struct literals scattered
    /// across workshop commands and skill sync code.
    #[must_use]
    pub fn new_default(
        status: LifecycleStatus,
        version: impl Into<String>,
        authored: impl Into<String>,
        symptoms: Vec<String>,
        source: SkillSource,
        installed: impl Into<String>,
        bundled: bool,
    ) -> Self {
        let trust_tier = initial_trust_tier(&source);
        Self {
            status,
            version: version.into(),
            authored: authored.into(),
            symptoms,
            source,
            trust_tier,
            installed: installed.into(),
            last_evaluated: None,
            valid_until: None,
            coverage_score: None,
            superseded_by: None,
            deprecation_reason: None,
            probe_runs: 0,
            recent_probe_failures: 0,
            intervention_runs: 0,
            recent_intervention_failures: 0,
            last_probe_run_at: None,
            last_error: None,
            avg_probe_duration_ms: None,
            ewma_success_rate: None,
            bundled,
        }
    }
}

// LifecycleStatus is now defined in lifecycle.rs and re-exported above.

/// Where the skill was sourced from.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillSource {
    /// Shipped with Russell.
    Bundled,
    /// Installed from a remote registry.
    Registry {
        /// Registry name (e.g., "russell-official").
        registry: String,
        /// The slug used to fetch it.
        slug: String,
    },
    /// Created locally via workshop.
    Workshop,
    /// Manually copied by operator.
    Manual,
    /// Downloaded from a URL.
    Remote {
        /// Source URL.
        url: String,
    },
}

/// A remote registry source configured by the operator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrySource {
    /// Display name.
    pub name: String,
    /// Base URL or git clone URL.
    pub url: String,
    /// Kind of source.
    pub kind: RegistryKind,
}

/// Kinds of remote registry sources.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RegistryKind {
    /// A GitHub repository with skills/ directory.
    GithubRepo,
    /// A plain URL serving a directory listing.
    HttpIndex,
    /// A local filesystem path.
    LocalDir,
}

/// Gap 3: Execution trust tier — determines the gates required before
/// a skill's probes and interventions can execute.
///
/// Trust tiers are mapped from the skill governance framework
/// (`pragmatic-cybernetics/references/skill-governance.md`):
///
/// | Tier | Access | Gates Required |
/// |------|--------|---------------|
/// | T1 (Metadata) | Name, description only | None — informational only |
/// | T2 (Instructions) | Read skill instructions | G1: Provenance verified |
/// | T3 (Supervised) | Execute with consent | G2: Safety scanned + G3: Sandbox tested |
/// | T4 (Autonomous) | Full auto-execution | G4: Continuous monitoring active |
///
/// Initial trust tier is derived from `SkillSource`:
/// - `Bundled` → T4 (trusted by provenance)
/// - `Workshop` → T3 (operator-created, not yet verified)
/// - `Registry` → T2 (requires G2/G3 gates before execution)
/// - `Remote { url }` → T1 (metadata only, full ascent required)
/// - `Manual` → T2 (operator copied, no provenance chain)
///
/// Trust follows **Slovic asymmetry**: a single anomalous probe result
/// from a T4 skill demotes it to T3 immediately. Re-escalation requires
/// sustained evidence through multiple gates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustTier {
    /// Metadata only — name, description, symptoms. No execution.
    T1,
    /// Instructions readable — KNOWLEDGE.md injectable. No execution.
    T2,
    /// Supervised execution — requires operator consent for every action.
    T3,
    /// Autonomous execution — probes auto-execute, interventions auto-approve.
    T4,
}

/// Derive the initial trust tier from a skill's source.
///
/// Conservative defaults: even `Bundled` skills start at T4 because
/// they are shipped with Russell and are provenance-verified.
/// All external sources start at T1–T3 requiring gate escalation.
#[must_use]
pub fn initial_trust_tier(source: &SkillSource) -> TrustTier {
    match source {
        SkillSource::Bundled => TrustTier::T4,
        SkillSource::Workshop => TrustTier::T3,
        SkillSource::Registry { .. } => TrustTier::T2,
        SkillSource::Remote { .. } => TrustTier::T1,
        SkillSource::Manual => TrustTier::T2,
    }
}

fn default_trust_tier() -> TrustTier {
    TrustTier::T3
}

/// Configuration for remote registry sources.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RegistrySources {
    /// Configured remote sources.
    pub sources: Vec<RegistrySource>,
}

impl RegistryCache {
    /// Create an empty cache.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Load from a YAML file. Returns empty cache if file doesn't exist.
    ///
    /// # Errors
    /// Returns an error if the file exists but can't be parsed.
    pub fn load(path: &Path) -> Result<Self, RegistryError> {
        if !path.exists() {
            return Ok(Self::new());
        }
        let yaml = std::fs::read_to_string(path).map_err(|e| RegistryError::Read {
            path: path.to_path_buf(),
            source: e,
        })?;
        serde_yaml::from_str(&yaml).map_err(|e| RegistryError::Parse {
            path: path.to_path_buf(),
            message: e.to_string(),
        })
    }

    /// Save to a YAML file.
    ///
    /// # Errors
    /// Returns an error if the file can't be written.
    pub fn save(&self, path: &Path) -> Result<(), RegistryError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| RegistryError::Write {
                path: parent.to_path_buf(),
                source: e,
            })?;
        }
        let yaml = serde_yaml::to_string(self).map_err(|e| RegistryError::Serialize {
            message: e.to_string(),
        })?;
        std::fs::write(path, yaml).map_err(|e| RegistryError::Write {
            path: path.to_path_buf(),
            source: e,
        })?;
        Ok(())
    }

    /// Look up which installed skills address a given symptom.
    #[must_use]
    pub fn lookup_symptom(&self, symptom: &str) -> Vec<&RegistryEntry> {
        self.skills
            .values()
            .filter(|e| e.status.is_loadable() && e.symptoms.iter().any(|s| s == symptom))
            .collect()
    }

    /// Find all catalogued symptoms that have no installed skill.
    #[must_use]
    pub fn coverage_gaps(&self, all_symptoms: &[&str]) -> Vec<String> {
        let covered: std::collections::BTreeSet<&str> = self
            .skills
            .values()
            .filter(|e| e.status.is_loadable())
            .flat_map(|e| e.symptoms.iter().map(String::as_str))
            .collect();
        all_symptoms
            .iter()
            .filter(|s| !covered.contains(*s))
            .map(|s| (*s).to_string())
            .collect()
    }

    /// Get all skills in a given lifecycle status.
    #[must_use]
    pub fn by_status(&self, status: LifecycleStatus) -> Vec<(&String, &RegistryEntry)> {
        self.skills
            .iter()
            .filter(|(_, e)| e.status == status)
            .collect()
    }

    /// Upsert a skill entry.
    pub fn upsert(&mut self, skill_id: &str, entry: RegistryEntry) {
        self.skills.insert(skill_id.to_string(), entry);
    }

    /// Record a probe execution in the registry cache.
    ///
    /// Delegates to [`health::record_probe_execution`].
    pub fn record_execution(
        &mut self,
        skill_id: &str,
        success: bool,
        duration_ms: u64,
        error_message: Option<&str>,
    ) {
        if let Some(entry) = self.skills.get_mut(skill_id) {
            let now = now_date_iso8601();
            health::record_probe_execution(entry, success, duration_ms, error_message, &now);
        }
    }

    /// Record an intervention execution.
    ///
    /// Delegates to [`health::record_intervention_execution`].
    pub fn record_intervention(&mut self, skill_id: &str, success: bool, error_message: Option<&str>) {
        if let Some(entry) = self.skills.get_mut(skill_id) {
            health::record_intervention_execution(entry, success, error_message);
        }
    }

    /// Remove a skill entry from the cache entirely (for retired skills).
    pub fn remove_entry(&mut self, skill_id: &str) -> Option<RegistryEntry> {
        self.skills.remove(skill_id)
    }

    /// Load, mutate, save — safe concurrent update pattern.
    /// The closure receives `&mut self` so any mutation is committed
    /// atomically. Concurrent saves are best-effort (last writer wins),
    /// acceptable since the registry is rebuildable per JR-7.
    ///
    /// # Errors
    /// Returns a [`RegistryError`] if the cache cannot be loaded or saved.
    pub fn with_update(
        path: &Path,
        f: impl FnOnce(&mut Self),
    ) -> Result<(), RegistryError> {
        let mut cache = Self::load(path)?;
        f(&mut cache);
        cache.save(path)
    }

    /// Write a lifecycle transition event to the journal.
    ///
    /// Delegates to [`lifecycle::journal_transition`].
    pub fn journal_transition(
        journal: &JournalWriter,
        skill_id: &str,
        from_status: Option<LifecycleStatus>,
        to_status: LifecycleStatus,
        reason: Option<&str>,
    ) {
        lifecycle::journal_transition(journal, skill_id, from_status, to_status, reason);
    }

    /// Compute a quality score 0.0–1.0 for a skill entry.
    ///
    /// Delegates to [`health::compute_quality_score`].
    #[must_use]
    pub fn compute_score(
        entry: &RegistryEntry,
        manifest_content: &str,
        knowledge_exists: bool,
    ) -> f64 {
        health::compute_quality_score(entry, manifest_content, knowledge_exists)
    }

    /// Freshness score: how recently and reliably the skill has run.
    ///
    /// Delegates to [`health::freshness_score`].
    #[must_use]
    pub fn freshness_score(entry: &RegistryEntry) -> f64 {
        health::freshness_score(entry)
    }

    /// Check if a skill's authored date makes it stale (> 180 days).
    ///
    /// Delegates to [`health::is_stale`].
    #[must_use]
    pub fn is_stale(authored_date: &str, today: &str) -> bool {
        health::is_stale(authored_date, today)
    }
}

// Scoring and staleness functions are now in health.rs.
// The standalone `count_entries_in_section` and `staleness_threshold`
// have been moved to `health.rs` and are used via the delegating methods above.

/// Errors from registry operations.
#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    /// I/O error reading the cache file.
    #[error("cannot read registry cache {path}: {source}")]
    Read {
        /// File path.
        path: std::path::PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// Parse error.
    #[error("cannot parse registry cache {path}: {message}")]
    Parse {
        /// File path.
        path: std::path::PathBuf,
        /// Error message.
        message: String,
    },

    /// Serialization error.
    #[error("cannot serialize registry cache: {message}")]
    Serialize {
        /// Error message.
        message: String,
    },

    /// I/O error writing the cache file.
    #[error("cannot write registry cache {path}: {source}")]
    Write {
        /// File path.
        path: std::path::PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
}

// Safety scanner types and functions are now in safety.rs.
// They are re-exported at the top of this module for backward compatibility.

#[cfg(test)]
mod tests {
    use super::*;

    fn test_entry(status: LifecycleStatus, version: &str, symptoms: Vec<&str>, source: SkillSource) -> RegistryEntry {
        let bundled = matches!(source, SkillSource::Bundled);
        let trust_tier = initial_trust_tier(&source);
        RegistryEntry {
            status,
            version: version.into(),
            symptoms: symptoms.into_iter().map(String::from).collect(),
            source,
            trust_tier,
            installed: "2026-05-01".into(),
            last_evaluated: None,
            valid_until: None,
            coverage_score: None,
            superseded_by: None,
            deprecation_reason: None,
            probe_runs: 0,
            recent_probe_failures: 0,
            intervention_runs: 0,
            recent_intervention_failures: 0,
            last_probe_run_at: None,
            last_error: None,
            avg_probe_duration_ms: None,
            ewma_success_rate: None,
            bundled,
        }
    }

    #[test]
    fn empty_cache_has_no_gaps() {
        let cache = RegistryCache::new();
        let gaps = cache.coverage_gaps(&["vram_oom"]);
        assert_eq!(gaps.len(), 1);
        assert_eq!(gaps[0], "vram_oom");
    }

    #[test]
    fn cache_detects_coverage() {
        let mut cache = RegistryCache::new();
        cache.upsert("test-skill", test_entry(LifecycleStatus::Active, "0.1.0", vec!["vram_oom"], SkillSource::Bundled));
        let gaps = cache.coverage_gaps(&["vram_oom", "swap_pressure"]);
        assert_eq!(gaps, vec!["swap_pressure"]);
    }

    #[test]
    fn lookup_symptom_finds_active() {
        let mut cache = RegistryCache::new();
        cache.upsert("active-skill", test_entry(LifecycleStatus::Active, "1.0.0", vec!["vram_oom"], SkillSource::Bundled));
        cache.upsert("retired-skill", test_entry(LifecycleStatus::Retired, "0.0.1", vec!["vram_oom"], SkillSource::Manual));
        let results = cache.lookup_symptom("vram_oom");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].version, "1.0.0");
    }

    #[test]
    fn staleness_detection_works() {
        assert!(RegistryCache::is_stale("2025-11-01", "2026-05-13"));
        assert!(!RegistryCache::is_stale("2026-05-01", "2026-05-13"));
    }

    // Safety scanner tests have moved to registry::safety::tests.

    #[test]
    fn record_execution_increments_probe_runs() {
        let mut cache = RegistryCache::new();
        cache.upsert("test-skill", test_entry(LifecycleStatus::Active, "0.1.0", vec!["vram_oom"], SkillSource::Bundled));
        cache.record_execution("test-skill", true, 50, None);
        assert_eq!(cache.skills["test-skill"].probe_runs, 1);
        assert_eq!(cache.skills["test-skill"].recent_probe_failures, 0);
        assert!(cache.skills["test-skill"].last_probe_run_at.is_some());
        assert!(cache.skills["test-skill"].avg_probe_duration_ms.is_some());
    }

    #[test]
    fn record_execution_tracks_failures() {
        let mut cache = RegistryCache::new();
        let mut entry = test_entry(LifecycleStatus::Active, "0.1.0", vec!["vram_oom"], SkillSource::Bundled);
        entry.probe_runs = 5;
        entry.recent_probe_failures = 1;
        cache.upsert("test-skill", entry);
        cache.record_execution("test-skill", false, 120, Some("connection refused"));
        assert_eq!(cache.skills["test-skill"].probe_runs, 6);
        assert_eq!(cache.skills["test-skill"].recent_probe_failures, 2);
        assert_eq!(cache.skills["test-skill"].last_error.as_deref(), Some("connection refused"));
    }

    #[test]
    fn record_execution_updates_ewma() {
        let mut cache = RegistryCache::new();
        let mut entry = test_entry(LifecycleStatus::Active, "0.1.0", vec!["vram_oom"], SkillSource::Bundled);
        entry.probe_runs = 1;
        entry.avg_probe_duration_ms = Some(100.0);
        cache.upsert("test-skill", entry);
        cache.record_execution("test-skill", true, 200, None);
        let ewma = cache.skills["test-skill"].avg_probe_duration_ms.unwrap();
        assert!((ewma - 120.0).abs() < 0.01, "EWMA should be ~120 (0.2*200 + 0.8*100), got {ewma}");
    }

    #[test]
    fn record_execution_noop_on_missing_skill() {
        let mut cache = RegistryCache::new();
        cache.record_execution("nonexistent", false, 0, None);
        assert!(cache.skills.is_empty());
        cache.record_execution("nonexistent", true, 10, None);
        assert!(cache.skills.is_empty());
    }

    #[test]
    fn record_intervention_increments() {
        let mut cache = RegistryCache::new();
        cache.upsert("test-skill", test_entry(LifecycleStatus::Active, "0.1.0", vec!["vram_oom"], SkillSource::Bundled));
        cache.record_intervention("test-skill", true, None);
        assert_eq!(cache.skills["test-skill"].intervention_runs, 1);
        assert_eq!(cache.skills["test-skill"].recent_intervention_failures, 0);
        cache.record_intervention("test-skill", false, Some("timeout"));
        assert_eq!(cache.skills["test-skill"].intervention_runs, 2);
        assert_eq!(cache.skills["test-skill"].recent_intervention_failures, 1);
        assert_eq!(cache.skills["test-skill"].last_error.as_deref(), Some("timeout"));
    }

    #[test]
    fn remove_entry_returns_entry() {
        let mut cache = RegistryCache::new();
        cache.upsert("test-skill", test_entry(LifecycleStatus::Active, "0.1.0", vec![], SkillSource::Workshop));
        let removed = cache.remove_entry("test-skill");
        assert!(removed.is_some());
        assert!(!cache.skills.contains_key("test-skill"));
    }

    // Quality scoring and freshness tests have moved to registry::health::tests.
    // The delegating methods on RegistryCache are tested transitively.

    #[test]
    fn with_update_persists_changes() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("local-cache.yaml");
        RegistryCache::with_update(&path, |cache| {
            cache.upsert("test-skill", test_entry(LifecycleStatus::Active, "0.1.0", vec!["test"], SkillSource::Workshop));
        }).unwrap();
        let loaded = RegistryCache::load(&path).unwrap();
        assert!(loaded.skills.contains_key("test-skill"));
        assert_eq!(loaded.skills["test-skill"].probe_runs, 0);
    }
}

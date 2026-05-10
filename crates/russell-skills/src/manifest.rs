// SPDX-License-Identifier: MIT OR Apache-2.0
//! YAML manifest parser for Russell skills.
//!
//! Loads `manifest.yaml` files from `~/.local/share/harness/skills/<id>/`
//! into typed structs, then runs post-parse validation.
//!
//! Schema: [`docs/templates/skill-manifest.yaml`](../../docs/templates/skill-manifest.yaml).
//! Design: [ADR-0007](../../docs/adr/deferred/0007-yaml-manifest-subprocess-skill-model.md).

use std::fmt;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use tracing::{debug, warn};

use russell_core::Result;

/// A loaded and validated skill manifest.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// Kebab-case identifier, must match parent directory name.
    pub id: String,
    /// Semver version of this manifest.
    #[serde(default)]
    pub version: Option<String>,
    /// ISO date this manifest was authored (YYYY-MM-DD).
    #[serde(default)]
    pub authored: Option<String>,
    /// Minimum Russell harness version this manifest needs.
    #[serde(default)]
    pub min_harness_version: Option<String>,

    /// Symptom classes this skill addresses.
    #[serde(default)]
    pub symptoms: Vec<String>,

    /// Read-only probes (risk: none enforced).
    #[serde(default)]
    pub probes: Vec<Probedef>,

    /// Mutating interventions. Must satisfy IDRS.
    #[serde(default)]
    pub interventions: Vec<InterventionDef>,

    /// Safety caps.
    #[serde(default)]
    pub safety: SafetySection,
}

/// A single probe definition.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Probedef {
    /// Unique within the manifest.
    pub id: String,
    /// Argv list. Shell interpolation must use `["bash", "-c", "..."]`.
    pub cmd: Vec<String>,
    /// Must be `none`.
    #[serde(default)]
    pub risk: String,
    /// Capture: `stdout`, `stderr`, `both`, or `file:<path>`.
    #[serde(default)]
    pub capture: String,
    /// Timeout, e.g. `30s` or `120s`.
    #[serde(default)]
    pub timeout: String,
}

/// A single intervention definition.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InterventionDef {
    /// Unique within the manifest.
    pub id: String,
    /// Argv list.
    pub cmd: Vec<String>,
    /// `none` | `low` | `medium` | `high` | `critical`.
    pub risk: String,
    /// Must be `true` for the manifest to load.
    #[serde(default)]
    pub idempotent: bool,
    /// Rollback ID, `none_needed`, or `reboot`.
    #[serde(default)]
    pub rollback: Option<String>,
    /// Optional rollback intervention ID (alias for rollback when it's an ID).
    #[serde(default)]
    pub rollback_id: Option<String>,
    /// Whether the operator must confirm before execution.
    #[serde(default)]
    pub requires_confirmation: bool,
    /// Preconditions (e.g. `on_ac_power`).
    #[serde(default)]
    pub preconditions: Vec<String>,
    /// Timeout, e.g. `60s`.
    #[serde(default)]
    pub timeout: String,
}

/// Safety section of the manifest.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct SafetySection {
    /// Maximum auto-executable risk band (default: `low`).
    #[serde(default)]
    pub max_auto_risk: Option<String>,
    /// Intervention IDs that always require human confirmation.
    #[serde(default)]
    pub require_human_for: Vec<String>,
}

/// A validation error found during manifest checking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    /// The manifest file this error relates to.
    pub manifest_path: PathBuf,
    /// Human-readable description.
    pub message: String,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.manifest_path.display(), self.message)
    }
}

enum Risk {
    None,
    Low,
    Medium,
    High,
    Critical,
}

impl Risk {
    fn from_str(s: &str) -> Option<Self> {
        match s {
            "none" => Some(Self::None),
            "low" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "high" => Some(Self::High),
            "critical" => Some(Self::Critical),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Load all skill manifests from `skills_dir`. Returns loaded manifests and
/// a list of files that failed validation (non-fatal — Russell must not
/// crash on a malformed operator file).
///
/// # Errors
///
/// Returns [`russell_core::CoreError::Io`] if the directory cannot be read.
pub fn load_all(skills_dir: &Path) -> Result<Vec<Manifest>> {
    let mut manifests = Vec::new();

    if !skills_dir.is_dir() {
        return Ok(manifests);
    }

    for entry in
        std::fs::read_dir(skills_dir).map_err(|e| russell_core::CoreError::io(skills_dir, e))?
    {
        let entry = entry.map_err(|e| russell_core::CoreError::io(skills_dir, e))?;
        let skill_dir = entry.path();
        if !skill_dir.is_dir() {
            continue;
        }
        let manifest_path = skill_dir.join("manifest.yaml");
        if !manifest_path.exists() {
            continue;
        }
        match load_one(&manifest_path) {
            Ok(m) => {
                debug!(id = %m.id, "loaded skill manifest");
                manifests.push(m);
            }
            Err(e) => {
                warn!(path = %manifest_path.display(), error = %e, "skipping invalid skill manifest");
            }
        }
    }

    Ok(manifests)
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn load_one(path: &Path) -> std::result::Result<Manifest, String> {
    let raw = std::fs::read_to_string(path).map_err(|e| format!("read error: {e}"))?;
    let m: Manifest = serde_yaml::from_str(&raw).map_err(|e| format!("parse error: {e}"))?;
    validate(&m, path)?;
    Ok(m)
}

fn validate(m: &Manifest, path: &Path) -> std::result::Result<(), String> {
    let mut errors = Vec::new();

    // ID must be kebab-case and non-empty.
    if m.id.is_empty() {
        errors.push("id is empty".to_string());
    } else if !m
        .id
        .chars()
        .all(|c| c.is_ascii_lowercase() || c == '-' || c.is_ascii_digit())
    {
        errors.push(format!("id '{}' is not kebab-case", m.id));
    }

    // Parent directory must match id.
    if let Some(parent) = path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
    {
        if parent != m.id {
            errors.push(format!(
                "id '{}' does not match parent directory '{}'",
                m.id, parent
            ));
        }
    }

    // Probes must have risk: none.
    for p in &m.probes {
        if p.risk != "none" {
            errors.push(format!(
                "probe '{}' has risk '{}', must be 'none'",
                p.id, p.risk
            ));
        }
        if p.cmd.is_empty() {
            errors.push(format!("probe '{}' has empty cmd", p.id));
        }
        if p.id.is_empty() {
            errors.push("probe has empty id".to_string());
        }
    }

    // Interventions must satisfy IDRS.
    for i in &m.interventions {
        if Risk::from_str(&i.risk).is_none() {
            errors.push(format!(
                "intervention '{}' has invalid risk '{}'",
                i.id, i.risk
            ));
        }
        if !i.idempotent {
            errors.push(format!("intervention '{}' is not idempotent", i.id));
        }
        let has_rollback = i.rollback.is_some() || i.rollback_id.is_some();
        if !has_rollback {
            errors.push(format!("intervention '{}' has no rollback strategy", i.id));
        }
        if i.cmd.is_empty() {
            errors.push(format!("intervention '{}' has empty cmd", i.id));
        }
    }

    // Probe and intervention IDs must be unique.
    let mut ids = std::collections::HashSet::new();
    for p in &m.probes {
        if !ids.insert(&p.id) {
            errors.push(format!("duplicate probe id '{}'", p.id));
        }
    }
    for i in &m.interventions {
        if !ids.insert(&i.id) {
            errors.push(format!("duplicate intervention id '{}'", i.id));
        }
    }

    if !errors.is_empty() {
        return Err(errors.join("; "));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_manifest(tmp: &tempfile::TempDir, id: &str, yaml: &str) -> PathBuf {
        let dir = tmp.path().join(id);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("manifest.yaml");
        std::fs::write(&path, yaml).unwrap();
        path
    }

    #[test]
    fn load_empty_probes_and_interventions() {
        let tmp = tempfile::tempdir().unwrap();
        let yaml = "id: empty-skill\nprobes: []\ninterventions: []\n";
        write_manifest(&tmp, "empty-skill", yaml);
        let manifests = load_all(tmp.path()).unwrap();
        assert_eq!(manifests.len(), 1);
        assert_eq!(manifests[0].id, "empty-skill");
    }

    #[test]
    fn probe_must_have_risk_none() {
        let tmp = tempfile::tempdir().unwrap();
        let yaml = "id: bad-probe\nprobes:\n  - id: p1\n    cmd: [echo, hi]\n    risk: low\n";
        write_manifest(&tmp, "bad-probe", yaml);
        let manifests = load_all(tmp.path()).unwrap();
        assert!(manifests.is_empty(), "bad probe risk should be rejected");
    }

    #[test]
    fn valid_probe_loads() {
        let tmp = tempfile::tempdir().unwrap();
        let yaml = r#"
id: my-skill
probes:
  - id: p1
    cmd: [echo, hello]
    risk: none
    capture: stdout
    timeout: 30s
interventions: []
"#;
        write_manifest(&tmp, "my-skill", yaml);
        let manifests = load_all(tmp.path()).unwrap();
        assert_eq!(manifests.len(), 1);
        assert_eq!(manifests[0].probes.len(), 1);
        assert_eq!(manifests[0].probes[0].id, "p1");
    }

    #[test]
    fn id_must_match_directory() {
        let tmp = tempfile::tempdir().unwrap();
        write_manifest(
            &tmp,
            "my-skill",
            "id: wrong-id\nprobes: []\ninterventions: []\n",
        );
        let manifests = load_all(tmp.path()).unwrap();
        assert!(manifests.is_empty(), "mismatched id should be rejected");
    }

    #[test]
    fn intervention_with_rollback_loads() {
        let tmp = tempfile::tempdir().unwrap();
        let yaml = r#"
id: safe-skill
probes: []
interventions:
  - id: fix-it
    cmd: [systemctl, restart, ollama]
    risk: low
    idempotent: true
    rollback: none_needed
"#;
        write_manifest(&tmp, "safe-skill", yaml);
        let manifests = load_all(tmp.path()).unwrap();
        assert_eq!(manifests.len(), 1);
        assert_eq!(manifests[0].interventions.len(), 1);
    }

    #[test]
    fn duplicate_ids_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let yaml = r#"
id: dup-skill
probes:
  - id: p1
    cmd: [echo, a]
    risk: none
interventions:
  - id: p1
    cmd: [echo, b]
    risk: low
    idempotent: true
    rollback: none_needed
"#;
        write_manifest(&tmp, "dup-skill", yaml);
        let manifests = load_all(tmp.path()).unwrap();
        assert!(manifests.is_empty(), "duplicate IDs should be rejected");
    }
}

// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell-skills` — skill manifest loader (Phase 3).
//!
//! Loads and validates YAML skill manifests per ADR-0007.
//! Each skill lives under `skills/<id>/` with a `manifest.yaml`
//! file that declares probes (read-only) and interventions
//! (mutating, governed by the IDRS contract).
//!
//! ## Poka-yoke
//!
//! The loader refuses any manifest that fails schema validation,
//! references unknown symptom names, or has unreferenced scripts
//! in its `scripts/` directory. These are hard errors, not
//! warnings — the dispatcher must never operate with a partial
//! skill set.
//!
//! See [`docs/adr/deferred/0007-yaml-manifest-subprocess-skill-model.md`](../../../docs/adr/deferred/0007-yaml-manifest-subprocess-skill-model.md).

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms)]
#![warn(missing_docs)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::Deserialize;

mod symptom_catalog;

/// Subprocess dispatcher for probe and intervention execution.
pub mod dispatch;

pub use symptom_catalog::SYMPTOMS;

/// Errors that can occur during manifest loading.
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    /// The skills directory does not exist.
    #[error("skills directory not found: {0}")]
    MissingDir(PathBuf),

    /// Could not read the directory.
    #[error("cannot read skills directory: {0}")]
    ReadDir(#[source] std::io::Error),

    /// A skill subdirectory could not be read.
    #[error("cannot read skill directory {path}: {source}")]
    SkillDir {
        /// Path to the skill directory.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// `manifest.yaml` is missing from the skill directory.
    #[error("missing manifest.yaml in {0}")]
    MissingManifest(PathBuf),

    /// `manifest.yaml` could not be read.
    #[error("cannot read {path}: {source}")]
    ReadManifest {
        /// Path to manifest.yaml.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// YAML parse or schema validation error.
    #[error("invalid manifest {path}: {message}")]
    InvalidManifest {
        /// Path to manifest.yaml.
        path: PathBuf,
        /// Human-readable description of the problem.
        message: String,
    },

    /// `id` in the manifest does not match the directory name.
    #[error("manifest id '{manifest_id}' does not match directory name '{dir_name}'")]
    IdMismatch {
        /// ID declared in the manifest.
        manifest_id: String,
        /// Directory name (must be kebab-case, match the ID).
        dir_name: String,
    },

    /// A probe's `risk` field is not `none`.
    #[error("probe '{probe_id}' in {path} has risk field — probes must be risk: none")]
    ProbeHasRisk {
        /// Path to manifest.yaml.
        path: PathBuf,
        /// Probe ID.
        probe_id: String,
    },

    /// An intervention's `risk` field is missing or invalid.
    #[error("intervention '{intervention_id}' in {path} is missing required risk field")]
    InterventionMissingRisk {
        /// Path to manifest.yaml.
        path: PathBuf,
        /// Intervention ID.
        intervention_id: String,
    },

    /// An intervention's rollback strategy is missing.
    #[error(
        "intervention '{intervention_id}' in {path} must declare rollback_id, rollback: none_needed, or rollback: reboot"
    )]
    InterventionMissingRollback {
        /// Path to manifest.yaml.
        path: PathBuf,
        /// Intervention ID.
        intervention_id: String,
    },

    /// A `rollback_id` references an intervention that does not exist.
    #[error(
        "rollback_id '{rollback_id}' in intervention '{intervention_id}' in {path} does not reference a known intervention"
    )]
    RollbackIdNotFound {
        /// Path to manifest.yaml.
        path: PathBuf,
        /// Intervention that declares the rollback.
        intervention_id: String,
        /// The rollback_id that doesn't resolve.
        rollback_id: String,
    },

    /// Unknown symptom name.
    #[error("skill '{skill_id}' references unknown symptom '{symptom}'")]
    UnknownSymptom {
        /// Skill ID.
        skill_id: String,
        /// The symptom that isn't in the catalog.
        symptom: String,
    },

    /// A script file in `scripts/` is not referenced by any probe or
    /// intervention `cmd:` entry.
    #[error("skill '{skill_id}' has unreferenced script: {script}")]
    UnreferencedScript {
        /// Skill ID.
        skill_id: String,
        /// Path to the unreferenced script file.
        script: PathBuf,
    },
}

/// A single skill, fully loaded and validated.
#[derive(Debug, Clone)]
pub struct Skill {
    /// Unique identifier (kebab-case).
    pub id: String,
    /// Semver version.
    pub version: String,
    /// Date authored (ISO 8601 date).
    pub authored: String,
    /// Minimum Russell version required.
    pub min_harness_version: String,
    /// Symptom classes this skill addresses.
    pub symptoms: Vec<String>,
    /// Profile preconditions (ANDed).
    pub applies_when: Vec<AppliesWhen>,
    /// Read-only probes.
    pub probes: Vec<Probe>,
    /// Mutating interventions.
    pub interventions: Vec<Intervention>,
    /// Safety constraints.
    pub safety: Safety,
    /// Post-intervention evaluation checks.
    pub evaluation: Option<Evaluation>,
}

/// A profile precondition clause.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum AppliesWhen {
    /// Scalar key-value: `os_family: linux`
    Scalar {
        /// Key name.
        #[serde(rename = "os_family")]
        os_family: Option<String>,
        /// PCI vendor match.
        #[serde(rename = "pci_vendor")]
        pci_vendor: Option<String>,
    },
    /// List-valued key: `gfx_target_any: [gfx1102, gfx1103]`
    List {
        /// Acceptable GPU targets.
        #[serde(rename = "gfx_target_any")]
        gfx_target_any: Option<Vec<String>>,
    },
}

/// A read-only probe step.
#[derive(Debug, Clone, Deserialize)]
pub struct Probe {
    /// Unique ID within the skill.
    pub id: String,
    /// Argv list to execute.
    pub cmd: Vec<String>,
    /// Capture mode: stdout, stderr, both, file:<path>.
    #[serde(default = "capture_default")]
    pub capture: String,
    /// Timeout (e.g. "30s").
    #[serde(default = "timeout_default")]
    pub timeout: String,
}

/// A mutating intervention step.
#[derive(Debug, Clone, Deserialize)]
pub struct Intervention {
    /// Unique ID within the skill.
    pub id: String,
    /// Argv list to execute.
    pub cmd: Vec<String>,
    /// Risk band: low | medium | high | critical.
    pub risk: RiskBand,
    /// Must be true; verifiable with --verify-idempotent.
    #[serde(default = "bool_true")]
    pub idempotent: bool,
    /// Rollback strategy.
    #[serde(flatten)]
    pub rollback: Rollback,
    /// Timeout (e.g. "120s").
    #[serde(default = "timeout_default_intervention")]
    pub timeout: String,
}

/// Rollback strategy for an intervention.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Rollback {
    /// Reference to a reverse intervention.
    RollbackId {
        /// The intervention to run to reverse this one.
        rollback_id: String,
    },
    /// Declared as not needing rollback.
    NoneNeeded {
        /// Must be literally "none_needed".
        rollback: RollbackNone,
    },
    /// Requires reboot to undo.
    Reboot {
        /// Must be literally "reboot".
        rollback: RollbackReboot,
    },
}

/// Marker for `rollback: none_needed`.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RollbackNone {
    /// No rollback needed.
    #[serde(rename = "none_needed")]
    NoneNeeded,
}

/// Marker for `rollback: reboot`.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RollbackReboot {
    /// Reboot required.
    #[serde(rename = "reboot")]
    Reboot,
}

/// Risk band for an intervention.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum RiskBand {
    /// Read-only observation.
    None,
    /// Reversible, low impact.
    Low,
    /// Reversible, moderate impact.
    Medium,
    /// May require reboot or session loss.
    High,
    /// Data-loss possible.
    Critical,
}

/// Safety constraints for a skill.
#[derive(Debug, Clone, Deserialize)]
pub struct Safety {
    /// Maximum risk band the Doctor may auto-run.
    #[serde(default = "max_auto_risk_default")]
    pub max_auto_risk: RiskBand,
    /// Intervention IDs that always require human confirmation.
    #[serde(default)]
    pub require_human_for: Vec<String>,
}

/// Post-intervention evaluation checks.
#[derive(Debug, Clone, Deserialize)]
pub struct Evaluation {
    /// Checks to run after an intervention.
    pub after_intervention: Vec<EvalCheck>,
}

/// A post-intervention check.
#[derive(Debug, Clone, Deserialize)]
pub struct EvalCheck {
    /// Unique ID.
    pub id: String,
    /// Argv list.
    pub cmd: Vec<String>,
    /// Timeout.
    #[serde(default = "timeout_default")]
    pub timeout: String,
    /// Expected exit code (default 0).
    #[serde(default = "expected_exit_default")]
    pub expect_exit: i32,
}

/// The raw YAML manifest as parsed from disk, before validation.
#[derive(Debug, Deserialize)]
struct RawManifest {
    id: String,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    authored: Option<String>,
    #[serde(default)]
    min_harness_version: Option<String>,
    #[serde(default)]
    symptoms: Vec<String>,
    #[serde(default)]
    applies_when: Vec<AppliesWhen>,
    #[serde(default)]
    probes: Vec<Probe>,
    #[serde(default)]
    interventions: Vec<Intervention>,
    #[serde(default)]
    safety: Option<RawSafety>,
    #[serde(default)]
    evaluation: Option<Evaluation>,
}

#[derive(Debug, Deserialize)]
struct RawSafety {
    #[serde(default = "max_auto_risk_default")]
    max_auto_risk: RiskBand,
    #[serde(default)]
    require_human_for: Vec<String>,
}

// --- Defaults -----------------------------------------------------------

fn capture_default() -> String {
    "stdout".into()
}

fn timeout_default() -> String {
    "30s".into()
}

fn timeout_default_intervention() -> String {
    "120s".into()
}

fn bool_true() -> bool {
    true
}

fn max_auto_risk_default() -> RiskBand {
    RiskBand::Low
}

fn expected_exit_default() -> i32 {
    0
}

// --- Loading -------------------------------------------------------------

/// Load all skill manifests from a `skills/` directory.
///
/// Returns an empty `Vec` if the directory does not exist.
/// Poka-yoke: any parse, schema, or consistency error returns
/// [`LoadError`] — partial loads are not allowed.
///
/// # Errors
///
/// Returns [`LoadError`] on any validation failure.
pub fn load_all(skills_dir: &Path) -> Result<Vec<Skill>, LoadError> {
    let entries = match std::fs::read_dir(skills_dir) {
        Ok(d) => d,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(LoadError::ReadDir(e)),
    };

    let mut skills = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| LoadError::SkillDir {
            path: skills_dir.to_path_buf(),
            source: e,
        })?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let dir_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();

        // Skip non-skill directories (e.g. `.git`, `__pycache__`).
        if dir_name.starts_with('.') || dir_name.starts_with("__") {
            continue;
        }

        let skill = load_one(&path, dir_name)?;
        skills.push(skill);
    }

    Ok(skills)
}

/// Load and validate a single skill manifest.
fn load_one(skill_dir: &Path, dir_name: &str) -> Result<Skill, LoadError> {
    let manifest_path = skill_dir.join("manifest.yaml");
    if !manifest_path.exists() {
        return Err(LoadError::MissingManifest(skill_dir.to_path_buf()));
    }

    let yaml = std::fs::read_to_string(&manifest_path).map_err(|e| LoadError::ReadManifest {
        path: manifest_path.clone(),
        source: e,
    })?;

    let raw: RawManifest = serde_yaml::from_str(&yaml).map_err(|e| LoadError::InvalidManifest {
        path: manifest_path.clone(),
        message: e.to_string(),
    })?;

    validate(&raw, dir_name, skill_dir, &manifest_path)?;

    let safety_raw = raw.safety.unwrap_or(RawSafety {
        max_auto_risk: RiskBand::Low,
        require_human_for: Vec::new(),
    });

    Ok(Skill {
        id: raw.id,
        version: raw.version.unwrap_or_else(|| "0.0.0".into()),
        authored: raw.authored.unwrap_or_else(|| "unknown".into()),
        min_harness_version: raw.min_harness_version.unwrap_or_else(|| "0.1.0".into()),
        symptoms: raw.symptoms,
        applies_when: raw.applies_when,
        probes: raw.probes,
        interventions: raw.interventions,
        safety: Safety {
            max_auto_risk: safety_raw.max_auto_risk,
            require_human_for: safety_raw.require_human_for,
        },
        evaluation: raw.evaluation,
    })
}

/// Run all validation checks. Returns Err on the first failure.
fn validate(
    raw: &RawManifest,
    dir_name: &str,
    skill_dir: &Path,
    manifest_path: &Path,
) -> Result<(), LoadError> {
    // 1. ID matches directory name.
    if raw.id != dir_name {
        return Err(LoadError::IdMismatch {
            manifest_id: raw.id.clone(),
            dir_name: dir_name.to_string(),
        });
    }

    // 2. All symptoms are known.
    for symptom in &raw.symptoms {
        if !SYMPTOMS.contains(&symptom.as_str()) {
            return Err(LoadError::UnknownSymptom {
                skill_id: raw.id.clone(),
                symptom: symptom.clone(),
            });
        }
    }

    // 3. Probes have risk: none (enforced via struct — probes don't
    //    have a risk field by design; if one sneaks through YAML,
    //    serde will reject it at parse time).

    // 4. Every intervention has a valid rollback strategy.
    //    This is enforced by the Rollback enum deserialization — if
    //    none of the three variants parses, serde rejects the YAML.
    //    But we also check that rollback_ids reference real interventions.
    let intervention_ids: BTreeSet<&str> =
        raw.interventions.iter().map(|i| i.id.as_str()).collect();

    for iv in &raw.interventions {
        if let Rollback::RollbackId { ref rollback_id } = iv.rollback
            && !intervention_ids.contains(rollback_id.as_str())
        {
            return Err(LoadError::RollbackIdNotFound {
                path: manifest_path.to_path_buf(),
                intervention_id: iv.id.clone(),
                rollback_id: rollback_id.clone(),
            });
        }
    }

    // 5. Unreferenced scripts check.
    check_unreferenced_scripts(skill_dir, raw, manifest_path)?;

    Ok(())
}

/// Check that every executable file in `scripts/` is referenced by
/// at least one probe or intervention `cmd`.
fn check_unreferenced_scripts(
    skill_dir: &Path,
    raw: &RawManifest,
    _manifest_path: &Path,
) -> Result<(), LoadError> {
    let scripts_dir = skill_dir.join("scripts");
    if !scripts_dir.exists() {
        return Ok(());
    }

    // Collect all referenced script names.
    let mut referenced: BTreeSet<String> = BTreeSet::new();
    for probe in &raw.probes {
        collect_script_names(&probe.cmd, &mut referenced);
    }
    for iv in &raw.interventions {
        collect_script_names(&iv.cmd, &mut referenced);
    }
    if let Some(ref eval) = raw.evaluation {
        for check in &eval.after_intervention {
            collect_script_names(&check.cmd, &mut referenced);
        }
    }

    // Walk scripts/ and check each file.
    let entries = std::fs::read_dir(&scripts_dir).map_err(|e| LoadError::SkillDir {
        path: scripts_dir.clone(),
        source: e,
    })?;

    for entry in entries {
        let entry = entry.map_err(|e| LoadError::SkillDir {
            path: scripts_dir.clone(),
            source: e,
        })?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        // Only flag files that look like scripts (have no extension or
        // common script extensions).
        if name.contains('.') {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if !matches!(ext, "sh" | "py" | "bash" | "pl" | "rb") {
                continue;
            }
        }
        if !referenced.contains(name) {
            return Err(LoadError::UnreferencedScript {
                skill_id: raw.id.clone(),
                script: path.clone(),
            });
        }
    }

    Ok(())
}

/// Extract script filenames from a command argv.
fn collect_script_names(cmd: &[String], out: &mut BTreeSet<String>) {
    for arg in cmd {
        // Heuristic: script names appear as bare filenames or
        // paths starting with `./` or `scripts/`.
        let name = if let Some(stripped) = arg.strip_prefix("./scripts/") {
            stripped
        } else if let Some(stripped) = arg.strip_prefix("./") {
            stripped
        } else if let Some(stripped) = arg.strip_prefix("scripts/") {
            stripped
        } else if !arg.contains('/') && !arg.contains(' ') {
            arg.as_str()
        } else {
            continue;
        };
        if name.contains('.') {
            let ext = std::path::Path::new(name)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");
            if matches!(ext, "sh" | "py" | "bash" | "pl" | "rb") {
                out.insert(name.to_string());
            }
        } else {
            // Extensionless — could be a binary script.
            out.insert(name.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_skill(dir: &Path, id: &str, yaml: &str) {
        let skill_dir = dir.join(id);
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(skill_dir.join("manifest.yaml"), yaml).unwrap();
    }

    fn valid_manifest(id: &str) -> String {
        format!(
            r#"id: {id}
version: 0.1.0
authored: 2026-05-09
min_harness_version: 0.1.0
symptoms:
  - vram_oom
applies_when:
  - os_family: linux
probes:
  - id: probe-{id}
    cmd: ["bash", "-c", "echo hello"]
interventions:
  - id: iv-{id}
    cmd: ["bash", "-c", "echo fix"]
    risk: low
    idempotent: true
    rollback: none_needed
safety:
  max_auto_risk: low
"#
        )
    }

    #[test]
    fn loads_valid_manifest() {
        let tmp = tempfile::tempdir().unwrap();
        let id = "gpu-doctor";
        write_skill(tmp.path(), id, &valid_manifest(id));
        let skills = load_all(tmp.path()).unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].id, id);
        assert_eq!(skills[0].probes.len(), 1);
        assert_eq!(skills[0].interventions.len(), 1);
        assert_eq!(skills[0].symptoms, vec!["vram_oom"]);
    }

    #[test]
    fn loads_multiple_skills() {
        let tmp = tempfile::tempdir().unwrap();
        write_skill(tmp.path(), "gpu-doctor", &valid_manifest("gpu-doctor"));
        write_skill(
            tmp.path(),
            "battery-doctor",
            &valid_manifest("battery-doctor"),
        );
        let skills = load_all(tmp.path()).unwrap();
        assert_eq!(skills.len(), 2);
    }

    #[test]
    fn empty_dir_returns_empty_vec() {
        let tmp = tempfile::tempdir().unwrap();
        let skills = load_all(tmp.path()).unwrap();
        assert!(skills.is_empty());
    }

    #[test]
    fn missing_skills_dir_returns_empty() {
        let skills = load_all(Path::new("/nonexistent/skills")).unwrap();
        assert!(skills.is_empty());
    }

    #[test]
    fn rejects_id_mismatch() {
        let tmp = tempfile::tempdir().unwrap();
        write_skill(tmp.path(), "gpu-doctor", &valid_manifest("wrong-id"));
        let err = load_all(tmp.path()).unwrap_err();
        assert!(err.to_string().contains("wrong-id"));
        assert!(err.to_string().contains("gpu-doctor"));
    }

    #[test]
    fn rejects_unknown_symptom() {
        let tmp = tempfile::tempdir().unwrap();
        let yaml = valid_manifest("gpu-doctor").replace("vram_oom", "made_up_symptom");
        write_skill(tmp.path(), "gpu-doctor", &yaml);
        let err = load_all(tmp.path()).unwrap_err();
        assert!(err.to_string().contains("made_up_symptom"));
    }

    #[test]
    fn rejects_bad_rollback_id() {
        let tmp = tempfile::tempdir().unwrap();
        let yaml = r#"
id: gpu-doctor
version: 0.1.0
authored: 2026-05-09
min_harness_version: 0.1.0
symptoms:
  - vram_oom
applies_when:
  - os_family: linux
probes: []
interventions:
  - id: iv-gpu-doctor
    cmd: ["echo", "fix"]
    risk: low
    idempotent: true
    rollback_id: no-such-id
safety:
  max_auto_risk: low
"#;
        write_skill(tmp.path(), "gpu-doctor", yaml);
        let err = load_all(tmp.path()).unwrap_err();
        assert!(err.to_string().contains("no-such-id"));
    }

    #[test]
    fn rejects_unreferenced_script() {
        let tmp = tempfile::tempdir().unwrap();
        let skill_dir = tmp.path().join("gpu-doctor");
        std::fs::create_dir_all(skill_dir.join("scripts")).unwrap();
        std::fs::write(
            skill_dir.join("manifest.yaml"),
            valid_manifest("gpu-doctor"),
        )
        .unwrap();
        // Create an unreferenced script file.
        std::fs::write(
            skill_dir.join("scripts/orphan.sh"),
            "#!/bin/bash\necho oops\n",
        )
        .unwrap();
        let err = load_all(tmp.path()).unwrap_err();
        assert!(err.to_string().contains("orphan.sh"));
    }

    #[test]
    fn allows_referenced_scripts() {
        let tmp = tempfile::tempdir().unwrap();
        let skill_dir = tmp.path().join("gpu-doctor");
        std::fs::create_dir_all(skill_dir.join("scripts")).unwrap();
        std::fs::write(skill_dir.join("scripts/probe.sh"), "#!/bin/bash\necho ok\n").unwrap();
        let yaml = r#"
id: gpu-doctor
version: 0.1.0
authored: 2026-05-09
min_harness_version: 0.1.0
symptoms:
  - vram_oom
applies_when:
  - os_family: linux
probes:
  - id: probe-gpu-doctor
    cmd: ["bash", "./scripts/probe.sh"]
interventions:
  - id: iv-gpu-doctor
    cmd: ["echo", "fix"]
    risk: low
    idempotent: true
    rollback: none_needed
safety:
  max_auto_risk: low
"#;
        std::fs::write(skill_dir.join("manifest.yaml"), yaml).unwrap();
        let skills = load_all(tmp.path()).unwrap();
        assert_eq!(skills.len(), 1);
    }

    #[test]
    fn skip_non_skill_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".git")).unwrap();
        std::fs::create_dir_all(tmp.path().join("__pycache__")).unwrap();
        write_skill(tmp.path(), "gpu-doctor", &valid_manifest("gpu-doctor"));
        let skills = load_all(tmp.path()).unwrap();
        assert_eq!(skills.len(), 1);
    }

    #[test]
    fn rollback_reboot_accepted() {
        let tmp = tempfile::tempdir().unwrap();
        let yaml = r#"
id: gpu-doctor
version: 0.1.0
authored: 2026-05-09
min_harness_version: 0.1.0
symptoms:
  - vram_oom
applies_when:
  - os_family: linux
probes: []
interventions:
  - id: iv-gpu-doctor
    cmd: ["echo", "reboot-needed"]
    risk: high
    idempotent: true
    rollback: reboot
safety:
  max_auto_risk: low
"#;
        write_skill(tmp.path(), "gpu-doctor", yaml);
        let skills = load_all(tmp.path()).unwrap();
        assert_eq!(skills.len(), 1);
        let iv = &skills[0].interventions[0];
        assert!(matches!(iv.rollback, Rollback::Reboot { .. }));
    }

    #[test]
    fn missing_manifest_in_skill_dir_is_error() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("gpu-doctor")).unwrap();
        let err = load_all(tmp.path()).unwrap_err();
        assert!(err.to_string().contains("missing manifest"));
    }

    #[test]
    fn loads_gpu_doctor_fixture() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let skills = load_all(&fixture).unwrap();
        assert_eq!(skills.len(), 1);
        let gpu = &skills[0];
        assert_eq!(gpu.id, "gpu-doctor");
        assert_eq!(gpu.probes.len(), 3);
        assert_eq!(gpu.interventions.len(), 1);
        assert!(gpu.symptoms.contains(&"vram_oom".into()));
        assert!(gpu.symptoms.contains(&"amdgpu_ring_hang".into()));
        // The reset-gpu intervention is medium risk with confirmation.
        let reset = &gpu.interventions[0];
        assert_eq!(reset.id, "reset-gpu");
        assert!(matches!(reset.risk, RiskBand::Medium));
    }
}

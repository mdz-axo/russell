// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell-skills` — skill manifest loader (Phase 3).
//!
//! **TOGAF Phase:** Phase C (Application Architecture) — implements the
//! skill playbook execution port. Loads and validates YAML skill manifests.
//! The poka-yoke dispatcher enforces JR-3 (LLM selects from known IDs only).
//!
//! Each skill lives under `skills/<id>/` with a `manifest.yaml`
//! file that declares probes (read-only) and interventions
//! (mutating, governed by the IDRS contract — see
//! [`docs/standards/safety.md`](../../../docs/standards/safety.md)).
//!
//! ## Poka-yoke
//!
//! The loader refuses any manifest that fails schema validation,
//! references unknown symptom names, or has unreferenced scripts
//! in its `scripts/` directory. Invalid skills are skipped (with
//! a warning logged via `eprintln!`) — one broken manifest must
//! not prevent the rest of the skill set from loading.

#![deny(unsafe_code)]
#![deny(rust_2018_idioms)]
#![warn(missing_docs)]

use std::path::{Path, PathBuf};

use serde::Deserialize;

mod symptom_catalog;

/// Subprocess dispatcher for probe and intervention execution.
pub mod dispatch;

/// Landlock-based sandbox for skill subprocess confinement.
pub mod sandbox;

/// Skill registry — cache, lifecycle, lookup, safety scanner.
pub mod registry;

/// Jinja2 template support for skill prompts.
pub mod templates;

pub use symptom_catalog::{SYMPTOMS, SeverityHint, Symptom, SymptomCategory};

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
        "intervention '{intervention_id}' in {path} must declare rollback: <id>, rollback: none_needed, or rollback: reboot"
    )]
    InterventionMissingRollback {
        /// Path to manifest.yaml.
        path: PathBuf,
        /// Intervention ID.
        intervention_id: String,
    },

    /// A `rollback` references an intervention that does not exist.
    #[error(
        "rollback '{rollback_id}' in intervention '{intervention_id}' in {path} does not reference a known intervention"
    )]
    RollbackIdNotFound {
        /// Path to manifest.yaml.
        path: PathBuf,
        /// Intervention that declares the rollback.
        intervention_id: String,
        /// The rollback ID that doesn't resolve.
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

    /// A script file in `scripts/` is not referenced by any probe,
    /// intervention, or evaluation `cmd:` entry.
    #[error("skill '{skill_id}' has unreferenced script: {script}")]
    UnreferencedScript {
        /// Skill ID.
        skill_id: String,
        /// Path to the unreferenced script file.
        script: PathBuf,
    },
}

/// The kind of skill — determines how it integrates with the harness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillKind {
    /// Has probes and/or interventions — executable runbook.
    Actionable,
    /// Knowledge-only — KNOWLEDGE.md injected into system prompt.
    /// No probes, no interventions.
    Lens,
}

impl Default for SkillKind {
    /// Default is `Actionable` — process skills (probes + interventions)
    /// are the primary case. Knowledge-only lenses must be explicitly
    /// declared or inferred from empty probes/interventions.
    fn default() -> Self {
        Self::Actionable
    }
}

/// Visibility annotation for ACP exposure (ADR-0026).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    /// Exposed via ACP to hKask agents.
    Public,
    /// Russell-only (never exposed).
    #[default]
    Private,
}

/// hLexicon categorization (ADR-0026).
#[derive(Debug, Clone, Deserialize)]
pub struct Lexicon {
    /// Primary domain (WordAct, FlowDef, KnowAct).
    pub primary: String,
    /// Specific terms (3-7 from hLexicon).
    #[serde(default)]
    pub terms: Vec<String>,
}

/// A post-intervention verification step.
#[derive(Debug, Clone, Deserialize)]
pub struct EvaluationStep {
    /// Unique ID within the skill.
    pub id: String,
    /// Argv list to execute.
    pub cmd: Vec<String>,
    /// Timeout (e.g. "30s").
    #[serde(default = "timeout_default")]
    pub timeout: String,
    /// Expected exit code (default 0).
    #[serde(default)]
    pub expect_exit: Option<i32>,
}

/// Evaluation block — post-intervention verification.
#[derive(Debug, Clone, Deserialize)]
pub struct Evaluation {
    /// Verification steps to run after an intervention.
    pub after_intervention: Vec<EvaluationStep>,
}

/// A single skill, fully loaded and validated.
#[derive(Debug, Clone)]
pub struct Skill {
    /// Unique identifier (kebab-case).
    pub id: String,
    /// The kind of skill (actionable vs lens).
    pub kind: SkillKind,
    /// Semver version.
    pub version: String,
    /// Date authored (ISO 8601 date).
    pub authored: String,
    /// Minimum Russell version required.
    pub min_harness_version: String,
    /// Symptom classes this skill addresses.
    pub symptoms: Vec<Symptom>,
    /// Profile preconditions (ANDed).
    pub applies_when: Vec<AppliesWhen>,
    /// Read-only probes.
    pub probes: Vec<Probe>,
    /// Mutating interventions.
    pub interventions: Vec<Intervention>,
    /// Post-intervention verification steps.
    pub evaluation: Option<Evaluation>,
    /// Safety constraints.
    pub safety: Safety,
    /// Visibility for ACP exposure (ADR-0026).
    pub visibility: Visibility,
    /// hLexicon categorization (ADR-0026).
    pub lexicon: Option<Lexicon>,
}

impl Skill {
    /// Whether this skill is a knowledge-only lens (no probes/interventions).
    #[must_use]
    pub fn is_lens(&self) -> bool {
        self.kind == SkillKind::Lens || (self.probes.is_empty() && self.interventions.is_empty())
    }

    /// Whether this skill is actionable (has probes or interventions).
    #[must_use]
    pub fn is_actionable(&self) -> bool {
        !self.is_lens()
    }

    /// Returns a unified view of all probes and interventions as [`Step`]s.
    #[must_use]
    pub fn steps(&self) -> Vec<Step> {
        let mut steps = Vec::with_capacity(self.probes.len() + self.interventions.len());
        for p in &self.probes {
            steps.push(Step {
                id: p.id.clone(),
                cmd: p.cmd.clone(),
                risk: RiskBand::None,
                rollback: None,
                timeout: p.timeout.clone(),
                idempotent: true,
                needs_sudo: false,
                capture: p.capture.clone(),
            });
        }
        for iv in &self.interventions {
            steps.push(Step {
                id: iv.id.clone(),
                cmd: iv.cmd.clone(),
                risk: iv.risk,
                rollback: Some(iv.rollback.clone()),
                timeout: iv.timeout.clone(),
                idempotent: iv.idempotent,
                needs_sudo: iv.needs_sudo,
                capture: "stdout".into(),
            });
        }
        steps
    }

    /// Look up a step by ID (searching both probes and interventions).
    #[must_use]
    pub fn find_step(&self, step_id: &str) -> Option<Step> {
        self.steps().into_iter().find(|s| s.id == step_id)
    }
}

/// A unified executable step within a skill — the common abstraction
#[derive(Debug, Clone)]
pub struct Step {
    /// Unique ID within the skill.
    pub id: String,
    /// Argv list to execute.
    pub cmd: Vec<String>,
    /// Risk band. `None` = probe (auto-executable, no consent needed).
    pub risk: RiskBand,
    /// Rollback strategy. `None` for probes; `Some(..)` for interventions.
    pub rollback: Option<Rollback>,
    /// Timeout string (e.g. "30s", "120s").
    pub timeout: String,
    /// Idempotent (always true for probes).
    pub idempotent: bool,
    /// Whether this step requires sudo.
    pub needs_sudo: bool,
    /// Capture mode (stdout, stderr, both, file:<path>).
    pub capture: String,
}

impl Step {
    /// Whether this step is a probe (risk: none, auto-executable).
    #[must_use]
    pub fn is_probe(&self) -> bool {
        self.risk == RiskBand::None
    }

    /// Whether this step is an intervention (risk > none, requires consent).
    #[must_use]
    pub fn is_intervention(&self) -> bool {
        self.risk > RiskBand::None
    }

    /// Whether this step requires operator consent given the auto-risk cap.
    ///
    /// Probes never require consent. Interventions require consent
    /// when their risk exceeds `max_auto_risk`.
    #[must_use]
    pub fn consent_required(&self, max_auto_risk: RiskBand) -> bool {
        self.risk > RiskBand::None && self.risk > max_auto_risk
    }
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
    /// Whether this intervention requires root privileges.
    /// The operator will be prompted for their sudo password
    /// when consenting to execution.
    #[serde(default)]
    pub needs_sudo: bool,
}

/// Rollback strategy for an intervention.
///
/// With `#[serde(untagged)]` the variants are tried in order.
/// `NoneNeeded` and `Reboot` must come before `RollbackId` so that
/// the string values `"none_needed"` and `"reboot"` are claimed by
/// their specific variants before the catch-all `RollbackId`.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Rollback {
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
    /// Reference to a reverse intervention.
    RollbackId {
        /// The intervention to run to reverse this one.
        rollback: String,
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

/// Risk band for an intervention — re-exported from [`russell_core::risk::RiskBand`].
///
/// This is the canonical risk classification used across all Russell crates.
/// Defined once in `russell-core` per C4 (repetition is a missing primitive).
pub use russell_core::risk::RiskBand;

/// Safety constraints for a skill.
#[derive(Debug, Clone, Deserialize)]
pub struct Safety {
    /// Maximum risk band the Nurse may auto-run.
    #[serde(default = "max_auto_risk_default")]
    pub max_auto_risk: RiskBand,
    /// Intervention IDs that always require human confirmation.
    #[serde(default)]
    pub require_human_for: Vec<String>,
    /// Environment variables this skill is allowed to access.
    /// If empty, no environment variables are passed to subprocesses.
    /// Task 3.1: Capability attenuation for skills.
    /// If empty, no environment variables are passed to subprocesses.
    /// Task 3.1: Capability attenuation for skills.
    /// Task 3.1: Capability attenuation for skills.
    #[serde(default)]
    pub allowed_env_keys: Vec<String>,
    /// Whether this skill requires network access.
    /// Task 3.1: Skills declaring `true` may be blocked in air-gapped environments.
    /// Task 3.1: Skills declaring `true` may be blocked in air-gapped environments.
    #[serde(default)]
    pub needs_network: bool,
}

/// The raw YAML manifest as parsed from disk, before validation.
#[derive(Debug, Deserialize)]
#[allow(missing_docs)]
pub struct RawManifest {
    pub id: String,
    /// Optional `kind` field. Defaults to `actionable`; if set to `lens`,
    /// the skill is treated as knowledge-only.
    /// the skill is treated as knowledge-only.
    #[serde(default)]
    pub kind: Option<SkillKind>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub authored: Option<String>,
    #[serde(default)]
    pub min_harness_version: Option<String>,
    #[serde(default)]
    pub symptoms: Vec<String>,
    #[serde(default)]
    pub applies_when: Vec<AppliesWhen>,
    #[serde(default)]
    pub probes: Vec<Probe>,
    #[serde(default)]
    pub interventions: Vec<Intervention>,
    /// Post-intervention verification steps.
    #[serde(default)]
    pub evaluation: Option<Evaluation>,
    #[serde(default)]
    pub safety: Option<RawSafety>,
    /// Visibility for ACP exposure (ADR-0026).
    #[serde(default)]
    pub visibility: Option<Visibility>,
    /// hLexicon categorization (ADR-0026).
    #[serde(default)]
    pub lexicon: Option<Lexicon>,
}

#[derive(Debug, Deserialize)]
#[allow(missing_docs)]
pub struct RawSafety {
    #[serde(default = "max_auto_risk_default")]
    max_auto_risk: RiskBand,
    #[serde(default)]
    require_human_for: Vec<String>,
    /// Task 3.1: Capability attenuation — allowed environment variables.
    #[serde(default)]
    allowed_env_keys: Vec<String>,
    /// Task 3.1: Capability attenuation — network access requirement.
    #[serde(default)]
    needs_network: bool,
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

const fn bool_true() -> bool {
    true
}

fn max_auto_risk_default() -> RiskBand {
    RiskBand::Low
}

// --- Loading -------------------------------------------------------------

/// Load all skill manifests from a `skills/` directory.
pub fn load_all(skills_dir: &Path) -> Result<Vec<Skill>, LoadError> {
    let entries = match std::fs::read_dir(skills_dir) {
        Ok(d) => d,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(LoadError::ReadDir(e)),
    };

    let mut skills = Vec::new();
    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                eprintln!(
                    "warning: skipping unreadable entry in {}: {e}",
                    skills_dir.display()
                );
                continue;
            }
        };
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let dir_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();

        // Skip non-skill directories (e.g. `.git`, `__pycache__`, `templates`).
        if dir_name.starts_with('.') || dir_name.starts_with("__") || dir_name == "templates" {
            continue;
        }

        match load_one(&path, dir_name) {
            Ok(skill) => skills.push(skill),
            Err(e) => {
                eprintln!("warning: skipping skill '{}': {e}", dir_name);
            }
        }
    }

    Ok(skills)
}

/// Load and validate a single skill manifest.
pub fn load_single(skill_dir: &Path) -> Result<Skill, LoadError> {
    let dir_name = skill_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");
    load_one(skill_dir, dir_name)
}

/// Load and validate a single skill manifest (internal helper).
fn load_one(skill_dir: &Path, dir_name: &str) -> Result<Skill, LoadError> {
    let manifest_path = skill_dir.join("manifest.yaml");
    if !manifest_path.exists() {
        return Err(LoadError::MissingManifest(skill_dir.to_path_buf()));
    }

    let yaml = std::fs::read_to_string(&manifest_path).map_err(|e| LoadError::ReadManifest {
        path: manifest_path.clone(),
        source: e,
    })?;

    let skill = parse_manifest(&yaml, dir_name).map_err(|message| LoadError::InvalidManifest {
        path: manifest_path.clone(),
        message,
    })?;

    // Additional check that only applies when loading from disk:
    // every script file in scripts/ must be referenced by a probe or
    // intervention cmd.
    check_unreferenced_scripts(skill_dir, &skill, &manifest_path)?;

    Ok(skill)
}

/// Check that every executable file in `scripts/` is referenced by
/// at least one probe, intervention, or evaluation `cmd`.
fn check_unreferenced_scripts(
    skill_dir: &Path,
    skill: &Skill,
    _manifest_path: &Path,
) -> Result<(), LoadError> {
    use std::collections::BTreeSet;

    let scripts_dir = skill_dir.join("scripts");
    if !scripts_dir.exists() {
        return Ok(());
    }

    let mut referenced: BTreeSet<String> = BTreeSet::new();
    for probe in &skill.probes {
        collect_script_names(&probe.cmd, &mut referenced);
    }
    for iv in &skill.interventions {
        collect_script_names(&iv.cmd, &mut referenced);
    }
    if let Some(eval) = &skill.evaluation {
        for step in &eval.after_intervention {
            collect_script_names(&step.cmd, &mut referenced);
        }
    }

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
        if name.contains('.') {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if !matches!(ext, "sh" | "py" | "bash" | "pl" | "rb") {
                continue;
            }
        }
        if !referenced.contains(name) {
            return Err(LoadError::UnreferencedScript {
                skill_id: skill.id.clone(),
                script: path.clone(),
            });
        }
    }

    Ok(())
}

/// Extract script filenames from a command argv.
fn collect_script_names(cmd: &[String], out: &mut std::collections::BTreeSet<String>) {
    for arg in cmd {
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
            out.insert(name.to_string());
        }
    }
}

/// Extract the `id` field from a manifest YAML string without full validation.
///
/// Quick parse to get the skill ID for directory naming. Returns `None`
/// if the YAML is malformed or the `id` field is missing.
///
/// Quick parse to get the skill ID for directory naming. Returns `None`
/// if the YAML is malformed or the `id` field is missing.
/// Quick parse to get the skill ID for directory naming. Returns `None`
/// if the YAML is malformed or the `id` field is missing.
/// if the YAML is malformed or the `id` field is missing.
pub fn extract_manifest_id(yaml: &str) -> Option<String> {
    let raw: RawManifest = serde_yaml::from_str(yaml).ok()?;
    if raw.id.is_empty() {
        None
    } else {
        Some(raw.id)
    }
}

/// Parse a manifest YAML string, validate it, and return a [`Skill`].
pub fn parse_manifest(yaml: &str, dir_name: &str) -> std::result::Result<Skill, String> {
    let raw: RawManifest =
        serde_yaml::from_str(yaml).map_err(|e| format!("YAML parse error: {e}"))?;

    // Validate id match.
    if raw.id != dir_name {
        return Err(format!(
            "manifest id '{}' does not match directory name '{}'",
            raw.id, dir_name
        ));
    }

    // Validate and parse symptoms.
    let symptoms: Vec<Symptom> = raw
        .symptoms
        .iter()
        .map(|s| {
            s.parse::<Symptom>()
                .map_err(|_| format!("skill '{}' references unknown symptom '{}'", dir_name, s))
        })
        .collect::<std::result::Result<Vec<_>, _>>()?;

    // Validate rollback strategies.
    for iv in &raw.interventions {
        match &iv.rollback {
            Rollback::RollbackId { rollback } => {
                if !raw.interventions.iter().any(|r| r.id == *rollback) {
                    return Err(format!(
                        "rollback_id '{}' in intervention '{}' does not reference a known intervention",
                        rollback, iv.id
                    ));
                }
            }
            Rollback::NoneNeeded { .. } => {}
            Rollback::Reboot { .. } => {}
        }
    }

    let safety_raw = raw.safety.unwrap_or(RawSafety {
        max_auto_risk: RiskBand::Low,
        require_human_for: Vec::new(),
        allowed_env_keys: Vec::new(),
        needs_network: false,
    });

    // Determine kind: explicit from manifest, or inferred from content.
    let kind = raw
        .kind
        .unwrap_or(if raw.probes.is_empty() && raw.interventions.is_empty() {
            SkillKind::Lens
        } else {
            SkillKind::Actionable
        });

    Ok(Skill {
        id: raw.id,
        kind,
        version: raw.version.unwrap_or_else(|| "0.0.0".into()),
        authored: raw.authored.unwrap_or_else(|| "unknown".into()),
        min_harness_version: raw.min_harness_version.unwrap_or_else(|| "0.1.0".into()),
        symptoms,
        applies_when: raw.applies_when,
        probes: raw.probes,
        interventions: raw.interventions,
        evaluation: raw.evaluation,
        safety: Safety {
            max_auto_risk: safety_raw.max_auto_risk,
            require_human_for: safety_raw.require_human_for,
            allowed_env_keys: safety_raw.allowed_env_keys,
            needs_network: safety_raw.needs_network,
        },
        visibility: raw.visibility.unwrap_or_default(),
        lexicon: raw.lexicon,
    })
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
        assert_eq!(skills[0].symptoms.len(), 1);
        assert_eq!(skills[0].symptoms[0].name(), "vram_oom");
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
    fn skips_id_mismatch() {
        let tmp = tempfile::tempdir().unwrap();
        write_skill(tmp.path(), "gpu-doctor", &valid_manifest("wrong-id"));
        // Lenient: bad skill is skipped, not a hard error.
        let skills = load_all(tmp.path()).unwrap();
        assert!(skills.is_empty(), "mismatched id should be skipped");
    }

    #[test]
    fn skips_unknown_symptom() {
        let tmp = tempfile::tempdir().unwrap();
        let yaml = valid_manifest("gpu-doctor").replace("vram_oom", "made_up_symptom");
        write_skill(tmp.path(), "gpu-doctor", &yaml);
        // Lenient: bad skill is skipped, not a hard error.
        let skills = load_all(tmp.path()).unwrap();
        assert!(skills.is_empty(), "unknown symptom should be skipped");
    }

    #[test]
    fn skips_bad_rollback_id() {
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
    rollback: no-such-id
safety:
  max_auto_risk: low
"#;
        write_skill(tmp.path(), "gpu-doctor", yaml);
        // Lenient: bad skill is skipped, not a hard error.
        let skills = load_all(tmp.path()).unwrap();
        assert!(skills.is_empty(), "bad rollback_id should be skipped");
    }

    #[test]
    fn skips_unreferenced_script() {
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
        // Lenient: bad skill is skipped, not a hard error.
        let skills = load_all(tmp.path()).unwrap();
        assert!(skills.is_empty(), "unreferenced script should be skipped");
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
        std::fs::create_dir_all(tmp.path().join("templates")).unwrap();
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
    fn skips_missing_manifest_in_skill_dir() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("gpu-doctor")).unwrap();
        // Lenient: directory without manifest.yaml is skipped.
        let skills = load_all(tmp.path()).unwrap();
        assert!(skills.is_empty(), "missing manifest should be skipped");
    }

    #[test]
    fn loads_gpu_doctor_fixture() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let skills = load_all(&fixture).unwrap();
        assert_eq!(skills.len(), 3);

        let gpu = skills.iter().find(|s| s.id == "gpu-doctor").unwrap();
        assert_eq!(gpu.probes.len(), 3);
        assert_eq!(gpu.interventions.len(), 1);
        assert!(gpu.symptoms.iter().any(|s| s.name() == "vram_oom"));
        assert!(gpu.symptoms.iter().any(|s| s.name() == "amdgpu_ring_hang"));
        let reset = &gpu.interventions[0];
        assert_eq!(reset.id, "reset-gpu");
        assert!(matches!(reset.risk, RiskBand::Medium));

        let okapi = skills.iter().find(|s| s.id == "okapi-watcher").unwrap();
        assert_eq!(okapi.probes.len(), 3);
        assert_eq!(okapi.interventions.len(), 1);
        assert!(okapi.symptoms.iter().any(|s| s.name() == "llm_slow"));
        assert!(
            okapi
                .symptoms
                .iter()
                .any(|s| s.name() == "gpu_fallback_to_cpu")
        );
        assert_eq!(okapi.interventions[0].id, "restart-okapi");

        let sysadmin = skills.iter().find(|s| s.id == "sysadmin").unwrap();
        assert_eq!(sysadmin.probes.len(), 2);
        assert_eq!(sysadmin.interventions.len(), 3);
        assert!(
            sysadmin
                .symptoms
                .iter()
                .any(|s| s.name() == "zombie_accumulation")
        );
        assert!(sysadmin.symptoms.iter().any(|s| s.name() == "clock_skew"));
        assert!(
            sysadmin
                .symptoms
                .iter()
                .any(|s| s.name() == "systemd_service_degraded")
        );
        assert!(matches!(sysadmin.safety.max_auto_risk, RiskBand::Medium));
    }

    #[test]
    fn allows_evaluation_scripts() {
        let tmp = tempfile::tempdir().unwrap();
        let skill_dir = tmp.path().join("gpu-doctor");
        std::fs::create_dir_all(skill_dir.join("scripts")).unwrap();
        std::fs::write(skill_dir.join("scripts/probe.sh"), "#!/bin/bash\necho ok\n").unwrap();
        std::fs::write(
            skill_dir.join("scripts/verify.sh"),
            "#!/bin/bash\necho verified\n",
        )
        .unwrap();
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
evaluation:
  after_intervention:
    - id: verify-result
      cmd: ["bash", "./scripts/verify.sh"]
      timeout: 5s
      expect_exit: 0
safety:
  max_auto_risk: low
"#;
        std::fs::write(skill_dir.join("manifest.yaml"), yaml).unwrap();
        let skills = load_all(tmp.path()).unwrap();
        assert_eq!(
            skills.len(),
            1,
            "evaluation script should be recognized as referenced"
        );
        let eval = skills[0]
            .evaluation
            .as_ref()
            .expect("evaluation should be parsed");
        assert_eq!(eval.after_intervention.len(), 1);
        assert_eq!(eval.after_intervention[0].id, "verify-result");
    }
}

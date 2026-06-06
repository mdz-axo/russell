// SPDX-License-Identifier: MIT OR Apache-2.0
//! Known symptom catalog for Russell skills.
//!
//! The symptom catalog serves as the controlled vocabulary of addressable
//! conditions. Skills reference these names in their `symptoms:` field.
//!
//! ## Two-tier loading
//!
//! 1. **Compiled-in seed** — the `SYMPTOMS` constant (via `include_str!`)
//!    ensures the binary always has a baseline catalog even if the data
//!    file is missing.
//! 2. **Runtime data file** — `data/symptoms.yaml` can be edited without
//!    recompilation. Use [`load_symptoms_from_file`] to load at runtime.
//!
//! Extending this catalog is governed by the skill lifecycle ADR
//! ([ADR-0024](../../docs/adr/0024-skill-registry-workshop-lifecycle.md)).
//! A manifest's `symptoms:` entry not in this set causes a
//! [`super::LoadError::UnknownSymptom`] at load time (poka-yoke).

use std::path::Path;

/// The compiled-in symptom catalog YAML (fallback seed).
/// Retained for future runtime YAML loading; currently unused because
/// the static `SYMPTOMS` array covers all known symptoms.
#[allow(dead_code)]
const SYMPTOMS_YAML: &str = include_str!("../data/symptoms.yaml");

/// Symptom category — groups related symptoms for filtering and display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SymptomCategory {
    /// Hardware / GPU symptoms.
    Hardware,
    /// System-level symptoms (OOM, swap, load).
    System,
    /// Ubuntu-specific distribution symptoms.
    Ubuntu,
    /// Cybernetic / feedback-loop symptoms.
    Cybernetic,
    /// Semantic / provenance symptoms.
    Semantic,
    /// Sysadmin / host maintenance symptoms.
    Sysadmin,
    /// Web search / external knowledge symptoms.
    WebSearch,
    /// Skill discovery / management symptoms.
    SkillDiscovery,
    /// Scenario testing / load probe symptoms.
    ScenarioTesting,
}

impl std::fmt::Display for SymptomCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Hardware => "hardware",
            Self::System => "system",
            Self::Ubuntu => "ubuntu",
            Self::Cybernetic => "cybernetic",
            Self::Semantic => "semantic",
            Self::Sysadmin => "sysadmin",
            Self::WebSearch => "web_search",
            Self::SkillDiscovery => "skill_discovery",
            Self::ScenarioTesting => "scenario_testing",
        };
        f.write_str(s)
    }
}

/// Severity hint for a symptom — guides Jack's triage prioritization.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, Default,
)]
#[serde(rename_all = "snake_case")]
pub enum SeverityHint {
    /// Informational; may warrant monitoring.
    Low,
    /// Likely requires attention within the next cadence.
    #[default]
    Medium,
    /// Statistically significant; action recommended soon.
    High,
    /// Known-dangerous; action required now.
    Critical,
}

impl std::fmt::Display for SeverityHint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        };
        f.write_str(s)
    }
}

/// A validated symptom with metadata.
///
/// Created via [`Symptom::from_str`] which validates against the catalog.
/// Unknown symptom names produce an error (poka-yoke: typos are caught
/// at load time, not at query time).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Symptom {
    name: String,
    category: SymptomCategory,
    severity_hint: SeverityHint,
}

impl Symptom {
    /// The symptom name (e.g. `vram_oom`).
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The category this symptom belongs to.
    #[must_use]
    pub fn category(&self) -> SymptomCategory {
        self.category
    }

    /// The severity hint for triage prioritization.
    #[must_use]
    pub fn severity_hint(&self) -> SeverityHint {
        self.severity_hint
    }
}

impl std::fmt::Display for Symptom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name)
    }
}

impl From<Symptom> for String {
    fn from(s: Symptom) -> String {
        s.name
    }
}

impl std::str::FromStr for Symptom {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        lookup_symptom(s).ok_or_else(|| format!("unknown symptom: {s}"))
    }
}

impl serde::Serialize for Symptom {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.name)
    }
}

impl<'de> serde::Deserialize<'de> for Symptom {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        s.parse::<Symptom>().map_err(serde::de::Error::custom)
    }
}

struct SymptomDef {
    name: &'static str,
    category: SymptomCategory,
    severity_hint: SeverityHint,
}

const SYMPTOM_DEFS: &[SymptomDef] = &[
    // Hardware / GPU
    SymptomDef {
        name: "amdgpu_ring_hang",
        category: SymptomCategory::Hardware,
        severity_hint: SeverityHint::Critical,
    },
    SymptomDef {
        name: "amdgpu_reset",
        category: SymptomCategory::Hardware,
        severity_hint: SeverityHint::High,
    },
    SymptomDef {
        name: "vram_oom",
        category: SymptomCategory::Hardware,
        severity_hint: SeverityHint::Critical,
    },
    SymptomDef {
        name: "gpu_temp_high",
        category: SymptomCategory::Hardware,
        severity_hint: SeverityHint::High,
    },
    SymptomDef {
        name: "gpu_fallback_to_cpu",
        category: SymptomCategory::Hardware,
        severity_hint: SeverityHint::High,
    },
    SymptomDef {
        name: "rocm_unreachable",
        category: SymptomCategory::Hardware,
        severity_hint: SeverityHint::Critical,
    },
    SymptomDef {
        name: "nvme_media_errors",
        category: SymptomCategory::Hardware,
        severity_hint: SeverityHint::Critical,
    },
    SymptomDef {
        name: "gpu_not_responding",
        category: SymptomCategory::Hardware,
        severity_hint: SeverityHint::High,
    },
    // System
    SymptomDef {
        name: "oom_killer_active",
        category: SymptomCategory::System,
        severity_hint: SeverityHint::Critical,
    },
    SymptomDef {
        name: "swap_pressure",
        category: SymptomCategory::System,
        severity_hint: SeverityHint::High,
    },
    SymptomDef {
        name: "loadavg_high",
        category: SymptomCategory::System,
        severity_hint: SeverityHint::Medium,
    },
    SymptomDef {
        name: "llm_slow",
        category: SymptomCategory::System,
        severity_hint: SeverityHint::Medium,
    },
    SymptomDef {
        name: "resource_exhaustion",
        category: SymptomCategory::System,
        severity_hint: SeverityHint::High,
    },
    SymptomDef {
        name: "memory_pressure",
        category: SymptomCategory::System,
        severity_hint: SeverityHint::High,
    },
    SymptomDef {
        name: "disk_pressure",
        category: SymptomCategory::System,
        severity_hint: SeverityHint::Medium,
    },
    SymptomDef {
        name: "boot_partition_full",
        category: SymptomCategory::System,
        severity_hint: SeverityHint::High,
    },
    SymptomDef {
        name: "kernel_param_misconfigured",
        category: SymptomCategory::System,
        severity_hint: SeverityHint::High,
    },
    SymptomDef {
        name: "llm_not_responding",
        category: SymptomCategory::System,
        severity_hint: SeverityHint::High,
    },
    SymptomDef {
        name: "model_loading_failure",
        category: SymptomCategory::System,
        severity_hint: SeverityHint::High,
    },
    // Ubuntu-specific
    SymptomDef {
        name: "systemd_service_degraded",
        category: SymptomCategory::Ubuntu,
        severity_hint: SeverityHint::Medium,
    },
    SymptomDef {
        name: "apt_stale",
        category: SymptomCategory::Ubuntu,
        severity_hint: SeverityHint::Low,
    },
    SymptomDef {
        name: "snap_autorefresh_stall",
        category: SymptomCategory::Ubuntu,
        severity_hint: SeverityHint::Low,
    },
    SymptomDef {
        name: "btrfs_fragmented",
        category: SymptomCategory::Ubuntu,
        severity_hint: SeverityHint::Medium,
    },
    SymptomDef {
        name: "zfs_scrub_overdue",
        category: SymptomCategory::Ubuntu,
        severity_hint: SeverityHint::Medium,
    },
    SymptomDef {
        name: "unattended_upgrades_failed",
        category: SymptomCategory::Ubuntu,
        severity_hint: SeverityHint::Medium,
    },
    SymptomDef {
        name: "kernel_livepatch_stale",
        category: SymptomCategory::Ubuntu,
        severity_hint: SeverityHint::Low,
    },
    SymptomDef {
        name: "fwupd_outdated",
        category: SymptomCategory::Ubuntu,
        severity_hint: SeverityHint::Low,
    },
    SymptomDef {
        name: "netplan_misconfigured",
        category: SymptomCategory::Ubuntu,
        severity_hint: SeverityHint::High,
    },
    SymptomDef {
        name: "apparmor_denial_spike",
        category: SymptomCategory::Ubuntu,
        severity_hint: SeverityHint::Medium,
    },
    SymptomDef {
        name: "journald_corruption",
        category: SymptomCategory::Ubuntu,
        severity_hint: SeverityHint::High,
    },
    SymptomDef {
        name: "tmp_mount_nosuid_missing",
        category: SymptomCategory::Ubuntu,
        severity_hint: SeverityHint::Low,
    },
    SymptomDef {
        name: "package_out_of_date",
        category: SymptomCategory::Ubuntu,
        severity_hint: SeverityHint::Low,
    },
    SymptomDef {
        name: "system_maintenance",
        category: SymptomCategory::Ubuntu,
        severity_hint: SeverityHint::Low,
    },
    SymptomDef {
        name: "pending_reboot",
        category: SymptomCategory::Ubuntu,
        severity_hint: SeverityHint::Low,
    },
    SymptomDef {
        name: "kernel_old_installed",
        category: SymptomCategory::Ubuntu,
        severity_hint: SeverityHint::Low,
    },
    SymptomDef {
        name: "autoremove_needed",
        category: SymptomCategory::Ubuntu,
        severity_hint: SeverityHint::Low,
    },
    SymptomDef {
        name: "held_packages",
        category: SymptomCategory::Ubuntu,
        severity_hint: SeverityHint::Low,
    },
    SymptomDef {
        name: "snap_stale",
        category: SymptomCategory::Ubuntu,
        severity_hint: SeverityHint::Low,
    },
    SymptomDef {
        name: "flatpak_stale",
        category: SymptomCategory::Ubuntu,
        severity_hint: SeverityHint::Low,
    },
    SymptomDef {
        name: "service_degraded",
        category: SymptomCategory::Ubuntu,
        severity_hint: SeverityHint::Medium,
    },
    // Cybernetic
    SymptomDef {
        name: "broken_feedback_loop",
        category: SymptomCategory::Cybernetic,
        severity_hint: SeverityHint::High,
    },
    SymptomDef {
        name: "alert_fatigue",
        category: SymptomCategory::Cybernetic,
        severity_hint: SeverityHint::Medium,
    },
    SymptomDef {
        name: "variety_deficit",
        category: SymptomCategory::Cybernetic,
        severity_hint: SeverityHint::Low,
    },
    SymptomDef {
        name: "model_reality_divergence",
        category: SymptomCategory::Cybernetic,
        severity_hint: SeverityHint::High,
    },
    SymptomDef {
        name: "observer_coupling",
        category: SymptomCategory::Cybernetic,
        severity_hint: SeverityHint::Medium,
    },
    SymptomDef {
        name: "missing_double_loop",
        category: SymptomCategory::Cybernetic,
        severity_hint: SeverityHint::Medium,
    },
    SymptomDef {
        name: "homeostatic_drift",
        category: SymptomCategory::Cybernetic,
        severity_hint: SeverityHint::Medium,
    },
    SymptomDef {
        name: "coordination_oscillation",
        category: SymptomCategory::Cybernetic,
        severity_hint: SeverityHint::Medium,
    },
    SymptomDef {
        name: "context_saturation",
        category: SymptomCategory::Cybernetic,
        severity_hint: SeverityHint::Low,
    },
    SymptomDef {
        name: "monitoring_independence_failure",
        category: SymptomCategory::Cybernetic,
        severity_hint: SeverityHint::High,
    },
    // Semantic
    SymptomDef {
        name: "provenance_gap",
        category: SymptomCategory::Semantic,
        severity_hint: SeverityHint::Medium,
    },
    SymptomDef {
        name: "confidence_decay",
        category: SymptomCategory::Semantic,
        severity_hint: SeverityHint::Medium,
    },
    SymptomDef {
        name: "classification_ambiguity",
        category: SymptomCategory::Semantic,
        severity_hint: SeverityHint::Low,
    },
    SymptomDef {
        name: "constraint_violation",
        category: SymptomCategory::Semantic,
        severity_hint: SeverityHint::High,
    },
    SymptomDef {
        name: "discourse_incoherence",
        category: SymptomCategory::Semantic,
        severity_hint: SeverityHint::Medium,
    },
    SymptomDef {
        name: "implicit_expectation_mismatch",
        category: SymptomCategory::Semantic,
        severity_hint: SeverityHint::Medium,
    },
    SymptomDef {
        name: "reference_implementation_drift",
        category: SymptomCategory::Semantic,
        severity_hint: SeverityHint::Medium,
    },
    SymptomDef {
        name: "operator_requests_interrogation",
        category: SymptomCategory::Semantic,
        severity_hint: SeverityHint::Low,
    },
    SymptomDef {
        name: "knowledge_assessment_needed",
        category: SymptomCategory::Semantic,
        severity_hint: SeverityHint::Low,
    },
    SymptomDef {
        name: "learning_goal_identified",
        category: SymptomCategory::Semantic,
        severity_hint: SeverityHint::Low,
    },
    SymptomDef {
        name: "script_safety_concern",
        category: SymptomCategory::Semantic,
        severity_hint: SeverityHint::High,
    },
    SymptomDef {
        name: "idrs_violation",
        category: SymptomCategory::Semantic,
        severity_hint: SeverityHint::High,
    },
    SymptomDef {
        name: "script_style_issue",
        category: SymptomCategory::Semantic,
        severity_hint: SeverityHint::Low,
    },
    SymptomDef {
        name: "script_content_needed",
        category: SymptomCategory::Semantic,
        severity_hint: SeverityHint::Low,
    },
    SymptomDef {
        name: "script_inventory_needed",
        category: SymptomCategory::Semantic,
        severity_hint: SeverityHint::Low,
    },
    // Sysadmin
    SymptomDef {
        name: "zombie_accumulation",
        category: SymptomCategory::Sysadmin,
        severity_hint: SeverityHint::Medium,
    },
    SymptomDef {
        name: "clock_skew",
        category: SymptomCategory::Sysadmin,
        severity_hint: SeverityHint::High,
    },
    SymptomDef {
        name: "systemd_timer_misfire",
        category: SymptomCategory::Sysadmin,
        severity_hint: SeverityHint::Medium,
    },
    SymptomDef {
        name: "journal_bloat",
        category: SymptomCategory::Sysadmin,
        severity_hint: SeverityHint::Low,
    },
    SymptomDef {
        name: "process_table_bloat",
        category: SymptomCategory::Sysadmin,
        severity_hint: SeverityHint::Medium,
    },
    SymptomDef {
        name: "swap_retention",
        category: SymptomCategory::Sysadmin,
        severity_hint: SeverityHint::Medium,
    },
    SymptomDef {
        name: "coredump_accumulation",
        category: SymptomCategory::Sysadmin,
        severity_hint: SeverityHint::Low,
    },
    SymptomDef {
        name: "stale_mount",
        category: SymptomCategory::Sysadmin,
        severity_hint: SeverityHint::Medium,
    },
    SymptomDef {
        name: "file_needs_creation",
        category: SymptomCategory::Sysadmin,
        severity_hint: SeverityHint::Low,
    },
    SymptomDef {
        name: "file_needs_update",
        category: SymptomCategory::Sysadmin,
        severity_hint: SeverityHint::Low,
    },
    SymptomDef {
        name: "file_needs_deletion",
        category: SymptomCategory::Sysadmin,
        severity_hint: SeverityHint::Medium,
    },
    SymptomDef {
        name: "directory_needs_creation",
        category: SymptomCategory::Sysadmin,
        severity_hint: SeverityHint::Low,
    },
    SymptomDef {
        name: "config_drift",
        category: SymptomCategory::Sysadmin,
        severity_hint: SeverityHint::Medium,
    },
    SymptomDef {
        name: "docker_daemon_down",
        category: SymptomCategory::Sysadmin,
        severity_hint: SeverityHint::High,
    },
    SymptomDef {
        name: "ollama_daemon_down",
        category: SymptomCategory::Sysadmin,
        severity_hint: SeverityHint::High,
    },
    SymptomDef {
        name: "cache_bloat",
        category: SymptomCategory::Sysadmin,
        severity_hint: SeverityHint::Low,
    },
    SymptomDef {
        name: "npm_global_stale",
        category: SymptomCategory::Sysadmin,
        severity_hint: SeverityHint::Low,
    },
    SymptomDef {
        name: "cargo_package_stale",
        category: SymptomCategory::Sysadmin,
        severity_hint: SeverityHint::Low,
    },
    SymptomDef {
        name: "zed_agent_stale",
        category: SymptomCategory::Sysadmin,
        severity_hint: SeverityHint::Low,
    },
    SymptomDef {
        name: "package_stale",
        category: SymptomCategory::Sysadmin,
        severity_hint: SeverityHint::Low,
    },
    SymptomDef {
        name: "rust_toolchain_stale",
        category: SymptomCategory::Sysadmin,
        severity_hint: SeverityHint::Low,
    },
    SymptomDef {
        name: "pip_packages_stale",
        category: SymptomCategory::Sysadmin,
        severity_hint: SeverityHint::Low,
    },
    SymptomDef {
        name: "docker_images_stale",
        category: SymptomCategory::Sysadmin,
        severity_hint: SeverityHint::Low,
    },
    SymptomDef {
        name: "dev_tool_stale",
        category: SymptomCategory::Sysadmin,
        severity_hint: SeverityHint::Low,
    },
    // Web search
    SymptomDef {
        name: "search_capability_needed",
        category: SymptomCategory::WebSearch,
        severity_hint: SeverityHint::Low,
    },
    SymptomDef {
        name: "web_knowledge_gap",
        category: SymptomCategory::WebSearch,
        severity_hint: SeverityHint::Medium,
    },
    SymptomDef {
        name: "skill_source_unknown",
        category: SymptomCategory::WebSearch,
        severity_hint: SeverityHint::Low,
    },
    SymptomDef {
        name: "external_reference_needed",
        category: SymptomCategory::WebSearch,
        severity_hint: SeverityHint::Medium,
    },
    SymptomDef {
        name: "documentation_outdated",
        category: SymptomCategory::WebSearch,
        severity_hint: SeverityHint::Low,
    },
    SymptomDef {
        name: "upstream_version_check_needed",
        category: SymptomCategory::WebSearch,
        severity_hint: SeverityHint::Low,
    },
    SymptomDef {
        name: "api_status_check_needed",
        category: SymptomCategory::WebSearch,
        severity_hint: SeverityHint::Medium,
    },
    SymptomDef {
        name: "package_metadata_stale",
        category: SymptomCategory::WebSearch,
        severity_hint: SeverityHint::Low,
    },
    SymptomDef {
        name: "security_advisory_relevant",
        category: SymptomCategory::WebSearch,
        severity_hint: SeverityHint::High,
    },
    // Skill discovery / management
    SymptomDef {
        name: "skill_not_in_catalog",
        category: SymptomCategory::SkillDiscovery,
        severity_hint: SeverityHint::Low,
    },
    SymptomDef {
        name: "skill_manifest_invalid",
        category: SymptomCategory::SkillDiscovery,
        severity_hint: SeverityHint::Medium,
    },
    SymptomDef {
        name: "skill_install_failed",
        category: SymptomCategory::SkillDiscovery,
        severity_hint: SeverityHint::Medium,
    },
    SymptomDef {
        name: "skill_version_stale",
        category: SymptomCategory::SkillDiscovery,
        severity_hint: SeverityHint::Low,
    },
    SymptomDef {
        name: "skill_symptom_not_in_catalog",
        category: SymptomCategory::SkillDiscovery,
        severity_hint: SeverityHint::Medium,
    },
    SymptomDef {
        name: "skill_probe_script_missing",
        category: SymptomCategory::SkillDiscovery,
        severity_hint: SeverityHint::High,
    },
    SymptomDef {
        name: "skill_rollback_unresolvable",
        category: SymptomCategory::SkillDiscovery,
        severity_hint: SeverityHint::High,
    },
    SymptomDef {
        name: "skill_dependency_missing",
        category: SymptomCategory::SkillDiscovery,
        severity_hint: SeverityHint::Medium,
    },
    SymptomDef {
        name: "skill_hardware_incompatible",
        category: SymptomCategory::SkillDiscovery,
        severity_hint: SeverityHint::High,
    },
    SymptomDef {
        name: "skill_coverage_gap",
        category: SymptomCategory::SkillDiscovery,
        severity_hint: SeverityHint::Low,
    },
    SymptomDef {
        name: "skill_needs_conversion",
        category: SymptomCategory::SkillDiscovery,
        severity_hint: SeverityHint::Medium,
    },
    SymptomDef {
        name: "flowdef_skill_detected",
        category: SymptomCategory::SkillDiscovery,
        severity_hint: SeverityHint::Low,
    },
    SymptomDef {
        name: "skill_audit_needed",
        category: SymptomCategory::SkillDiscovery,
        severity_hint: SeverityHint::Medium,
    },
    // Scenario testing
    SymptomDef {
        name: "agent_latency_spike",
        category: SymptomCategory::ScenarioTesting,
        severity_hint: SeverityHint::Medium,
    },
    SymptomDef {
        name: "agent_throughput_degraded",
        category: SymptomCategory::ScenarioTesting,
        severity_hint: SeverityHint::Medium,
    },
    SymptomDef {
        name: "agent_model_loading_failure",
        category: SymptomCategory::ScenarioTesting,
        severity_hint: SeverityHint::High,
    },
    SymptomDef {
        name: "agent_error_rate_elevated",
        category: SymptomCategory::ScenarioTesting,
        severity_hint: SeverityHint::High,
    },
    SymptomDef {
        name: "agent_baseline_regression",
        category: SymptomCategory::ScenarioTesting,
        severity_hint: SeverityHint::Medium,
    },
    SymptomDef {
        name: "agent_test_scenario_failed",
        category: SymptomCategory::ScenarioTesting,
        severity_hint: SeverityHint::Medium,
    },
    SymptomDef {
        name: "agent_concurrent_load_timeout",
        category: SymptomCategory::ScenarioTesting,
        severity_hint: SeverityHint::High,
    },
    SymptomDef {
        name: "agent_resource_exhaustion_under_load",
        category: SymptomCategory::ScenarioTesting,
        severity_hint: SeverityHint::Critical,
    },
    SymptomDef {
        name: "capability_attenuation_failure",
        category: SymptomCategory::ScenarioTesting,
        severity_hint: SeverityHint::Critical,
    },
    SymptomDef {
        name: "prompt_sanitization_failure",
        category: SymptomCategory::ScenarioTesting,
        severity_hint: SeverityHint::Critical,
    },
];

/// Look up a symptom by name. Returns `None` if not in the catalog.
#[must_use]
pub fn lookup_symptom(name: &str) -> Option<Symptom> {
    SYMPTOM_DEFS
        .iter()
        .find(|d| d.name == name)
        .map(|d| Symptom {
            name: d.name.to_string(),
            category: d.category,
            severity_hint: d.severity_hint,
        })
}

/// The set of symptom class names that skills may reference.
pub static SYMPTOMS: &[&str] = &[
    // Hardware / GPU
    "amdgpu_ring_hang",
    "amdgpu_reset",
    "vram_oom",
    "gpu_temp_high",
    "gpu_fallback_to_cpu",
    "rocm_unreachable",
    "nvme_media_errors",
    "gpu_not_responding",
    // System
    "oom_killer_active",
    "swap_pressure",
    "loadavg_high",
    "llm_slow",
    "resource_exhaustion",
    "memory_pressure",
    "disk_pressure",
    "boot_partition_full",
    "kernel_param_misconfigured",
    "llm_not_responding",
    "model_loading_failure",
    // Ubuntu-specific (ubuntu-jack skill)
    "systemd_service_degraded",
    "apt_stale",
    "snap_autorefresh_stall",
    "btrfs_fragmented",
    "zfs_scrub_overdue",
    "unattended_upgrades_failed",
    "kernel_livepatch_stale",
    "fwupd_outdated",
    "netplan_misconfigured",
    "apparmor_denial_spike",
    "journald_corruption",
    "tmp_mount_nosuid_missing",
    "package_out_of_date",
    "system_maintenance",
    "pending_reboot",
    "kernel_old_installed",
    "autoremove_needed",
    "held_packages",
    "snap_stale",
    "flatpak_stale",
    "service_degraded",
    // Cybernetic (pragmatic-cybernetics skill)
    "broken_feedback_loop",
    "alert_fatigue",
    "variety_deficit",
    "model_reality_divergence",
    "observer_coupling",
    "missing_double_loop",
    "homeostatic_drift",
    "coordination_oscillation",
    "context_saturation",
    "monitoring_independence_failure",
    // Semantic (pragmatic-semantics skill)
    "provenance_gap",
    "confidence_decay",
    "classification_ambiguity",
    "constraint_violation",
    "discourse_incoherence",
    "implicit_expectation_mismatch",
    "reference_implementation_drift",
    "operator_requests_interrogation",
    "knowledge_assessment_needed",
    "learning_goal_identified",
    "script_safety_concern",
    "idrs_violation",
    "script_style_issue",
    "script_content_needed",
    "script_inventory_needed",
    // Sysadmin (sysadmin skill — host maintenance tooling)
    "zombie_accumulation",
    "clock_skew",
    "systemd_timer_misfire",
    "journal_bloat",
    "process_table_bloat",
    "swap_retention",
    "coredump_accumulation",
    "stale_mount",
    "file_needs_creation",
    "file_needs_update",
    "file_needs_deletion",
    "directory_needs_creation",
    "config_drift",
    "docker_daemon_down",
    "ollama_daemon_down",
    "cache_bloat",
    "npm_global_stale",
    "cargo_package_stale",
    "zed_agent_stale",
    "package_stale",
    "rust_toolchain_stale",
    "pip_packages_stale",
    "docker_images_stale",
    "dev_tool_stale",
    // Web search (web-search skill — MCP bridge to Brave Search, Firecrawl, Browserbase)
    "search_capability_needed",
    "web_knowledge_gap",
    "skill_source_unknown",
    "external_reference_needed",
    "documentation_outdated",
    "upstream_version_check_needed",
    "api_status_check_needed",
    "package_metadata_stale",
    "security_advisory_relevant",
    // Skill discovery / management (skill-discovery, skill-manager skills)
    "skill_not_in_catalog",
    "skill_manifest_invalid",
    "skill_install_failed",
    "skill_version_stale",
    "skill_symptom_not_in_catalog",
    "skill_probe_script_missing",
    "skill_rollback_unresolvable",
    "skill_dependency_missing",
    "skill_hardware_incompatible",
    "skill_coverage_gap",
    "skill_needs_conversion",
    "flowdef_skill_detected",
    "skill_audit_needed",
    // Scenario testing (scenario-tester skill — test agentic AI systems)
    "agent_latency_spike",
    "agent_throughput_degraded",
    "agent_model_loading_failure",
    "agent_error_rate_elevated",
    "agent_baseline_regression",
    "agent_test_scenario_failed",
    "agent_concurrent_load_timeout",
    "agent_resource_exhaustion_under_load",
    // Scenario testing — security and capability probes
    "capability_attenuation_failure",
    "prompt_sanitization_failure",
];

/// Load symptoms from a YAML file on disk.
/// Retained for future runtime YAML loading; currently unused because
/// the static `SYMPTOMS` array covers all known symptoms.
#[allow(dead_code)]
pub fn load_symptoms_from_file(path: &Path) -> Vec<String> {
    match std::fs::read_to_string(path) {
        Ok(content) => parse_symptoms_yaml(&content),
        Err(_) => parse_symptoms_yaml(SYMPTOMS_YAML),
    }
}

/// Parse a YAML list of symptom strings.
///
/// Handles the `- name` format used in `data/symptoms.yaml`.
/// Retained for future runtime YAML loading.
#[allow(dead_code)]
fn parse_symptoms_yaml(yaml: &str) -> Vec<String> {
    // Simple line-based parser: lines starting with "- " are symptom entries.
    yaml.lines()
        .map(|l| l.trim())
        .filter(|l| l.starts_with("- "))
        .map(|l| l[2..].trim().to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_in_yaml_parses() {
        let parsed = parse_symptoms_yaml(SYMPTOMS_YAML);
        assert!(
            parsed.len() >= 70,
            "expected >=70 symptoms, got {}",
            parsed.len()
        );
        assert!(parsed.contains(&"vram_oom".to_string()));
        assert!(parsed.contains(&"agent_latency_spike".to_string()));
    }

    #[test]
    fn static_symptoms_matches_yaml() {
        let parsed = parse_symptoms_yaml(SYMPTOMS_YAML);
        for s in SYMPTOMS {
            assert!(
                parsed.contains(&s.to_string()),
                "SYMPTOMS constant has '{s}' but YAML doesn't"
            );
        }
    }

    #[test]
    fn load_from_nonexistent_falls_back() {
        let symptoms = load_symptoms_from_file(Path::new("/nonexistent/symptoms.yaml"));
        assert!(!symptoms.is_empty());
        assert!(symptoms.contains(&"vram_oom".to_string()));
    }

    #[test]
    fn symptom_from_str_valid() {
        let s: Symptom = "vram_oom".parse().unwrap();
        assert_eq!(s.name(), "vram_oom");
        assert_eq!(s.category(), SymptomCategory::Hardware);
        assert_eq!(s.severity_hint(), SeverityHint::Critical);
    }

    #[test]
    fn symptom_from_str_invalid() {
        let result = "made_up_symptom".parse::<Symptom>();
        assert!(result.is_err());
    }

    #[test]
    fn symptom_defs_cover_all_symptoms() {
        for name in SYMPTOMS {
            assert!(
                lookup_symptom(name).is_some(),
                "SYMPTOMS has '{name}' but SYMPTOM_DEFS doesn't"
            );
        }
    }

    #[test]
    fn symptom_display_roundtrips() {
        let s: Symptom = "llm_slow".parse().unwrap();
        assert_eq!(s.to_string(), "llm_slow");
    }

    #[test]
    fn symptom_serialize_roundtrips() {
        let s: Symptom = "zombie_accumulation".parse().unwrap();
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(json, "\"zombie_accumulation\"");
        let back: Symptom = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }
}

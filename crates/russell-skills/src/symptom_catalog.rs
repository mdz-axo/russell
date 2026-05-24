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
    // System
    "oom_killer_active",
    "swap_pressure",
    "loadavg_high",
    "llm_slow",
    "resource_exhaustion",
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
    // Sysadmin (sysadmin skill — host maintenance tooling)
    "zombie_accumulation",
    "clock_skew",
    "systemd_timer_misfire",
    "journal_bloat",
    "process_table_bloat",
    "swap_retention",
    "coredump_accumulation",
    "stale_mount",
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
        // The YAML has the same entries as the SYMPTOMS constant (currently 85).
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
        // Every entry in SYMPTOMS should be in the parsed YAML.
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
}

// SPDX-License-Identifier: MIT OR Apache-2.0
//! Known symptom catalog for Russell skills.
//!
//! Extending this catalog is governed by the skill lifecycle ADR
//! ([ADR-0024](../../docs/adr/0024-skill-registry-workshop-lifecycle.md)).
//! A manifest's `symptoms:` entry not in this set causes a
//! [`super::LoadError::UnknownSymptom`] at load time (poka-yoke).

/// The set of symptom class names that skills may reference.
/// A `symptoms:` entry in a manifest that is not in this set
/// causes a [`super::LoadError::UnknownSymptom`] at load time.
pub const SYMPTOMS: &[&str] = &[
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
    // Skill discovery (skill-discovery skill — find, evaluate, install new skills)
    "skill_not_in_catalog",
    "skill_manifest_invalid",
    "skill_install_failed",
    "skill_version_stale",
    "skill_symptom_not_in_catalog",
    "skill_probe_script_missing",
    "skill_rollback_unresolvable",
    "skill_dependency_missing",
    "skill_hardware_incompatible",
    // Scenario testing (scenario-tester skill — test agentic AI systems)
    "agent_latency_spike",
    "agent_throughput_degraded",
    "agent_model_loading_failure",
    "agent_error_rate_elevated",
    "agent_baseline_regression",
    "agent_test_scenario_failed",
    "agent_concurrent_load_timeout",
    "agent_resource_exhaustion_under_load",
];

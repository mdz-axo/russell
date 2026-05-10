// SPDX-License-Identifier: MIT OR Apache-2.0
//! Known symptom catalog for Russell skills.
//!
//! Extending this catalog requires a short ADR per ADR-0007.

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
];

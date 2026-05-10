// SPDX-License-Identifier: MIT OR Apache-2.0
//! Symptom catalog — the controlled vocabulary for skill matching.
//!
//! The Doctor looks up skills by symptom name. A skill whose
//! manifest references a symptom not in this set is rejected
//! at load time (poka-yoke).
//!
//! Adding a new symptom requires updating this file and
//! registering it in a short ADR.

use std::collections::BTreeSet;

lazy_static::lazy_static! {
    /// The canonical set of known symptom names.
    pub static ref SYMPTOMS: BTreeSet<&'static str> = BTreeSet::from([
        // --- GPU / vRAM ---
        "vram_oom",
        "amdgpu_ring_hang",
        "amdgpu_reset",
        "gpu_fallback_to_cpu",
        "rocm_unreachable",
        "nvidia_driver_stale",

        // --- Memory ---
        "system_memory_pressure",
        "oom_killer_active",
        "swap_thrashing",

        // --- Disk / Storage ---
        "disk_full",
        "disk_health_degraded",
        "inode_exhaustion",

        // --- CPU / Thermal ---
        "cpu_throttle",
        "thermal_shutdown_imminent",
        "high_steal_time",

        // --- Network ---
        "network_degraded",
        "dns_timeout",

        // --- Russell self ---
        "sentinel_stale",
        "journal_write_stall",
        "llm_slow",
        "timer_drift",

        // --- Generic ---
        "resource_exhaustion",
        "unknown_degradation",
    ]);
}

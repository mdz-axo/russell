// SPDX-License-Identifier: MIT OR Apache-2.0
//! Known symptom catalog for Russell skills.
//!
//! Extending this catalog requires a short ADR per ADR-0007.

/// The set of symptom class names that skills may reference.
/// A `symptoms:` entry in a manifest that is not in this set
/// causes a [`super::LoadError::UnknownSymptom`] at load time.
pub const SYMPTOMS: &[&str] = &[
    "amdgpu_ring_hang",
    "vram_oom",
    "gpu_temp_high",
    "nvme_media_errors",
    "oom_killer_active",
    "swap_pressure",
    "loadavg_high",
];

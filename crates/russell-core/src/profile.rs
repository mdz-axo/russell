// SPDX-License-Identifier: MIT OR Apache-2.0
//! The `russell.profile.v1` machine chart.
//!
//! See [ADR-0006](../../../docs/adr/0006-profile-abstraction.md).
//!
//! Phase 0 implements the read path and a minimal writer used by
//! the CLI `profile` subcommand to materialise a stub profile
//! when none exists. The real Bootstrap state machine
//! (`cybernetic-health-harness.md` §13) arrives in a later phase.

use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{CoreError, Result};

/// Schema tag persisted on every profile file.
pub const PROFILE_SCHEMA: &str = "russell.profile.v1";

fn default_schema() -> String {
    PROFILE_SCHEMA.to_string()
}

/// OS identity block.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OsInfo {
    /// `"linux" | "darwin" | "windows"` (only `linux` supported in v1).
    pub family: String,
    /// Distro id, e.g. `"ubuntu"`.
    pub distro: String,
    /// Version string, e.g. `"25.10"`.
    pub version: String,
    /// Kernel version, e.g. `"6.17.0-20-generic"`.
    pub kernel: String,
}

/// CPU identity block.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CpuInfo {
    /// Vendor string (`AuthenticAMD`, `GenuineIntel`).
    pub vendor: String,
    /// Human-readable model.
    pub model: String,
    /// Logical core count.
    #[serde(default)]
    pub cores: u32,
    /// Logical thread count.
    #[serde(default)]
    pub threads: u32,
}

/// Chassis / system identity.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChassisInfo {
    /// DMI vendor field.
    pub vendor: String,
    /// DMI product-name field.
    pub product: String,
    /// BIOS version.
    pub bios: String,
}

/// Aggregate host block.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HostInfo {
    /// OS details.
    pub os: OsInfo,
    /// Chassis details.
    pub chassis: ChassisInfo,
    /// CPU details.
    pub cpu: CpuInfo,
    /// System memory in mebibytes.
    #[serde(default)]
    pub memory_mib: u64,
    /// Swap in mebibytes.
    #[serde(default)]
    pub swap_mib: u64,
}

/// A GPU slot.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GpuInfo {
    /// PCI address, e.g. `"c4:00.0"`.
    pub pci: String,
    /// PCI vendor id, e.g. `"0x1002"`.
    pub vendor_id: String,
    /// Marketing name.
    pub name: String,
    /// gfx target, if applicable.
    pub gfx: Option<String>,
    /// Declared role: `"compute"`, `"display"`, `"hybrid"`.
    pub role: String,
}

/// Network-egress opt-ins.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NetworkOptIns {
    /// Whether the Nurse may reach a non-local LLM backend.
    #[serde(default)]
    pub llm_egress: bool,
    /// Whether the skill registry may be fetched over the network.
    #[serde(default)]
    pub skill_registry_egress: bool,
}

/// Generative-model overrides persisted in the profile.
///
/// All fields are `Option` — missing means "use the compiled default".
/// When the operator runs `/settings set temperature 0.8`, the value
/// is written here and survives restarts.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GenerativeConfig {
    /// Sampling temperature (0.0–2.0).
    pub temperature: Option<f64>,
    /// Top-k sampling.
    pub top_k: Option<u32>,
    /// Top-p (nucleus) sampling (0.0–1.0).
    pub top_p: Option<f64>,
    /// Repeat penalty (>= 1.0).
    pub repeat_penalty: Option<f64>,
    /// Whether HHH (Helpful, Honest, Harmless) filter is active.
    pub hhh_filter: Option<bool>,
    /// Active persona name.
    pub persona: Option<String>,
}

/// The full `russell.profile.v1` document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    /// Schema tag — always validated on load.
    #[serde(default = "default_schema")]
    pub schema: String,
    /// Stable machine fingerprint-derived ID.
    pub profile_id: String,
    /// When this profile was last authored.
    pub authored_at: String,
    /// Host block.
    #[serde(default)]
    pub host: HostInfo,
    /// GPU slots (0..n).
    #[serde(default)]
    pub gpus: Vec<GpuInfo>,
    /// When the bootstrap last completed.
    pub bootstrap_completed_at: Option<String>,
    /// When the honeymoon window ends (RFC3339). During the
    /// window, `risk >= high` interventions default to *propose*.
    pub honeymoon_ends_at: Option<String>,
    /// Capability flags (`"rocm"`, `"lvfs"`, `"polkit"`,
    /// `"systemd-user"`). Closed vocabulary.
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// Network egress opt-ins.
    #[serde(default)]
    pub network: NetworkOptIns,
    /// Generative-model overrides. Absent = use compiled defaults.
    #[serde(default)]
    pub generative: Option<GenerativeConfig>,
}

impl Profile {
    /// Build a minimal stub profile suitable for Phase-0 demos.
    /// Populates only the `schema`, `profile_id`, and timestamp;
    /// the Bootstrap will fill the rest.
    #[must_use]
    pub fn stub() -> Self {
        Self {
            schema: PROFILE_SCHEMA.to_string(),
            profile_id: format!("phase0-{}", ulid::Ulid::new()),
            authored_at: crate::time::now_rfc3339(),
            host: HostInfo::default(),
            gpus: Vec::new(),
            bootstrap_completed_at: None,
            honeymoon_ends_at: None,
            capabilities: Vec::new(),
            network: NetworkOptIns::default(),
            generative: None,
        }
    }

    /// Load from disk. Refuses unknown schema versions.
    pub fn load(path: &Path) -> Result<Self> {
        let bytes = std::fs::read(path).map_err(|e| CoreError::io(path, e))?;
        let parsed: Profile = serde_json::from_slice(&bytes)?;
        if parsed.schema != PROFILE_SCHEMA {
            return Err(CoreError::UnknownSchema {
                expected: PROFILE_SCHEMA,
                found: parsed.schema,
            });
        }
        Ok(parsed)
    }

    /// Atomic write: writes to `path.tmp`, fsyncs, then renames.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Io`] on FS errors.
    pub fn save(&self, path: &Path) -> Result<()> {
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        crate::paths::ensure_dir(parent)?;
        let tmp = parent.join(format!(
            "{}.tmp",
            path.file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("profile.json")
        ));
        {
            let mut f = std::fs::File::create(&tmp).map_err(|e| CoreError::io(&tmp, e))?;
            let buf = serde_json::to_vec_pretty(self)?;
            f.write_all(&buf).map_err(|e| CoreError::io(&tmp, e))?;
            f.sync_all().map_err(|e| CoreError::io(&tmp, e))?;
        }
        std::fs::rename(&tmp, path).map_err(|e| CoreError::io(path, e))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generative_config_default_is_all_none() {
        let cfg = GenerativeConfig::default();
        assert!(cfg.temperature.is_none());
        assert!(cfg.top_k.is_none());
        assert!(cfg.top_p.is_none());
        assert!(cfg.repeat_penalty.is_none());
        assert!(cfg.hhh_filter.is_none());
        assert!(cfg.persona.is_none());
    }

    #[test]
    fn generative_config_serialization_roundtrip() {
        let cfg = GenerativeConfig {
            temperature: Some(0.8),
            top_k: Some(50),
            top_p: None,
            repeat_penalty: Some(1.2),
            hhh_filter: Some(false),
            persona: Some("nurse".to_string()),
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let back: GenerativeConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.temperature, Some(0.8));
        assert_eq!(back.top_k, Some(50));
        assert!(back.top_p.is_none());
        assert_eq!(back.repeat_penalty, Some(1.2));
        assert_eq!(back.hhh_filter, Some(false));
        assert_eq!(back.persona, Some("nurse".to_string()));
    }

    #[test]
    fn generative_config_none_fields_deserialize_from_missing_keys() {
        // Simulates a profile JSON that has no generative block at all.
        let json = r"{}";
        let cfg: GenerativeConfig = serde_json::from_str(json).unwrap();
        assert!(cfg.temperature.is_none());
        assert!(cfg.top_k.is_none());
    }

    #[test]
    fn profile_without_generative_loads() {
        // A legacy profile without the `generative` field should still load.
        let json = serde_json::json!({
            "schema": PROFILE_SCHEMA,
            "profile_id": "test-id",
            "authored_at": "2025-01-01T00:00:00Z"
        });
        let profile: Profile = serde_json::from_value(json).unwrap();
        assert!(profile.generative.is_none());
    }

    #[test]
    fn profile_with_generative_roundtrip() {
        let mut profile = Profile::stub();
        profile.generative = Some(GenerativeConfig {
            temperature: Some(0.9),
            top_k: None,
            top_p: Some(0.8),
            repeat_penalty: None,
            hhh_filter: None,
            persona: None,
        });
        let json = serde_json::to_string_pretty(&profile).unwrap();
        let back: Profile = serde_json::from_str(&json).unwrap();
        let gc = back.generative.unwrap();
        assert_eq!(gc.temperature, Some(0.9));
        assert!(gc.top_k.is_none());
        assert_eq!(gc.top_p, Some(0.8));
    }

    #[test]
    fn generative_config_partial_update_persists() {
        // Setting only one field leaves others as None ("use default").
        let mut cfg = GenerativeConfig::default();
        cfg.temperature = Some(0.5);
        let json = serde_json::to_string(&cfg).unwrap();
        let back: GenerativeConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.temperature, Some(0.5));
        assert!(back.top_k.is_none());
        assert!(back.persona.is_none());
    }
}

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
        }
    }

    /// Load from disk. Refuses unknown schema versions.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Io`] on FS errors, [`CoreError::Json`]
    /// on malformed JSON, [`CoreError::UnknownSchema`] on version
    /// mismatch.
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
    fn stub_round_trips() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("profile.json");
        let p = Profile::stub();
        p.save(&path).unwrap();
        let back = Profile::load(&path).unwrap();
        assert_eq!(back.schema, PROFILE_SCHEMA);
        assert!(back.profile_id.starts_with("phase0-"));
    }

    #[test]
    fn unknown_schema_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("profile.json");
        std::fs::write(
            &path,
            r#"{
              "schema":"russell.profile.v99",
              "profile_id":"x",
              "authored_at":"2026-04-17T00:00:00Z"
            }"#,
        )
        .unwrap();
        match Profile::load(&path) {
            Err(CoreError::UnknownSchema { expected, found }) => {
                assert_eq!(expected, PROFILE_SCHEMA);
                assert_eq!(found, "russell.profile.v99");
            }
            other => panic!("expected UnknownSchema, got {other:?}"),
        }
    }

    #[test]
    fn missing_file_reports_io_error() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("absent.json");
        match Profile::load(&path) {
            Err(CoreError::Io { .. }) => (),
            other => panic!("expected Io, got {other:?}"),
        }
    }

    #[test]
    fn save_is_atomic_no_tmp_left() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("profile.json");
        Profile::stub().save(&path).unwrap();
        let entries: Vec<_> = std::fs::read_dir(tmp.path())
            .unwrap()
            .filter_map(std::result::Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert!(entries.contains(&"profile.json".to_string()));
        assert!(!entries.iter().any(|n| n.ends_with(".tmp")));
    }
}

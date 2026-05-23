// SPDX-License-Identifier: MIT OR Apache-2.0
//! Skill dispatch port — hexagonal architecture abstraction.
//!
//! Defines the port interface for skill dispatch, allowing different
//! adapter implementations (local subprocess, remote RPC, mock for testing).
//!
//! See [ADR-0013](../../../docs/adr/0013-rust-workspace-layout.md).

use crate::error::Result;
use crate::types::{ProbeInfo, SkillInfo};

/// Port for skill dispatch operations.
///
/// Implementations provide skill discovery, information retrieval,
/// and probe/intervention execution. The ACP handler depends on this
/// trait rather than a concrete implementation.
#[async_trait::async_trait]
pub trait SkillDispatchPort: Send {
    /// Load all public skills exposed via ACP.
    fn load_public_skills(&self) -> Vec<SkillInfo>;

    /// Get information about a specific skill.
    fn get_skill_info(&self, skill_id: &str) -> Option<SkillInfo>;

    /// List all public probes across all skills.
    fn list_probes(&self) -> Vec<ProbeInfo>;

    /// Dispatch a skill step (probe or intervention).
    ///
    /// # Errors
    ///
    /// Returns [`AcpError`] if the skill is not found, not exposed,
    /// or execution fails.
    async fn dispatch_skill(&self, skill_id: &str, args: &serde_json::Value) -> Result<String>;

    /// Run a specific probe (read-only, always allowed if skill is public).
    ///
    /// # Errors
    ///
    /// Returns [`AcpError`] if the probe is not found or execution fails.
    async fn run_probe(
        &self,
        skill_id: &str,
        probe_id: &str,
        args: &serde_json::Value,
    ) -> Result<String>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_port_trait_exists() {
        // Verify the trait is defined and can be used as a trait object.
        fn _accepts_port(_port: Box<dyn SkillDispatchPort>) {}
    }
}

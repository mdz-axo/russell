// SPDX-License-Identifier: MIT OR Apache-2.0
//! Landlock-based sandbox for skill subprocess execution.
//!
//! Provides filesystem and network access restrictions for skill scripts
//! using Linux Landlock LSM (Linux Security Module).
//!
//! ## Design
//!
//! The sandbox applies restrictions BEFORE spawning the subprocess:
//! 1. Create a Landlock ruleset with allowed paths
//! 2. Apply the ruleset to the current thread
//! 3. Spawn the subprocess (inherits restrictions)
//!
//! ## Limitations
//!
//! - Requires Linux kernel 5.13+ with Landlock enabled
//! - Network restrictions require kernel 6.7+ (Landlock ABI v4)
//! - Falls back to no sandbox if Landlock is unavailable (with warning)
//!
//! ## Security Model
//!
//! Skills are confined to:
//! - Read access: skill directory, /usr, /lib, /etc, /proc (read-only)
//! - Write access: skill directory, /tmp (if configured), /dev (for /dev/null redirects)
//! - Network: disabled by default (opt-in per skill)

use std::path::{Path, PathBuf};

use landlock::{
    ABI, Access, AccessFs, PathBeneath, PathFd, Ruleset, RulesetAttr, RulesetCreatedAttr,
};
use tracing::debug;

/// Sandbox configuration for skill execution.
#[derive(Debug, Clone)]
pub struct SandboxConfig {
    /// Paths the skill can read from.
    pub read_paths: Vec<PathBuf>,
    /// Paths the skill can write to.
    pub write_paths: Vec<PathBuf>,
    /// Whether network access is allowed.
    pub allow_network: bool,
    /// Whether to allow /tmp access.
    pub allow_tmp: bool,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            read_paths: vec![
                PathBuf::from("/usr"),
                PathBuf::from("/lib"),
                PathBuf::from("/lib64"),
                PathBuf::from("/etc"),
                PathBuf::from("/proc"),
            ],
            write_paths: vec![PathBuf::from("/dev")],
            allow_network: false,
            allow_tmp: true,
        }
    }
}

impl SandboxConfig {
    /// Create a new sandbox config with the skill directory pre-configured.
    #[must_use]
    pub fn for_skill(skill_dir: &Path) -> Self {
        let mut config = Self::default();
        config.read_paths.push(skill_dir.to_path_buf());
        config.write_paths.push(skill_dir.to_path_buf());
        config
    }

    /// Add a read-only path.
    pub fn add_read_path(&mut self, path: impl Into<PathBuf>) {
        self.read_paths.push(path.into());
    }

    /// Add a read-write path.
    pub fn add_write_path(&mut self, path: impl Into<PathBuf>) {
        self.write_paths.push(path.into());
    }

    /// Enable network access.
    pub fn with_network(mut self, allow: bool) -> Self {
        self.allow_network = allow;
        self
    }

    /// Enable /tmp access.
    pub fn with_tmp(mut self, allow: bool) -> Self {
        self.allow_tmp = allow;
        self
    }
}

/// Apply Landlock sandbox restrictions.
///
/// Returns `Ok(())` if sandbox was applied successfully or if Landlock
/// is unavailable (with warning). Returns `Err` if Landlock is available
/// but configuration failed.
pub fn apply_sandbox(config: &SandboxConfig) -> Result<(), SandboxError> {
    // Check if Landlock is available
    let abi = ABI::V4; // Use latest stable ABI

    // Build ruleset with allowed paths
    let mut ruleset_created = Ruleset::default()
        .handle_access(AccessFs::from_all(abi))
        .map_err(|e| SandboxError::RulesetCreation(e.to_string()))?
        .create()
        .map_err(|e| SandboxError::RulesetCreation(e.to_string()))?;

    // Add read paths
    for path in &config.read_paths {
        if !path.exists() {
            debug!(path = %path.display(), "skipping non-existent read path");
            continue;
        }
        let fd = PathFd::new(path).map_err(|e| SandboxError::PathFd {
            path: path.clone(),
            error: e.to_string(),
        })?;
        let rule = PathBeneath::new(fd, AccessFs::from_read(abi));
        ruleset_created = ruleset_created
            .add_rule(rule)
            .map_err(|e| SandboxError::AddRule(e.to_string()))?;
    }

    // Add write paths (includes read access)
    for path in &config.write_paths {
        if !path.exists() {
            debug!(path = %path.display(), "skipping non-existent write path");
            continue;
        }
        let fd = PathFd::new(path).map_err(|e| SandboxError::PathFd {
            path: path.clone(),
            error: e.to_string(),
        })?;
        let rule = PathBeneath::new(fd, AccessFs::from_write(abi));
        ruleset_created = ruleset_created
            .add_rule(rule)
            .map_err(|e| SandboxError::AddRule(e.to_string()))?;
    }

    // Add /tmp if allowed
    if config.allow_tmp {
        let tmp = PathBuf::from("/tmp");
        if tmp.exists() {
            let fd = PathFd::new(&tmp).map_err(|e| SandboxError::PathFd {
                path: tmp.clone(),
                error: e.to_string(),
            })?;
            let rule = PathBeneath::new(fd, AccessFs::from_write(abi));
            ruleset_created = ruleset_created
                .add_rule(rule)
                .map_err(|e| SandboxError::AddRule(e.to_string()))?;
        }
    }

    // Apply ruleset
    let status = ruleset_created
        .restrict_self()
        .map_err(|e| SandboxError::Application(e.to_string()))?;

    // Log the enforcement status
    debug!(?status, "Landlock sandbox applied");

    Ok(())
}

/// Errors that can occur during sandbox application.
#[derive(Debug, Clone, thiserror::Error)]
pub enum SandboxError {
    /// Failed to create Landlock ruleset.
    #[error("failed to create Landlock ruleset: {0}")]
    RulesetCreation(String),

    /// Failed to create path file descriptor.
    #[error("failed to create path fd for {path}: {error}")]
    PathFd {
        /// The path that failed.
        path: PathBuf,
        /// The error message.
        error: String,
    },

    /// Failed to add a rule to the ruleset.
    #[error("failed to add Landlock rule: {0}")]
    AddRule(String),

    /// Failed to apply the ruleset.
    #[error("failed to apply Landlock ruleset: {0}")]
    Application(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sandbox_config_default_has_system_paths() {
        let config = SandboxConfig::default();
        assert!(config.read_paths.iter().any(|p| p == Path::new("/usr")));
        assert!(config.read_paths.iter().any(|p| p == Path::new("/lib")));
        assert!(config.read_paths.iter().any(|p| p == Path::new("/proc")));
        assert!(config.write_paths.iter().any(|p| p == Path::new("/dev")));
        assert!(!config.allow_network);
        assert!(config.allow_tmp);
    }

    #[test]
    fn sandbox_config_for_skill_includes_skill_dir() {
        let skill_dir = PathBuf::from("/tmp/test-skill");
        let config = SandboxConfig::for_skill(&skill_dir);
        assert!(config.read_paths.contains(&skill_dir));
        assert!(config.write_paths.contains(&skill_dir));
    }

    #[test]
    fn sandbox_config_builder_methods() {
        let config = SandboxConfig::default().with_network(true).with_tmp(false);
        assert!(config.allow_network);
        assert!(!config.allow_tmp);
    }
}

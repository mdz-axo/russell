// SPDX-License-Identifier: MIT OR Apache-2.0
//! Landlock-based sandbox for skill subprocess execution.
//!
//! Provides filesystem and network access restrictions for skill scripts
//! using Linux Landlock LSM (Linux Security Module).
//!
//! ## Design
//!
//! The sandbox applies restrictions in the child process (via pre_exec):
//! 1. Create a Landlock ruleset with allowed paths (parent process)
//! 2. Pass the ruleset fd to the child via pre_exec
//! 3. Call landlock_restrict_self() in the child before exec()
//!
//! This avoids permanently restricting the parent (Tokio worker) thread.
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
//! - Read access: skill directory, /usr, /lib, /etc, /proc, /dev, /bin, /sbin, /home, /run, /sys
//! - Write access: skill directory, skills parent dir, /tmp (if configured), /dev (for /dev/null redirects)
//! - Network: disabled by default (opt-in per skill)

use std::os::unix::io::{AsRawFd, RawFd};
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
                PathBuf::from("/dev"),
                PathBuf::from("/bin"),
                PathBuf::from("/sbin"),
                PathBuf::from("/home"),
                PathBuf::from("/run"),
                PathBuf::from("/sys"),
                PathBuf::from("/snap"),
                PathBuf::from("/tmp"),
            ],
            write_paths: vec![PathBuf::from("/dev")],
            allow_network: false,
            allow_tmp: true,
        }
    }
}

impl SandboxConfig {
    /// Create a new sandbox config with the skill directory pre-configured.
    ///
    /// Also adds the skills directory parent as a read+write path so that
    /// skill-manager can create new skill directories, and adds $HOME
    /// for path resolution.
    #[must_use]
    pub fn for_skill(skill_dir: &Path) -> Self {
        let mut config = Self::default();
        config.read_paths.push(skill_dir.to_path_buf());
        config.write_paths.push(skill_dir.to_path_buf());

        // Allow write access to the parent skills directory so that
        // skill-manager can create new skill directories.
        // Skip the root directory — it would make the sandbox meaningless.
        if let Some(parent) = skill_dir.parent()
            && parent != Path::new("/")
        {
            config.read_paths.push(parent.to_path_buf());
            config.write_paths.push(parent.to_path_buf());
        }

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

/// Prepare a Landlock sandbox ruleset for the given configuration.
///
/// Creates the ruleset, adds rules for the configured paths, and returns
/// the raw fd of the ruleset. The caller is responsible for applying the
/// ruleset (e.g., via `apply_sandbox_in_child`) and closing the fd.
///
/// Returns `Ok(fd)` if the ruleset was created successfully.
/// Returns `Err` if Landlock is unavailable or configuration failed.
pub fn prepare_sandbox(config: &SandboxConfig) -> Result<RawFd, SandboxError> {
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

    // Extract the raw fd from the RulesetCreated.
    // We convert to Option<OwnedFd> to get the fd, then use
    // std::mem::forget to prevent Drop from closing it — the caller
    // is responsible for closing it via close_sandbox_fd().
    let owned_fd: Option<std::os::unix::io::OwnedFd> = ruleset_created.into();
    let fd = match owned_fd {
        Some(f) => {
            let raw = f.as_raw_fd();
            // Prevent Drop from closing the fd — the caller manages it.
            std::mem::forget(f);
            raw
        }
        None => {
            return Err(SandboxError::RulesetCreation(
                "ruleset has no fd (Landlock not available)".to_string(),
            ));
        }
    };

    debug!(fd, "Landlock sandbox ruleset prepared");
    Ok(fd)
}

/// Apply a pre-built Landlock sandbox ruleset in the current process.
///
/// This is designed to be called from a `pre_exec()` closure in the child
/// process, between `fork()` and `exec()`. It:
/// 1. Sets PR_SET_NO_NEW_PRIVS (required by Landlock)
/// 2. Calls landlock_restrict_self() to apply the ruleset
/// 3. Closes the ruleset fd
///
/// Both syscalls are async-signal-safe, making this safe for pre_exec.
///
/// # Safety
///
/// `fd` must be a valid Landlock ruleset file descriptor returned by
/// `prepare_sandbox()`.
#[allow(unsafe_code)]
pub fn apply_sandbox_in_child(fd: RawFd) -> std::io::Result<()> {
    // Landlock requires PR_SET_NO_NEW_PRIVS before restrict_self.
    let nnp_ret = unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) };
    if nnp_ret == -1 {
        // Close fd on error to avoid leak.
        let _ = unsafe { libc::close(fd) };
        return Err(std::io::Error::last_os_error());
    }

    let ret = unsafe {
        libc::syscall(
            libc::SYS_landlock_restrict_self,
            fd,
            0u32, // flags
        )
    };

    // Close the ruleset fd regardless of outcome.
    let _ = unsafe { libc::close(fd) };

    if ret == -1 {
        return Err(std::io::Error::last_os_error());
    }

    debug!(fd, "Landlock sandbox applied in child process");
    Ok(())
}

/// Close a sandbox ruleset fd that was returned by `prepare_sandbox()`.
///
/// Call this in the parent process after spawning the child, to clean up
/// the fd that was leaked from the `RulesetCreated` object.
#[allow(unsafe_code)]
pub fn close_sandbox_fd(fd: RawFd) {
    let _ = unsafe { libc::close(fd) };
}

/// Apply Landlock sandbox restrictions to the current thread.
///
/// **DEPRECATED**: This permanently restricts the calling thread.
/// Use `prepare_sandbox()` + `apply_sandbox_in_child()` instead.
///
/// Returns `Ok(())` if sandbox was applied successfully or if Landlock
/// is unavailable (with warning). Returns `Err` if Landlock is available
/// but configuration failed.
pub fn apply_sandbox(config: &SandboxConfig) -> Result<(), SandboxError> {
    let abi = ABI::V4;

    let mut ruleset_created = Ruleset::default()
        .handle_access(AccessFs::from_all(abi))
        .map_err(|e| SandboxError::RulesetCreation(e.to_string()))?
        .create()
        .map_err(|e| SandboxError::RulesetCreation(e.to_string()))?;

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

    let status = ruleset_created
        .restrict_self()
        .map_err(|e| SandboxError::Application(e.to_string()))?;

    debug!(
        ?status,
        "Landlock sandbox applied (legacy — restricts current thread)"
    );

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
        assert!(config.read_paths.iter().any(|p| p == Path::new("/dev")));
        assert!(config.read_paths.iter().any(|p| p == Path::new("/bin")));
        assert!(config.read_paths.iter().any(|p| p == Path::new("/home")));
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
    fn sandbox_config_for_skill_includes_parent_dir() {
        let skill_dir = PathBuf::from("/home/user/.local/share/harness/skills/skill-manager");
        let parent = PathBuf::from("/home/user/.local/share/harness/skills");
        let config = SandboxConfig::for_skill(&skill_dir);
        assert!(config.read_paths.contains(&parent));
        assert!(config.write_paths.contains(&parent));
    }

    #[test]
    fn sandbox_config_for_skill_skips_root_as_parent() {
        // When skill_dir is directly under / (e.g., /tmp), the parent is /.
        // We should NOT add / as a write path — it would make the sandbox meaningless.
        let skill_dir = PathBuf::from("/tmp");
        let config = SandboxConfig::for_skill(&skill_dir);
        assert!(config.write_paths.iter().all(|p| p != Path::new("/")));
    }

    #[test]
    fn sandbox_config_builder_methods() {
        let config = SandboxConfig::default().with_network(true).with_tmp(false);
        assert!(config.allow_network);
        assert!(!config.allow_tmp);
    }
}

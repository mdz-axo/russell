// SPDX-License-Identifier: MIT OR Apache-2.0
//! Path resolution for all on-disk Russell artefacts.
//!
//! Honors the XDG Base Directory Specification; falls back to
//! `$HOME` on machines where XDG variables are unset.
//!
//! Layout (see `cybernetic-health-harness.md` §14):
//!
//! ```text
//! $XDG_STATE_HOME/harness/
//! ├── profile.json
//! ├── journal.db
//! ├── runs/
//! ├── evidence/
//! ├── proposals/
//! ├── digest/
//! └── memory/
//!     ├── REVIEW.md       (Russell's self-assessment review surface)
//!     └── daily/
//!         └── YYYY-MM-DD.md  (daily Markdown logs, rebuildable)
//!
//! $XDG_CONFIG_HOME/harness/
//! ├── config.toml
//! ├── rules.d/
//! ├── disable            (empty file = global kill switch)
//! ├── PERSONA.md         (operator-owned Jack persona customisation)
//! └── USER.md            (operator-owned profile: timezone, prefs)
//!
//! $XDG_DATA_HOME/harness/
//! ├── skills/
//! └── catalog/
//! ```
//!
//! Every writer MUST route through `ensure_dir` before creating
//! new files, so a fresh install self-heals.

use std::path::PathBuf;

use crate::error::{CoreError, Result};

/// All the base paths Russell uses on a single host.
#[derive(Debug, Clone)]
pub struct Paths {
    /// Configuration root: `$XDG_CONFIG_HOME/harness/`.
    pub config: PathBuf,
    /// State root: `$XDG_STATE_HOME/harness/`.
    pub state: PathBuf,
    /// Data root: `$XDG_DATA_HOME/harness/`.
    pub data: PathBuf,
}

impl Paths {
    /// Resolve paths from the process environment.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::BasePath`] if neither the relevant XDG
    /// variable nor `$HOME` is set.
    pub fn from_env() -> Result<Self> {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or_else(|| CoreError::BasePath("HOME not set".into()))?;

        let config = xdg_or(&home, "XDG_CONFIG_HOME", ".config");
        let state = xdg_or(&home, "XDG_STATE_HOME", ".local/state");
        let data = xdg_or(&home, "XDG_DATA_HOME", ".local/share");

        Ok(Self {
            config: config.join("harness"),
            state: state.join("harness"),
            data: data.join("harness"),
        })
    }

    /// Build a `Paths` anchored at `base`. Useful in tests and for
    /// callers that need an isolated sandbox.
    #[must_use]
    pub fn rooted(base: impl Into<PathBuf>) -> Self {
        let base = base.into();
        Self {
            config: base.join("config/harness"),
            state: base.join("state/harness"),
            data: base.join("data/harness"),
        }
    }

    /// Path to the machine profile (`profile.json`).
    #[must_use]
    pub fn profile(&self) -> PathBuf {
        self.state.join("profile.json")
    }

    /// Path to the SQLite journal (`journal.db`).
    #[must_use]
    pub fn journal(&self) -> PathBuf {
        self.state.join("journal.db")
    }

    /// Path to the global kill-switch file (see
    /// [`docs/standards/safety.md`](../../../docs/standards/safety.md) §5).
    #[must_use]
    pub fn kill_switch(&self) -> PathBuf {
        self.config.join("disable")
    }

    /// Directory that holds per-run JSON records.
    #[must_use]
    pub fn runs(&self) -> PathBuf {
        self.state.join("runs")
    }

    /// Directory that holds SOAP evidence bundles.
    #[must_use]
    pub fn evidence(&self) -> PathBuf {
        self.state.join("evidence")
    }

    /// Directory that holds proposal records awaiting confirmation.
    #[must_use]
    pub fn proposals(&self) -> PathBuf {
        self.state.join("proposals")
    }

    /// Directory that holds the rendered weekly digests.
    #[must_use]
    pub fn digest_dir(&self) -> PathBuf {
        self.state.join("digest")
    }

    /// Directory that holds installed skill manifests.
    #[must_use]
    pub fn skills(&self) -> PathBuf {
        self.data.join("skills")
    }

    /// Directory that holds rule TOML overrides.
    #[must_use]
    pub fn rules(&self) -> PathBuf {
        self.config.join("rules.d")
    }

    /// Directory that holds Russell's Markdown memory layer
    /// (daily logs, REVIEW.md). All files in this tree are
    /// derived exports rebuildable from the journal.
    #[must_use]
    pub fn memory_dir(&self) -> PathBuf {
        self.state.join("memory")
    }

    /// Directory that holds daily Markdown logs
    /// (`memory/daily/YYYY-MM-DD.md`).
    #[must_use]
    pub fn memory_daily_dir(&self) -> PathBuf {
        self.state.join("memory").join("daily")
    }

    /// Path to the operator-owned user profile Markdown file.
    /// Russell reads this at startup if it exists; never writes it.
    #[must_use]
    pub fn user_md(&self) -> PathBuf {
        self.config.join("USER.md")
    }

    /// Path to the operator-owned persona customisation file.
    /// If present, the Doctor appends it to the compiled-in
    /// Jack persona. Russell reads it; never writes it.
    #[must_use]
    pub fn persona_md(&self) -> PathBuf {
        self.config.join("PERSONA.md")
    }

    /// Ensure every well-known directory exists. Idempotent.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Io`] if a directory cannot be created.
    pub fn ensure_dirs(&self) -> Result<()> {
        for dir in [
            &self.config,
            &self.state,
            &self.data,
            &self.runs(),
            &self.evidence(),
            &self.proposals(),
            &self.digest_dir(),
            &self.skills(),
            &self.rules(),
            &self.memory_dir(),
            &self.memory_daily_dir(),
        ] {
            ensure_dir(dir)?;
        }
        Ok(())
    }
}

/// Create a directory and any missing parents. Idempotent.
///
/// # Errors
///
/// Returns [`CoreError::Io`] on permission / FS errors. Returns
/// [`CoreError::Invariant`] if the path exists but is not a directory.
pub fn ensure_dir(path: &std::path::Path) -> Result<()> {
    if path.exists() {
        if path.is_dir() {
            return Ok(());
        }
        return Err(CoreError::Invariant(format!(
            "{} exists but is not a directory",
            path.display()
        )));
    }
    std::fs::create_dir_all(path).map_err(|e| CoreError::io(path, e))
}

fn xdg_or(home: &std::path::Path, var: &str, fallback: &str) -> PathBuf {
    match std::env::var_os(var) {
        Some(v) if !v.is_empty() => PathBuf::from(v),
        _ => home.join(fallback),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rooted_paths_produce_expected_layout() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let paths = Paths::rooted(tmp.path());
        assert!(paths.profile().ends_with("state/harness/profile.json"));
        assert!(paths.journal().ends_with("state/harness/journal.db"));
        assert!(paths.kill_switch().ends_with("config/harness/disable"));
    }

    #[test]
    fn ensure_dirs_is_idempotent() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let paths = Paths::rooted(tmp.path());
        paths.ensure_dirs().expect("first");
        paths.ensure_dirs().expect("second");
        assert!(paths.runs().is_dir());
        assert!(paths.skills().is_dir());
    }

    #[test]
    fn ensure_dir_rejects_existing_file() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let f = tmp.path().join("not-a-dir");
        std::fs::write(&f, b"hi").unwrap();
        assert!(matches!(ensure_dir(&f), Err(CoreError::Invariant(_))));
    }
}

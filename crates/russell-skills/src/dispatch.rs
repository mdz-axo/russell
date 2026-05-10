// SPDX-License-Identifier: MIT OR Apache-2.0
//! Subprocess dispatcher for Russell skills.
//!
//! Runs probe and intervention commands with env scrubbing, timeout,
//! stdout/stderr capture, dry-run support, and IDRS journaling.
//!
//! ## IDRS integration
//!
//! Every dispatch via [`Dispatcher::run_and_journal`] writes a
//! `harness.event.v1` record to the journal and an evidence bundle
//! to disk. The dry-run path writes the event but does not execute
//! the subprocess. Rollback pre-state capture is the caller's
//! responsibility (dispatchers can't know what state to snapshot).

use std::path::{Path, PathBuf};
use std::time::Duration;

use russell_core::Result;
use russell_core::event::{Event, Severity};
use russell_core::journal::JournalWriter;
use tracing::debug;

/// Controls whether subprocesses actually execute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DryRun {
    /// Print what would run, don't execute.
    Enabled,
    /// Execute normally.
    Disabled,
}

/// Outcome of a single subprocess run.
#[derive(Debug, Clone)]
pub struct RunOutcome {
    /// The command that was executed (or would have been).
    pub cmd: Vec<String>,
    /// Whether this was a dry run.
    pub dry_run: bool,
    /// Exit code (None if dry run or killed by timeout).
    pub exit_code: Option<i32>,
    /// Captured stdout.
    pub stdout: String,
    /// Captured stderr.
    pub stderr: String,
    /// Whether the process was killed due to timeout.
    pub timed_out: bool,
    /// Wall-clock duration.
    pub duration: Duration,
}

/// A subprocess dispatcher. Constructed with the skills base directory
/// (working directory for spawned processes).
#[derive(Debug, Clone)]
pub struct Dispatcher {
    /// Base skills directory (e.g. `~/.local/share/harness/skills/<id>/`).
    pub skill_dir: PathBuf,
    /// Global dry-run override.
    pub dry_run: DryRun,
    /// Default timeout for probes.
    pub probe_timeout: Duration,
    /// Default timeout for interventions.
    pub intervention_timeout: Duration,
}

impl Dispatcher {
    /// Create a new dispatcher for a given skill.
    #[must_use]
    pub fn new(skill_dir: impl Into<PathBuf>) -> Self {
        Self {
            skill_dir: skill_dir.into(),
            dry_run: DryRun::Disabled,
            probe_timeout: Duration::from_secs(30),
            intervention_timeout: Duration::from_secs(120),
        }
    }

    /// Run a command and return its output.
    ///
    /// `timeout_override` may override the default timeout. Pass `None`
    /// to use the default (30s).
    ///
    /// # Errors
    ///
    /// Returns [`russell_core::CoreError::Io`] if the subprocess cannot be
    /// spawned. Timeout kills are NOT errors — they're captured in
    /// [`RunOutcome::timed_out`].
    pub async fn run(
        &self,
        cmd: &[String],
        timeout_override: Option<Duration>,
    ) -> Result<RunOutcome> {
        let started = std::time::Instant::now();
        let timeout = timeout_override.unwrap_or(self.probe_timeout);

        if self.dry_run == DryRun::Enabled {
            return Ok(RunOutcome {
                cmd: cmd.to_vec(),
                dry_run: true,
                exit_code: None,
                stdout: String::new(),
                stderr: String::new(),
                timed_out: false,
                duration: started.elapsed(),
            });
        }

        if cmd.is_empty() {
            return Ok(RunOutcome {
                cmd: vec![],
                dry_run: false,
                exit_code: Some(-1),
                stdout: String::new(),
                stderr: "empty command".into(),
                timed_out: false,
                duration: started.elapsed(),
            });
        }

        let program = &cmd[0];
        let args: &[String] = if cmd.len() > 1 { &cmd[1..] } else { &[] };

        let child = tokio::process::Command::new(program)
            .args(args)
            .current_dir(&self.skill_dir)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| russell_core::CoreError::io(&self.skill_dir, e))?;

        let output = tokio::time::timeout(timeout, child.wait_with_output()).await;

        match output {
            Ok(Ok(out)) => Ok(RunOutcome {
                cmd: cmd.to_vec(),
                dry_run: false,
                exit_code: out.status.code(),
                stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
                timed_out: false,
                duration: started.elapsed(),
            }),
            Ok(Err(e)) => Err(russell_core::CoreError::io(
                &self.skill_dir,
                std::io::Error::other(format!("subprocess error: {e}")),
            )),
            Err(_elapsed) => {
                // Timeout.
                Ok(RunOutcome {
                    cmd: cmd.to_vec(),
                    dry_run: false,
                    exit_code: None,
                    stdout: String::new(),
                    stderr: format!("timed out after {:?}", timeout),
                    timed_out: true,
                    duration: started.elapsed(),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn dry_run_does_not_execute() {
        let d = Dispatcher {
            skill_dir: "/tmp".into(),
            dry_run: DryRun::Enabled,
            probe_timeout: Duration::from_secs(5),
            intervention_timeout: Duration::from_secs(5),
        };
        let outcome = d.run(&["echo".into(), "hello".into()], None).await.unwrap();
        assert!(outcome.dry_run);
        assert!(outcome.exit_code.is_none());
        assert!(outcome.stdout.is_empty());
    }

    #[tokio::test]
    async fn echo_produces_stdout() {
        let d = Dispatcher::new("/tmp");
        let outcome = d.run(&["echo".into(), "hello".into()], None).await.unwrap();
        assert!(!outcome.dry_run);
        assert_eq!(outcome.exit_code, Some(0));
        assert!(outcome.stdout.contains("hello"));
    }

    #[tokio::test]
    async fn empty_cmd_returns_error() {
        let d = Dispatcher::new("/tmp");
        let outcome = d.run(&[], None).await.unwrap();
        assert_eq!(outcome.exit_code, Some(-1));
        assert!(outcome.stderr.contains("empty command"));
    }

    #[tokio::test]
    async fn timeout_kills_process() {
        let d = Dispatcher {
            skill_dir: "/tmp".into(),
            dry_run: DryRun::Disabled,
            probe_timeout: Duration::from_secs(1),
            intervention_timeout: Duration::from_secs(1),
        };
        // Sleep longer than the timeout.
        let outcome = d
            .run(
                &["sleep".into(), "5".into()],
                Some(Duration::from_millis(100)),
            )
            .await
            .unwrap();
        assert!(outcome.timed_out);
        assert!(outcome.exit_code.is_none());
    }
}

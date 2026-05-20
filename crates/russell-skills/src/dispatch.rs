// SPDX-License-Identifier: MIT OR Apache-2.0
//! Subprocess dispatcher for Russell skills.
//!
//! Runs probe and intervention commands with env scrubbing, timeout,
//! stdout/stderr capture, dry-run support, and IDRS journaling.
//!
//! ## IDRS integration
//!
//! Every dispatch via [`Dispatcher::run_and_journal`] satisfies the
//! IDRS contract defined in
//! [`docs/standards/safety.md`](../../../docs/standards/safety.md):
//!
//! - **I (Idempotent):** Enforced per-intervention via manifest
//!   `idempotent: true` field.
//! - **D (Dry-run):** `--dry-run` flag writes `would_*` actions,
//!   never executes subprocess.
//! - **R (Rollback):** [`Dispatcher::run_intervention_with_rollback`]
//!   supports `rollback_id` (reverse intervention), `none_needed`,
//!   and `reboot` strategies.
//! - **S (Structured log):** Every dispatch writes a `harness.event.v1`
//!   record to the journal and an evidence bundle to disk.
//!
//! The dry-run path writes the event but does not execute
//! the subprocess. Rollback pre-state capture is the caller's
//! responsibility (dispatchers can't know what state to snapshot).

use std::path::{Path, PathBuf};
use std::time::Duration;

use russell_core::Result;
use russell_core::event::{Event, Severity};
use russell_core::journal::JournalWriter;
use russell_core::journal::port::JournalWritePort;
use tracing::{debug, warn};
use zeroize::Zeroize;

use crate::RiskBand;

/// Errors that can occur during risk checking.
#[derive(Debug, Clone, thiserror::Error)]
pub enum RiskError {
    /// The intervention's risk exceeds the auto-execute cap.
    #[error("risk {:?} exceeds max_auto_risk {:?}", risk, max_allowed)]
    RiskTooHigh {
        /// The intervention's declared risk.
        risk: RiskBand,
        /// The maximum risk the dispatcher may auto-run.
        max_allowed: RiskBand,
    },
}

/// A sudo password that zeroes its memory on drop.
pub struct SudoCredential {
    inner: String,
}

impl SudoCredential {
    /// Wrap a password string. Takes ownership to ensure no copies
    /// remain in the caller's scope.
    /// remain in the caller's scope.
    #[must_use]
    pub fn new(password: String) -> Self {
        Self { inner: password }
    }

    /// Borrow the credential for one-shot use (stdin piping).
    /// The reference is short-lived — caller must not store it.
    /// The reference is short-lived — caller must not store it.
    pub(crate) fn as_str(&self) -> &str {
        &self.inner
    }
}

impl Drop for SudoCredential {
    fn drop(&mut self) {
        self.inner.zeroize();
    }
}

impl std::fmt::Debug for SudoCredential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SudoCredential(***)")
    }
}

/// Controls whether subprocesses actually execute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DryRun {
    /// Print what would run, don't execute.
    Enabled,
    /// Execute normally.
    Disabled,
}

/// Outcome of a single subprocess run.
///
/// For rollback-protected interventions, the `rollback` field
/// contains the rollback run outcome (if rollback was triggered).
///
/// For rollback-protected interventions, the `rollback` field
/// contains the rollback run outcome (if rollback was triggered).
/// For rollback-protected interventions, the `rollback` field
/// contains the rollback run outcome (if rollback was triggered).
/// contains the rollback run outcome (if rollback was triggered).
#[derive(Debug, Clone, PartialEq)]
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
    /// Rollback outcome, if this was an intervention that failed
    /// and rollback was triggered. `None` for probes and successful
    /// interventions.
    /// and rollback was triggered. `None` for probes and successful
    /// interventions.
    /// interventions.
    pub rollback: Option<Box<RunOutcome>>,
}

impl RunOutcome {
    /// Whether the forward command succeeded (exit=0, no timeout).
    #[must_use]
    pub fn succeeded(&self) -> bool {
        self.exit_code == Some(0) && !self.timed_out
    }

    /// Whether rollback was applied.
    #[must_use]
    pub fn rollback_applied(&self) -> bool {
        self.rollback.is_some()
    }

    /// Whether the overall operation is safe — either forward succeeded,
    /// or forward failed but rollback succeeded.
    /// or forward failed but rollback succeeded.
    #[must_use]
    pub fn is_safe(&self) -> bool {
        if self.succeeded() {
            return true;
        }
        self.rollback
            .as_ref()
            .is_some_and(|r| r.exit_code == Some(0) && !r.timed_out)
    }
}

/// Rollback strategy as resolved by the manifest loader.
#[derive(Debug, Clone)]
pub enum RollbackStrategy {
    /// Roll back via a named intervention.
    RollbackId {
        /// The intervention ID to run in reverse.
        id: String,
    },
    /// No rollback needed (declared in manifest).
    NoneNeeded,
    /// Reboot required to undo (requires human confirmation).
    Reboot,
}

/// Backward-compatibility alias for code that previously used the
/// separate `RollbackOutcome` type. New code should use [`RunOutcome`]
/// directly — it now carries rollback information inline.
/// separate `RollbackOutcome` type. New code should use [`RunOutcome`]
/// directly — it now carries rollback information inline.
/// directly — it now carries rollback information inline.
#[deprecated(
    since = "0.2.0",
    note = "use RunOutcome directly; rollback is now an inline field"
)]
pub type RollbackOutcome = RunOutcome;

/// Whether a dispatch is a probe (read-only) or intervention (mutating).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepType {
    /// Read-only observation.
    Probe,
    /// Mutating action.
    Intervention,
}

impl StepType {
    /// Returns the step type as a lowercase string slice.
    pub fn as_str(self) -> &'static str {
        match self {
            StepType::Probe => "probe",
            StepType::Intervention => "intervention",
        }
    }
}

/// Input struct for [`Dispatcher::dispatch`] — the structured
pub struct DispatchRequest<'a> {
    /// Journal to write evidence events to.
    pub journal: &'a JournalWriter,
    /// Base directory for evidence bundles.
    pub evidence_base: &'a Path,
    /// Command to execute.
    pub cmd: &'a [String],
    /// Skill identifier (from manifest).
    pub skill_id: &'a str,
    /// Step identifier within the skill.
    pub step_id: &'a str,
    /// Whether this is a probe or intervention.
    pub step_type: StepType,
    /// Risk band string from the manifest.
    pub risk_band: &'a str,
    /// Optional timeout override.
    pub timeout: Option<Duration>,
}

/// A subprocess dispatcher. Constructed with the skills base directory
pub struct Dispatcher {
    /// Base skills directory (e.g. `~/.local/share/harness/skills/<id>/`).
    pub skill_dir: PathBuf,
    /// Global dry-run override.
    pub dry_run: DryRun,
    /// Default timeout for probes.
    pub probe_timeout: Duration,
    /// Default timeout for interventions.
    pub intervention_timeout: Duration,
    /// Maximum risk band that may be auto-executed.
    /// Interventions above this cap are refused with [`RiskError::RiskTooHigh`].
    /// Default: `Low`.
    /// Interventions above this cap are refused with [`RiskError::RiskTooHigh`].
    /// Default: `Low`.
    /// Default: `Low`.
    pub max_auto_risk: RiskBand,
    /// Sudo credential for interventions that need root privileges.
    /// Zeroed on drop. Set to `None` if no sudo interventions expected.
    /// Zeroed on drop. Set to `None` if no sudo interventions expected.
    pub sudo_password: Option<SudoCredential>,
    /// Arbitrary content to pipe to the subprocess stdin after sudo auth
    /// (if applicable). Used for interventions like `create-manifest` where
    /// the LLM produces content that must be piped to the CLI command.
    /// (if applicable). Used for interventions like `create-manifest` where
    /// the LLM produces content that must be piped to the CLI command.
    /// the LLM produces content that must be piped to the CLI command.
    pub stdin_content: Option<String>,
    /// Environment variables this skill is allowed to access.
    /// Combined with ENV_ALLOWLIST at dispatch time.
    /// Task 3.1: Capability attenuation.
    /// Combined with ENV_ALLOWLIST at dispatch time.
    /// Task 3.1: Capability attenuation.
    /// Task 3.1: Capability attenuation.
    pub allowed_env_keys: Vec<String>,
}

impl std::fmt::Debug for Dispatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Dispatcher")
            .field("skill_dir", &self.skill_dir)
            .field("dry_run", &self.dry_run)
            .field("probe_timeout", &self.probe_timeout)
            .field("intervention_timeout", &self.intervention_timeout)
            .field("max_auto_risk", &self.max_auto_risk)
            .field("sudo_password", &self.sudo_password.as_ref().map(|_| "***"))
            .field(
                "stdin_content",
                &self
                    .stdin_content
                    .as_ref()
                    .map(|c| format!("{} bytes", c.len())),
            )
            .finish()
    }
}

/// Errors from command path validation.
#[derive(Debug, Clone, thiserror::Error)]
pub enum CommandPathError {
    /// Command uses a bare name that would be resolved via PATH.
    /// Skill commands must use relative (`./scripts/foo.sh`) or
    /// absolute (`/usr/bin/python3`) paths.
    /// Skill commands must use relative (`./scripts/foo.sh`) or
    /// absolute (`/usr/bin/python3`) paths.
    /// absolute (`/usr/bin/python3`) paths.
    #[error("bare command name {name:?} rejected — use relative ./scripts/ path or absolute path")]
    BareCommand {
        /// The rejected command name.
        name: String,
    },
    /// Command attempts path traversal outside the skill directory.
    #[error("path traversal in command: {path:?}")]
    PathTraversal {
        /// The path containing traversal.
        path: String,
    },
}

/// Validate that a command path is acceptable for skill execution.
pub fn validate_command_path(cmd: &str) -> std::result::Result<(), CommandPathError> {
    // Allow known interpreters that are part of the subprocess contract.
    const ALLOWED_INTERPRETERS: &[&str] =
        &["sh", "bash", "dash", "python3", "python", "perl", "ruby"];

    if ALLOWED_INTERPRETERS.contains(&cmd) {
        return Ok(());
    }

    // Absolute paths are acceptable (operator controls the manifest).
    if cmd.starts_with('/') {
        return Ok(());
    }

    // Relative paths starting with ./ are acceptable.
    if cmd.starts_with("./") {
        // Check for traversal.
        if cmd.contains("/../") || cmd.ends_with("/..") {
            return Err(CommandPathError::PathTraversal {
                path: cmd.to_string(),
            });
        }
        return Ok(());
    }

    // Anything else is a bare command name — reject.
    Err(CommandPathError::BareCommand {
        name: cmd.to_string(),
    })
}

impl Dispatcher {
    /// Environment variables safe to propagate to skill subprocesses.
    const ENV_ALLOWLIST: [&str; 6] = ["HOME", "USER", "LANG", "LC_ALL", "TERM", "SHELL"];

    /// Create a new dispatcher for a given skill.
    #[must_use]
    pub fn new(skill_dir: impl Into<PathBuf>) -> Self {
        Self {
            skill_dir: skill_dir.into(),
            dry_run: DryRun::Disabled,
            probe_timeout: Duration::from_secs(30),
            intervention_timeout: Duration::from_secs(120),
            max_auto_risk: RiskBand::Low,
            sudo_password: None,
            stdin_content: None,
            allowed_env_keys: Vec::new(),
        }
    }

    /// Check whether an intervention at the given risk band may auto-execute.
    pub fn check_risk(&self, risk: RiskBand, dry_run: bool) -> std::result::Result<(), RiskError> {
        if dry_run {
            return Ok(());
        }
        if risk > self.max_auto_risk {
            warn!(
                risk = ?risk,
                max_auto_risk = ?self.max_auto_risk,
                "intervention blocked: risk exceeds auto cap"
            );
            return Err(RiskError::RiskTooHigh {
                risk,
                max_allowed: self.max_auto_risk,
            });
        }
        Ok(())
    }

    /// Run a command and return its output.
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
                rollback: None,
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
                rollback: None,
            });
        }

        // Validate command path — reject bare names that would
        // resolve via PATH (prevents command injection via manifest).
        if let Err(e) = validate_command_path(&cmd[0]) {
            warn!(cmd = %cmd[0], error = %e, "command path validation failed");
            return Ok(RunOutcome {
                cmd: cmd.to_vec(),
                dry_run: false,
                exit_code: Some(-1),
                stdout: String::new(),
                stderr: format!("command rejected: {e}"),
                timed_out: false,
                duration: started.elapsed(),
                rollback: None,
            });
        }

        // Build the command — possibly with sudo wrapper.
        let (program, args) = self.build_command(cmd);

        let mut child_cmd = tokio::process::Command::new(&program);
        child_cmd
            .args(&args)
            .current_dir(&self.skill_dir)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);

        // --- ENV SCRUBBING (T11) ---
        // Clear inherited env to prevent leaking secrets (API keys,
        // RUSSELL_* internals, .env contents) to skill subprocesses.
        // Only a minimal allowlist is propagated.
        child_cmd.env_clear();
        // Combine default allowlist with skill-specific allowed_env_keys.
        let mut combined_allowlist: Vec<&str> = Self::ENV_ALLOWLIST.to_vec();
        for key in &self.allowed_env_keys {
            combined_allowlist.push(key.as_str());
        }
        for key in &combined_allowlist {
            if let Ok(val) = std::env::var(key) {
                child_cmd.env(key, val);
            }
        }
        // Force a restrictive PATH — no user bin dirs unless the
        // operator explicitly overrides via manifest `env:` (future).
        child_cmd.env("PATH", "/usr/local/bin:/usr/bin:/bin");

        let mut child = child_cmd
            .spawn()
            .map_err(|e| russell_core::CoreError::io(&self.skill_dir, e))?;

        // Pipe sudo password if present. The credential is borrowed
        // (not cloned) to avoid extra copies in memory.
        if let Some(ref credential) = self.sudo_password
            && let Some(mut stdin) = child.stdin.take()
        {
            use tokio::io::AsyncWriteExt;
            let pw = credential.as_str();
            if let Err(e) = stdin.write_all(pw.as_bytes()).await {
                warn!(error = %e, "failed to write sudo password to stdin");
            }
            if let Err(e) = stdin.write_all(b"\n").await {
                warn!(error = %e, "failed to write newline after sudo password");
            }
            drop(stdin);
        }

        // Pipe arbitrary stdin content (used for interventions like
        // create-manifest where the LLM produces content). Only used
        // when sudo is NOT set — sudo stdin piping is exclusive.
        if self.sudo_password.is_none()
            && let Some(ref content) = self.stdin_content
            && let Some(mut stdin) = child.stdin.take()
        {
            use tokio::io::AsyncWriteExt;
            if let Err(e) = stdin.write_all(content.as_bytes()).await {
                warn!(error = %e, "failed to write stdin content");
            }
            drop(stdin);
        }

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
                rollback: None,
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
                    stderr: format!("timed out after {timeout:?}"),
                    timed_out: true,
                    duration: started.elapsed(),
                    rollback: None,
                })
            }
        }
    }

    /// Build the actual program + args, wrapping in `sudo -S` if needed.
    fn build_command(&self, cmd: &[String]) -> (String, Vec<String>) {
        if self.sudo_password.is_some() {
            let mut args = vec!["-S".to_string(), "--".to_string()];
            args.push(cmd[0].clone());
            if cmd.len() > 1 {
                args.extend_from_slice(&cmd[1..]);
            }
            ("sudo".to_string(), args)
        } else {
            let program = cmd[0].clone();
            let args: Vec<String> = if cmd.len() > 1 {
                cmd[1..].to_vec()
            } else {
                vec![]
            };
            (program, args)
        }
    }

    /// Run a command and journal the result.
    #[allow(clippy::too_many_arguments)]
    pub async fn run_and_journal(
        &self,
        journal: &JournalWriter,
        evidence_base: &Path,
        cmd: &[String],
        skill_id: &str,
        step_id: &str,
        step_type: StepType,
        risk_band: &str,
        timeout_override: Option<Duration>,
    ) -> Result<RunOutcome> {
        let outcome = match self.run(cmd, timeout_override).await {
            Ok(o) => o,
            Err(e) => {
                // Spawn failure — journal a failure event with no run outcome.
                let mut ev = Event::new("skill_intervention", Severity::Warn);
                ev.tier = Some("skill".into());
                ev.module = Some(format!("skill/{skill_id}/{step_id}"));
                ev.dry_run = false;
                ev.summary = Some(format!("spawn failed: {e} :: skill/{skill_id}/{step_id}",));
                ev.outputs.insert("risk".into(), risk_band.into());
                ev.outputs
                    .insert("step_type".into(), step_type.as_str().into());
                if let Err(je) = journal.append(&ev) {
                    tracing::warn!(error = %je, "failed to journal spawn failure");
                }
                return Err(e);
            }
        };

        let action = match step_type {
            StepType::Probe => "skill_probe",
            StepType::Intervention => {
                if outcome.dry_run {
                    "would_skill_intervention"
                } else {
                    "skill_intervention"
                }
            }
        };

        let severity = if outcome.exit_code == Some(0) || outcome.dry_run {
            Severity::Info
        } else {
            Severity::Warn
        };

        let mut ev = Event::new(action, severity);
        ev.tier = Some("skill".into());
        ev.module = Some(format!("skill/{skill_id}/{step_id}"));
        ev.dry_run = outcome.dry_run;
        ev.duration_ms = Some(outcome.duration.as_millis() as u64);
        ev.summary = Some(format!(
            "{} {}/{}{}",
            if outcome.dry_run { "[DRY RUN]" } else { "" },
            skill_id,
            step_id,
            match outcome.exit_code {
                Some(c) => format!(" exit={c}"),
                None if outcome.timed_out => " TIMEOUT".into(),
                None => " (no exit code)".into(),
            },
        ));
        ev.outputs
            .insert("exit_code".into(), outcome.exit_code.into());
        ev.outputs
            .insert("timed_out".into(), outcome.timed_out.into());
        ev.outputs.insert("risk".into(), risk_band.into());
        ev.outputs
            .insert("step_type".into(), step_type.as_str().into());

        // Write evidence bundle.
        let evidence_dir = evidence_base
            .join("skills")
            .join(skill_id)
            .join(step_id)
            .join(russell_core::time::now_rfc3339().replace(':', "-"));
        if let Err(e) = write_evidence(&evidence_dir, &outcome, &ev) {
            tracing::warn!(dir = %evidence_dir.display(), error = %e, "failed to write evidence bundle");
        }
        ev.evidence_ref = Some(evidence_dir.display().to_string());

        journal.append(&ev)?;

        debug!(
            skill_id,
            step_id,
            exit_code = ?outcome.exit_code,
            dry_run = outcome.dry_run,
            duration_ms = %outcome.duration.as_millis(),
            "dispatched skill step",
        );

        Ok(outcome)
    }

    /// Port-based variant of [`run_and_journal`] — accepts any
    #[allow(clippy::too_many_arguments)]
    pub async fn run_and_journal_port(
        &self,
        journal: &dyn JournalWritePort,
        evidence_base: &Path,
        cmd: &[String],
        skill_id: &str,
        step_id: &str,
        step_type: StepType,
        risk_band: &str,
        timeout_override: Option<Duration>,
    ) -> Result<RunOutcome> {
        let outcome = match self.run(cmd, timeout_override).await {
            Ok(o) => o,
            Err(e) => {
                let mut ev = Event::new("skill_intervention", Severity::Warn);
                ev.tier = Some("skill".into());
                ev.module = Some(format!("skill/{skill_id}/{step_id}"));
                ev.summary = Some(format!("spawn failed: {e} :: skill/{skill_id}/{step_id}"));
                ev.outputs.insert("risk".into(), risk_band.into());
                ev.outputs
                    .insert("step_type".into(), step_type.as_str().into());
                let _ = journal.append(&ev);
                return Err(e);
            }
        };

        let action = match step_type {
            StepType::Probe => "skill_probe",
            StepType::Intervention => {
                if outcome.dry_run {
                    "would_skill_intervention"
                } else {
                    "skill_intervention"
                }
            }
        };
        let severity = if outcome.exit_code == Some(0) || outcome.dry_run {
            Severity::Info
        } else {
            Severity::Warn
        };

        let mut ev = Event::new(action, severity);
        ev.tier = Some("skill".into());
        ev.module = Some(format!("skill/{skill_id}/{step_id}"));
        ev.dry_run = outcome.dry_run;
        ev.duration_ms = Some(outcome.duration.as_millis() as u64);
        ev.summary = Some(format!(
            "{} {}/{}{}",
            if outcome.dry_run { "[DRY RUN]" } else { "" },
            skill_id,
            step_id,
            match outcome.exit_code {
                Some(c) => format!(" exit={c}"),
                None if outcome.timed_out => " TIMEOUT".into(),
                None => " (no exit code)".into(),
            },
        ));
        ev.outputs
            .insert("exit_code".into(), outcome.exit_code.into());
        ev.outputs
            .insert("timed_out".into(), outcome.timed_out.into());
        ev.outputs.insert("risk".into(), risk_band.into());
        ev.outputs
            .insert("step_type".into(), step_type.as_str().into());

        let evidence_dir = evidence_base
            .join("skills")
            .join(skill_id)
            .join(step_id)
            .join(russell_core::time::now_rfc3339().replace(':', "-"));
        if let Err(e) = write_evidence(&evidence_dir, &outcome, &ev) {
            tracing::warn!(dir = %evidence_dir.display(), error = %e, "evidence write failed");
        }
        ev.evidence_ref = Some(evidence_dir.display().to_string());

        journal.append(&ev)?;
        Ok(outcome)
    }

    /// Run a skill step with journaling, using a structured request.
    pub async fn dispatch(&self, req: &DispatchRequest<'_>) -> Result<RunOutcome> {
        self.run_and_journal(
            req.journal,
            req.evidence_base,
            req.cmd,
            req.skill_id,
            req.step_id,
            req.step_type,
            req.risk_band,
            req.timeout,
        )
        .await
    }

    /// Run an intervention with automatic rollback on failure.
    #[allow(clippy::too_many_arguments)]
    pub async fn run_intervention_with_rollback<F>(
        &self,
        journal: &JournalWriter,
        evidence_base: &Path,
        skill_id: &str,
        step_id: &str,
        cmd: &[String],
        risk_band: &str,
        rollback: RollbackStrategy,
        get_rollback_cmd: F,
        timeout_override: Option<Duration>,
    ) -> Result<RunOutcome>
    where
        F: FnOnce(&str) -> Option<Vec<String>>,
    {
        // Forward run.
        let mut forward = self
            .run_and_journal(
                journal,
                evidence_base,
                cmd,
                skill_id,
                step_id,
                StepType::Intervention,
                risk_band,
                timeout_override,
            )
            .await?;

        if forward.succeeded() {
            return Ok(forward);
        }

        // Rollback needed.
        match rollback {
            RollbackStrategy::NoneNeeded => {
                debug!(
                    skill_id,
                    step_id, "intervention failed; rollback: none_needed — no action"
                );
                Ok(forward)
            }
            RollbackStrategy::Reboot => {
                warn!(
                    skill_id,
                    step_id, "intervention failed; rollback: reboot required"
                );
                Ok(forward)
            }
            RollbackStrategy::RollbackId { ref id } => {
                let rollback_cmd = match get_rollback_cmd(id) {
                    Some(cmd) => cmd,
                    None => {
                        warn!(
                            skill_id,
                            step_id,
                            rollback_id = %id,
                            "rollback command not found — rollback skipped"
                        );
                        return Ok(forward);
                    }
                };

                warn!(
                    skill_id,
                    step_id,
                    rollback_id = %id,
                    exit_code = ?forward.exit_code,
                    "intervention failed — running rollback"
                );

                let rollback_outcome = self
                    .run_and_journal(
                        journal,
                        evidence_base,
                        &rollback_cmd,
                        skill_id,
                        id,
                        StepType::Intervention,
                        risk_band,
                        timeout_override,
                    )
                    .await?;

                forward.rollback = Some(Box::new(rollback_outcome));
                Ok(forward)
            }
        }
    }
}

/// Convert a manifest [`Rollback`](crate::Rollback) to a dispatcher [`RollbackStrategy`].
///
/// The dispatcher doesn't own manifests, so the caller resolves the
/// strategy before calling `run_intervention_with_rollback`.
///
/// The dispatcher doesn't own manifests, so the caller resolves the
/// strategy before calling `run_intervention_with_rollback`.
/// The dispatcher doesn't own manifests, so the caller resolves the
/// strategy before calling `run_intervention_with_rollback`.
/// strategy before calling `run_intervention_with_rollback`.
#[must_use]
pub fn resolve_rollback_strategy(rollback: &crate::Rollback) -> RollbackStrategy {
    match rollback {
        crate::Rollback::RollbackId { rollback_id } => RollbackStrategy::RollbackId {
            id: rollback_id.clone(),
        },
        crate::Rollback::NoneNeeded { .. } => RollbackStrategy::NoneNeeded,
        crate::Rollback::Reboot { .. } => RollbackStrategy::Reboot,
    }
}

/// Write stdout, stderr, and event JSON to the evidence directory.
///
/// Task 3.3: Evidence bundle sealing — computes SHA-256 hashes of all
/// evidence files and writes a manifest.json for tamper detection.
///
/// Task 3.3: Evidence bundle sealing — computes SHA-256 hashes of all
/// evidence files and writes a manifest.json for tamper detection.
/// Task 3.3: Evidence bundle sealing — computes SHA-256 hashes of all
/// evidence files and writes a manifest.json for tamper detection.
/// evidence files and writes a manifest.json for tamper detection.
fn write_evidence(dir: &Path, outcome: &RunOutcome, event: &Event) -> Result<()> {
    use sha2::{Digest, Sha256};

    std::fs::create_dir_all(dir).map_err(|e| russell_core::CoreError::io(dir, e))?;

    // Write stdout and compute hash.
    std::fs::write(dir.join("stdout.txt"), &outcome.stdout)
        .map_err(|e| russell_core::CoreError::io(dir, e))?;
    let mut hasher = Sha256::new();
    hasher.update(&outcome.stdout);
    let stdout_hash = hex::encode(hasher.finalize());

    // Write stderr and compute hash.
    let stderr_hash = if !outcome.stderr.is_empty() {
        std::fs::write(dir.join("stderr.txt"), &outcome.stderr)
            .map_err(|e| russell_core::CoreError::io(dir, e))?;
        let mut hasher = Sha256::new();
        hasher.update(&outcome.stderr);
        hex::encode(hasher.finalize())
    } else {
        String::new()
    };

    // Add hashes to event outputs for journal audit trail.
    let mut event_with_hashes = event.clone();
    event_with_hashes
        .outputs
        .insert("stdout_sha256".into(), stdout_hash.clone().into());
    if !stderr_hash.is_empty() {
        event_with_hashes
            .outputs
            .insert("stderr_sha256".into(), stderr_hash.clone().into());
    }

    // Write event JSON.
    let event_json = serde_json::to_string_pretty(&event_with_hashes)?;
    std::fs::write(dir.join("event.json"), event_json)
        .map_err(|e| russell_core::CoreError::io(dir, e))?;

    // Write manifest.json with file hashes and timestamp (Task 3.3).
    let manifest = serde_json::json!({
        "version": "1.0",
        "created_at": russell_core::time::now_rfc3339(),
        "files": {
            "stdout.txt": {
                "sha256": stdout_hash,
                "size_bytes": outcome.stdout.len()
            },
            "stderr.txt": {
                "sha256": stderr_hash,
                "size_bytes": outcome.stderr.len()
            },
            "event.json": {
                "note": "Self-hash not included"
            }
        }
    });
    let manifest_json = serde_json::to_string_pretty(&manifest)?;
    std::fs::write(dir.join("manifest.json"), manifest_json)
        .map_err(|e| russell_core::CoreError::io(dir, e))?;

    Ok(())
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
            max_auto_risk: RiskBand::Low,
            sudo_password: None,
            stdin_content: None,
            allowed_env_keys: Vec::new(),
        };
        // Dry-run skips validation — any command shape accepted.
        let outcome = d
            .run(&["/bin/echo".into(), "hello".into()], None)
            .await
            .unwrap();
        assert!(outcome.dry_run);
        assert!(outcome.exit_code.is_none());
        assert!(outcome.stdout.is_empty());
    }

    #[tokio::test]
    async fn echo_produces_stdout() {
        let d = Dispatcher::new("/tmp");
        let outcome = d
            .run(&["/bin/echo".into(), "hello".into()], None)
            .await
            .unwrap();
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
    async fn bare_command_rejected() {
        let d = Dispatcher::new("/tmp");
        let outcome = d
            .run(&["curl".into(), "http://evil.com".into()], None)
            .await
            .unwrap();
        assert_eq!(outcome.exit_code, Some(-1));
        assert!(outcome.stderr.contains("command rejected"));
    }

    #[tokio::test]
    async fn path_traversal_rejected() {
        let d = Dispatcher::new("/tmp");
        let outcome = d
            .run(&["./scripts/../../../etc/passwd".into()], None)
            .await
            .unwrap();
        assert_eq!(outcome.exit_code, Some(-1));
        assert!(outcome.stderr.contains("path traversal"));
    }

    #[tokio::test]
    async fn timeout_kills_process() {
        let d = Dispatcher {
            skill_dir: "/tmp".into(),
            dry_run: DryRun::Disabled,
            probe_timeout: Duration::from_secs(1),
            intervention_timeout: Duration::from_secs(1),
            max_auto_risk: RiskBand::Low,
            sudo_password: None,
            stdin_content: None,
            allowed_env_keys: Vec::new(),
        };
        // Sleep longer than the timeout — uses allowed interpreter `sh`.
        let outcome = d
            .run(
                &["sh".into(), "-c".into(), "sleep 5".into()],
                Some(Duration::from_millis(100)),
            )
            .await
            .unwrap();
        assert!(outcome.timed_out);
        assert!(outcome.exit_code.is_none());
    }

    // --- IDRS journaling tests ---

    #[tokio::test]
    async fn run_and_journal_writes_event_and_evidence() {
        let tmp = tempfile::tempdir().unwrap();
        let journal_path = tmp.path().join("journal.db");
        let journal = JournalWriter::open(&journal_path).unwrap();
        let evidence_base = tmp.path().join("evidence");

        let d = Dispatcher::new("/tmp");
        let outcome = d
            .run_and_journal(
                &journal,
                &evidence_base,
                &["/bin/echo".into(), "hello".into()],
                "test-skill",
                "probe-1",
                StepType::Probe,
                "none",
                None,
            )
            .await
            .unwrap();

        assert_eq!(outcome.exit_code, Some(0));
        assert!(!outcome.dry_run);

        // Journal should have the event.
        let reader = journal.reader();
        let rows = reader.recent(5).unwrap();
        assert!(!rows.is_empty());
        let row = &rows[0];
        assert_eq!(row.action, "skill_probe");
        assert_eq!(row.severity, russell_core::event::Severity::Info);

        // Evidence bundle should exist.
        let evidence_root = std::fs::read_dir(evidence_base.join("skills/test-skill/probe-1"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        assert!(evidence_root.join("stdout.txt").exists());
        assert!(evidence_root.join("event.json").exists());

        // stdout should contain "hello".
        let stdout = std::fs::read_to_string(evidence_root.join("stdout.txt")).unwrap();
        assert!(stdout.contains("hello"));
    }

    #[tokio::test]
    async fn dry_run_journals_would_action() {
        let tmp = tempfile::tempdir().unwrap();
        let journal_path = tmp.path().join("journal.db");
        let journal = JournalWriter::open(&journal_path).unwrap();
        let evidence_base = tmp.path().join("evidence");

        let d = Dispatcher {
            skill_dir: "/tmp".into(),
            dry_run: DryRun::Enabled,
            probe_timeout: Duration::from_secs(5),
            intervention_timeout: Duration::from_secs(5),
            max_auto_risk: RiskBand::Low,
            sudo_password: None,
            stdin_content: None,
            allowed_env_keys: Vec::new(),
        };
        let outcome = d
            .run_and_journal(
                &journal,
                &evidence_base,
                &["/bin/echo".into(), "would-have-run".into()],
                "test-skill",
                "iv-1",
                StepType::Intervention,
                "low",
                None,
            )
            .await
            .unwrap();

        assert!(outcome.dry_run);

        let reader = journal.reader();
        let rows = reader.recent(5).unwrap();
        assert_eq!(rows[0].action, "would_skill_intervention");
    }

    // --- Risk-band enforcement tests ---

    #[test]
    fn low_risk_allowed_at_low_cap() {
        let d = Dispatcher {
            skill_dir: "/tmp".into(),
            dry_run: DryRun::Disabled,
            probe_timeout: Duration::from_secs(5),
            intervention_timeout: Duration::from_secs(5),
            max_auto_risk: RiskBand::Low,
            sudo_password: None,
            stdin_content: None,
            allowed_env_keys: Vec::new(),
        };
        assert!(d.check_risk(RiskBand::None, false).is_ok());
        assert!(d.check_risk(RiskBand::Low, false).is_ok());
        assert!(d.check_risk(RiskBand::Medium, false).is_err());
        assert!(d.check_risk(RiskBand::High, false).is_err());
        assert!(d.check_risk(RiskBand::Critical, false).is_err());
    }

    #[test]
    fn dry_run_bypasses_risk_cap() {
        let d = Dispatcher {
            skill_dir: "/tmp".into(),
            dry_run: DryRun::Disabled,
            probe_timeout: Duration::from_secs(5),
            intervention_timeout: Duration::from_secs(5),
            max_auto_risk: RiskBand::Low,
            sudo_password: None,
            stdin_content: None,
            allowed_env_keys: Vec::new(),
        };
        // High risk in dry-run mode should still pass — no mutation occurs.
        assert!(d.check_risk(RiskBand::Critical, true).is_ok());
    }

    #[test]
    fn medium_risk_allowed_at_medium_cap() {
        let d = Dispatcher {
            skill_dir: "/tmp".into(),
            dry_run: DryRun::Disabled,
            probe_timeout: Duration::from_secs(5),
            intervention_timeout: Duration::from_secs(5),
            max_auto_risk: RiskBand::Medium,
            sudo_password: None,
            stdin_content: None,
            allowed_env_keys: Vec::new(),
        };
        assert!(d.check_risk(RiskBand::Medium, false).is_ok());
        assert!(d.check_risk(RiskBand::High, false).is_err());
    }

    #[test]
    fn risk_too_high_error_is_descriptive() {
        let d = Dispatcher {
            skill_dir: "/tmp".into(),
            dry_run: DryRun::Disabled,
            probe_timeout: Duration::from_secs(5),
            intervention_timeout: Duration::from_secs(5),
            max_auto_risk: RiskBand::Low,
            sudo_password: None,
            stdin_content: None,
            allowed_env_keys: Vec::new(),
        };
        let err = d.check_risk(RiskBand::High, false).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("High"));
        assert!(msg.contains("Low"));
    }

    // --- Rollback tests ---

    #[tokio::test]
    async fn rollback_runs_on_forward_failure() {
        let tmp = tempfile::tempdir().unwrap();
        let journal_path = tmp.path().join("journal.db");
        let journal = JournalWriter::open(&journal_path).unwrap();
        let evidence_base = tmp.path().join("evidence");

        let d = Dispatcher::new("/tmp");

        // Forward: a command that exits non-zero.
        // Rollback: echo "rolled back".
        let outcome = d
            .run_intervention_with_rollback(
                &journal,
                &evidence_base,
                "test-skill",
                "iv-bad",
                &["sh".into(), "-c".into(), "exit 1".into()],
                "low",
                RollbackStrategy::RollbackId {
                    id: "iv-revert".into(),
                },
                |_id| Some(vec!["/bin/echo".into(), "rolled-back".into()]),
                None,
            )
            .await
            .unwrap();

        // Forward failed.
        assert_eq!(outcome.exit_code, Some(1));
        // Rollback was applied.
        assert!(outcome.rollback_applied());
        let rb = outcome.rollback.unwrap();
        assert_eq!(rb.exit_code, Some(0));
        assert!(rb.stdout.contains("rolled-back"));
    }

    #[tokio::test]
    async fn no_rollback_when_forward_succeeds() {
        let tmp = tempfile::tempdir().unwrap();
        let journal_path = tmp.path().join("journal.db");
        let journal = JournalWriter::open(&journal_path).unwrap();
        let evidence_base = tmp.path().join("evidence");

        let d = Dispatcher::new("/tmp");

        let outcome = d
            .run_intervention_with_rollback(
                &journal,
                &evidence_base,
                "test-skill",
                "iv-good",
                &["/bin/echo".into(), "ok".into()],
                "low",
                RollbackStrategy::NoneNeeded,
                |_id| None,
                None,
            )
            .await
            .unwrap();

        assert_eq!(outcome.exit_code, Some(0));
        assert!(!outcome.rollback_applied());
        assert!(outcome.rollback.is_none());
        assert!(outcome.is_safe());
    }

    #[tokio::test]
    async fn none_needed_skips_rollback() {
        let tmp = tempfile::tempdir().unwrap();
        let journal_path = tmp.path().join("journal.db");
        let journal = JournalWriter::open(&journal_path).unwrap();
        let evidence_base = tmp.path().join("evidence");

        let d = Dispatcher::new("/tmp");

        let outcome = d
            .run_intervention_with_rollback(
                &journal,
                &evidence_base,
                "test-skill",
                "iv-restart",
                &["sh".into(), "-c".into(), "exit 2".into()],
                "low",
                RollbackStrategy::NoneNeeded,
                |_id| None,
                None,
            )
            .await
            .unwrap();

        // Forward failed but rollback is none_needed.
        assert_eq!(outcome.exit_code, Some(2));
        assert!(!outcome.rollback_applied());
        assert!(outcome.rollback.is_none());
    }

    #[tokio::test]
    async fn reboot_skips_rollback_and_warns() {
        let tmp = tempfile::tempdir().unwrap();
        let journal_path = tmp.path().join("journal.db");
        let journal = JournalWriter::open(&journal_path).unwrap();
        let evidence_base = tmp.path().join("evidence");

        let d = Dispatcher::new("/tmp");

        let outcome = d
            .run_intervention_with_rollback(
                &journal,
                &evidence_base,
                "test-skill",
                "iv-reboot",
                &["sh".into(), "-c".into(), "exit 3".into()],
                "high",
                RollbackStrategy::Reboot,
                |_id| None,
                None,
            )
            .await
            .unwrap();

        assert_eq!(outcome.exit_code, Some(3));
        assert!(!outcome.rollback_applied());
        assert!(outcome.rollback.is_none());
    }

    #[test]
    fn resolve_rollback_strategy_converts_correctly() {
        use crate::{Rollback, RollbackNone, RollbackReboot};

        let with_id = Rollback::RollbackId {
            rollback_id: "revert-id".into(),
        };
        if let RollbackStrategy::RollbackId { id } = resolve_rollback_strategy(&with_id) {
            assert_eq!(id, "revert-id");
        } else {
            panic!("expected RollbackId");
        }

        let none_needed = Rollback::NoneNeeded {
            rollback: RollbackNone::NoneNeeded,
        };
        assert!(matches!(
            resolve_rollback_strategy(&none_needed),
            RollbackStrategy::NoneNeeded
        ));

        let reboot = Rollback::Reboot {
            rollback: RollbackReboot::Reboot,
        };
        assert!(matches!(
            resolve_rollback_strategy(&reboot),
            RollbackStrategy::Reboot
        ));
    }
}

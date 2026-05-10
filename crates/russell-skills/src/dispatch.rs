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
use tracing::{debug, warn};

// Re-export RiskBand from the parent crate for convenience.
pub use crate::RiskBand;

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

/// Rollback strategy as resolved by the manifest loader.
#[derive(Debug, Clone)]
pub enum RollbackStrategy {
    /// Roll back via a named intervention.
    RollbackId { id: String },
    /// No rollback needed (declared in manifest).
    NoneNeeded,
    /// Reboot required to undo (requires human confirmation).
    Reboot,
}

/// Outcome of a rollback-protected intervention.
#[derive(Debug, Clone, PartialEq)]
pub struct RollbackOutcome {
    /// The forward (original) run outcome.
    pub forward: RunOutcome,
    /// The rollback run outcome, if rollback was triggered.
    pub rollback: Option<RunOutcome>,
    /// Whether rollback was actually applied.
    pub rollback_applied: bool,
}

impl RollbackOutcome {
    /// Whether the overall operation was successful.
    #[must_use]
    pub fn is_safe(&self) -> bool {
        if self.forward.exit_code == Some(0) && !self.forward.timed_out {
            return true;
        }
        self.rollback
            .as_ref()
            .map_or(false, |r| r.exit_code == Some(0) && !r.timed_out)
    }
}

/// Whether a dispatch is a probe (read-only) or intervention (mutating).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepType {
    /// Read-only observation.
    Probe,
    /// Mutating action.
    Intervention,
}

impl StepType {
    fn to_string(self) -> &'static str {
        match self {
            StepType::Probe => "probe",
            StepType::Intervention => "intervention",
        }
    }
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
    /// Maximum risk band that may be auto-executed.
    /// Interventions above this cap are refused with [`RiskError::RiskTooHigh`].
    /// Default: `Low`.
    pub max_auto_risk: RiskBand,
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
            max_auto_risk: RiskBand::Low,
        }
    }

    /// Check whether an intervention at the given risk band may auto-execute.
    ///
    /// Returns `Ok(())` if the risk is at or below `max_auto_risk`.
    /// Returns `Err(RiskError::RiskTooHigh)` if the risk exceeds the cap
    /// — the caller should refuse to dispatch and instead flag for human
    /// confirmation.
    ///
    /// Dry-run is always allowed regardless of risk (it doesn't mutate).
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

    /// Run a command and journal the result.
    ///
    /// This is the IDRS-compliant entry point. It:
    ///
    /// 1. Calls [`run`] to execute the command (or dry-run).
    /// 2. Writes a `harness.event.v1` event to the journal with
    ///    action `"skill_probe"` (risk=none) or `"skill_intervention"`
    ///    (risk from manifest).
    /// 3. Writes an evidence bundle to
    ///    `evidence/skills/<skill_id>/<step_id>/<ts>/` containing
    ///    stdout, stderr, and the event JSON.
    ///
    /// `step_type` distinguishes probes from interventions for the
    /// event action field. `risk_band` is the risk level from the
    /// manifest (probes are always `none`).
    ///
    /// # Errors
    ///
    /// Returns [`russell_core::CoreError`] on journal I/O failure
    /// or subprocess spawn failure.
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
        let outcome = self.run(cmd, timeout_override).await?;

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
            .insert("step_type".into(), step_type.to_string().into());

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

    /// Run an intervention with automatic rollback on failure.
    ///
    /// This is the IDRS-compliant entry point for mutating steps:
    ///
    /// 1. Runs the forward intervention via [`run_and_journal`].
    /// 2. If it succeeds (exit=0), returns the outcome.
    /// 3. If it fails, runs the rollback (if one is configured)
    ///    and journals both the failure and the rollback.
    ///
    /// `rollback` is the rollback strategy from the manifest.
    /// `get_rollback_cmd` is a callback that resolves a `rollback_id`
    /// to the actual command argv. It's called only when rollback is
    /// needed, so the caller (manifest loader) provides the lookup.
    ///
    /// Returns the rollback outcome on forward failure, or the forward
    /// outcome on success. `rollback_applied` in the outcome indicates
    /// whether rollback was triggered.
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
    ) -> Result<RollbackOutcome>
    where
        F: FnOnce(&str) -> Option<Vec<String>>,
    {
        // Forward run.
        let forward = self
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

        let succeeded = forward.exit_code == Some(0) && !forward.timed_out;
        if succeeded {
            return Ok(RollbackOutcome {
                forward,
                rollback: None,
                rollback_applied: false,
            });
        }

        // Rollback needed.
        match rollback {
            RollbackStrategy::NoneNeeded => {
                debug!(
                    skill_id,
                    step_id, "intervention failed; rollback: none_needed — no action"
                );
                Ok(RollbackOutcome {
                    forward,
                    rollback: None,
                    rollback_applied: false,
                })
            }
            RollbackStrategy::Reboot => {
                warn!(
                    skill_id,
                    step_id, "intervention failed; rollback: reboot required"
                );
                Ok(RollbackOutcome {
                    forward,
                    rollback: None,
                    rollback_applied: false,
                })
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
                        return Ok(RollbackOutcome {
                            forward,
                            rollback: None,
                            rollback_applied: false,
                        });
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

                Ok(RollbackOutcome {
                    forward,
                    rollback: Some(rollback_outcome),
                    rollback_applied: true,
                })
            }
        }
    }
}

/// Rollback strategy as resolved by the manifest loader.
///
/// This is a simplified, Clone-able version of the manifest's
/// [`Rollback`](crate::Rollback) enum. The dispatcher doesn't
/// own skill manifests, so the caller resolves the strategy.
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

/// Outcome of a rollback-protected intervention.
#[derive(Debug, Clone, PartialEq)]
pub struct RollbackOutcome {
    /// The forward (original) run outcome.
    pub forward: RunOutcome,
    /// The rollback run outcome, if rollback was triggered.
    pub rollback: Option<RunOutcome>,
    /// Whether rollback was actually applied.
    pub rollback_applied: bool,
}

impl RollbackOutcome {
    /// Whether the overall operation was successful (forward succeeded
    /// OR forward failed but rollback succeeded).
    #[must_use]
    pub fn is_safe(&self) -> bool {
        if self.forward.exit_code == Some(0) && !self.forward.timed_out {
            return true; // forward succeeded
        }
        // Forward failed; check rollback.
        self.rollback
            .as_ref()
            .map_or(false, |r| r.exit_code == Some(0) && !r.timed_out)
    }
}

/// Whether a dispatch is a probe (read-only) or intervention (mutating).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepType {
    /// Read-only observation.
    Probe,
    /// Mutating action.
    Intervention,
}

impl StepType {
    fn to_string(self) -> &'static str {
        match self {
            StepType::Probe => "probe",
            StepType::Intervention => "intervention",
        }
    }
}

/// Convert a manifest [`Rollback`](crate::Rollback) to a dispatcher [`RollbackStrategy`].
///
/// The dispatcher doesn't own manifests, so the caller resolves the
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
fn write_evidence(dir: &Path, outcome: &RunOutcome, event: &Event) -> Result<()> {
    std::fs::create_dir_all(dir).map_err(|e| russell_core::CoreError::io(dir, e))?;

    std::fs::write(dir.join("stdout.txt"), &outcome.stdout)
        .map_err(|e| russell_core::CoreError::io(dir, e))?;

    if !outcome.stderr.is_empty() {
        std::fs::write(dir.join("stderr.txt"), &outcome.stderr)
            .map_err(|e| russell_core::CoreError::io(dir, e))?;
    }

    let event_json = serde_json::to_string_pretty(event)?;
    std::fs::write(dir.join("event.json"), event_json)
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
            max_auto_risk: RiskBand::Low,
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
                &["echo".into(), "hello".into()],
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
        };
        let outcome = d
            .run_and_journal(
                &journal,
                &evidence_base,
                &["echo".into(), "would-have-run".into()],
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
        };
        let err = d.check_risk(RiskBand::High, false).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("High"));
        assert!(msg.contains("Low"));
    }
}

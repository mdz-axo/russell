// SPDX-License-Identifier: MIT OR Apache-2.0
//! Action execution for the chat REPL — local skill dispatch.
//!
//! Handles probes and interventions via the skill dispatcher with
//! IDRS compliance (journaling, rollback, evidence bundles).

use std::io::Write;

use russell_core::journal::{HelpSessionInput, HelpSessionStatus, JournalWriter};
use russell_core::paths::Paths;
use russell_meta::action::ResolvedAction;
use russell_skills::RiskBand;

use crate::commands::chat::PendingAction;

/// Execute a pending action (probe or intervention) via the skill dispatcher.
/// Returns a formatted result string suitable for injection into the LLM
/// conversation history, so Jack can see and interpret what happened.
pub async fn execute_pending_action(
    journal: &JournalWriter,
    paths: &Paths,
    pending: &PendingAction,
    session_id: &str,
    model: &str,
) -> Option<String> {
    use russell_skills::dispatch::{Dispatcher, DryRun, RollbackStrategy, StepType};
    use russell_skills::registry::RegistryCache;
    use std::io::Write;
    use std::time::Duration;

    let action = &pending.action;
    let skill_id = action.skill_id().to_string();
    let action_id = action.action_id().to_string();
    let skill_dir = paths.skills().join(&skill_id);
    let evidence_base = paths.evidence();

    let timeout = if action.is_probe() {
        Duration::from_secs(30)
    } else {
        Duration::from_secs(120)
    };

    let is_probe = action.is_probe();
    let is_shell = action.is_shell_command();
    let (
        risk,
        needs_sudo,
        max_auto_risk,
        requires_human,
        rollback_id,
        rollback_cmd,
        rollback_is_reboot,
    ) = match action {
        ResolvedAction::Probe { max_auto_risk, .. } => (
            RiskBand::None,
            false,
            *max_auto_risk,
            false,
            None,
            None,
            false,
        ),
        ResolvedAction::Intervention {
            risk,
            needs_sudo,
            max_auto_risk: _, // Ignored: consent already granted by operator.
            requires_human,
            rollback_id,
            rollback_cmd,
            rollback_is_reboot,
            ..
        } => (
            *risk,
            *needs_sudo,
            RiskBand::Critical, // Consent already granted; allow execution.
            *requires_human,
            rollback_id.clone(),
            rollback_cmd.clone(),
            *rollback_is_reboot,
        ),
        ResolvedAction::ShellCommand {
            risk, needs_sudo, ..
        } => (
            *risk,
            *needs_sudo,
            RiskBand::Critical, // Shell commands always require consent; once approved, allow execution.
            false,              // No requires_human flag
            None,               // No rollback for raw shell
            None,
            false,
        ),
        ResolvedAction::RemoteTool { .. } => (
            RiskBand::None, // Remote tools are read-only queries
            false,          // No sudo
            RiskBand::None, // max_auto_risk
            false,          // No requires_human
            None,           // No rollback
            None,
            false,
        ),
    };

    let mut dispatcher = Dispatcher::new(&skill_dir);
    // Task 3.1: Load skill to get allowed_env_keys for capability attenuation.
    if let Ok(skill) = russell_skills::load_single(&skill_dir) {
        dispatcher.allowed_env_keys = skill.safety.allowed_env_keys.clone();
    }
    dispatcher.intervention_timeout = timeout;
    dispatcher.probe_timeout = timeout;
    dispatcher.dry_run = DryRun::Disabled;
    dispatcher.max_auto_risk = max_auto_risk;
    dispatcher.stdin_content = pending.stdin_content.clone();

    // If this action requires explicit human confirmation, prompt again.
    if requires_human {
        print!(
            "  → This action is marked as requiring explicit human confirmation. Proceed? [y/N]: "
        );
        let _ = std::io::stdout().flush();
        let mut buf = String::new();
        if std::io::stdin().read_line(&mut buf).is_err() {
            println!("  → Could not read input. Aborting.");
            return None;
        }
        let answer = buf.trim().to_lowercase();
        if answer != "y" && answer != "yes" {
            println!("  → Aborted by operator.");
            return None;
        }
    }

    // Enforce risk cap — probes are always risk: none so this passes.
    if let Err(e) = dispatcher.check_risk(risk, false) {
        println!("  → Refused: {e}");
        return None;
    }

    // Prompt for sudo password if needed (secure terminal prompt, not CLI).
    // First check if sudo's credential cache is still valid (avoids re-prompting
    // the operator within the sudo timestamp_timeout window).
    let sudo_already_cached = if needs_sudo {
        tokio::process::Command::new("sudo")
            .args(["-n", "--", "true"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .await
            .is_ok_and(|s| s.success())
    } else {
        false
    };

    if needs_sudo && !sudo_already_cached {
        eprint!("  → Sudo password for this action: ");
        let _ = std::io::stderr().flush();
        let password = rpassword::read_password().unwrap_or_default();
        if password.is_empty() {
            println!("  → Empty password. Aborting action.");
            return None;
        }
        // Verify password by trying: sudo -S true
        use tokio::io::AsyncWriteExt;
        let mut verify = match tokio::process::Command::new("sudo")
            .args(["-S", "--", "true"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            Ok(child) => child,
            Err(_) => {
                println!("  → Could not run sudo. Aborting action.");
                return None;
            }
        };
        if let Some(ref mut stdin) = verify.stdin {
            let _ = stdin.write_all(format!("{password}\n").as_bytes()).await;
        }
        match verify.wait().await {
            Ok(s) if s.success() => {
                dispatcher.sudo_password =
                    Some(russell_skills::dispatch::SudoCredential::new(password));
                // Refresh the sudo timestamp so subsequent commands don't re-prompt
                // for at least the sudoers timestamp_timeout (typically 5-15 min).
                let _ = tokio::process::Command::new("sudo")
                    .args(["-v"])
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status()
                    .await;
            }
            _ => {
                println!("  → Wrong sudo password. Aborting action.");
                return None;
            }
        }
    } else if needs_sudo {
        // Sudo timestamp is already cached — use an empty credential as a marker
        // so the dispatcher still wraps commands with sudo (but doesn't pipe a password).
        dispatcher.sudo_password =
            Some(russell_skills::dispatch::SudoCredential::new(String::new()));
    }

    let step_type = if is_probe {
        StepType::Probe
    } else {
        StepType::Intervention
    };

    // Shell commands use direct execution, not the skill dispatcher.
    if is_shell {
        return execute_shell_command(journal, action, session_id, model, timeout).await;
    }

    if is_probe {
        // Probes: use run_and_journal (read-only, no rollback needed).
        let result = dispatcher
            .run_and_journal(
                journal,
                &evidence_base,
                &action.cmd(),
                &skill_id,
                &action_id,
                step_type,
                risk.as_str(),
                Some(timeout),
            )
            .await;

        // Update registry telemetry.
        let probe_success = result
            .as_ref()
            .is_ok_and(|o| o.exit_code == Some(0) && !o.timed_out);
        let probe_duration_ms = result
            .as_ref()
            .map(|o| o.duration.as_millis() as u64)
            .unwrap_or(0);
        let probe_error = result.as_ref().err().map(|e| e.to_string());
        let registry_path = paths.state.join("registry").join("local-cache.yaml");
        let _ = RegistryCache::with_update(&registry_path, |cache| {
            cache.record_execution(
                &skill_id,
                probe_success,
                probe_duration_ms,
                probe_error.as_deref(),
            );
        });

        return format_probe_result(result, &skill_id, &action_id);
    }

    // Interventions: use run_intervention_with_rollback for IDRS-R compliance.
    let rollback_strategy = if rollback_is_reboot {
        RollbackStrategy::Reboot
    } else if let Some(ref rid) = rollback_id {
        RollbackStrategy::RollbackId { id: rid.clone() }
    } else {
        RollbackStrategy::NoneNeeded
    };

    let rollback_cmd_owned = rollback_cmd.clone();
    let rollback_outcome = dispatcher
        .run_intervention_with_rollback(
            journal,
            &evidence_base,
            &skill_id,
            &action_id,
            &action.cmd(),
            risk.as_str(),
            rollback_strategy,
            |_rb_id| rollback_cmd_owned.clone(),
            Some(timeout),
        )
        .await;

    match rollback_outcome {
        Ok(outcome) => {
            let success = outcome.succeeded();
            let error_msg = if !success {
                Some(format!(
                    "exit {:?}, stderr: {}",
                    outcome.exit_code,
                    outcome.stderr.trim()
                ))
            } else {
                None
            };
            let registry_path = paths.state.join("registry").join("local-cache.yaml");
            let _ = RegistryCache::with_update(&registry_path, |cache| {
                cache.record_intervention(&skill_id, success, error_msg.as_deref());
            });

            if success {
                println!("  → Executed {skill_id}/{action_id} successfully.");
                journal_chat_turn(
                    journal,
                    session_id,
                    model,
                    &format!("/approve {skill_id}/{action_id}"),
                    &format!(
                        "executed: exit=0, stdout_len={}, stderr_len={}",
                        outcome.stdout.len(),
                        outcome.stderr.len()
                    ),
                );
                let report = format_intervention_result(&outcome, &skill_id, &action_id);
                Some(report)
            } else if outcome.rollback_applied() {
                let rb = outcome.rollback.as_ref().unwrap();
                println!(
                    "  → {skill_id}/{action_id} failed (exit {:?}). Rollback applied: exit {:?}.",
                    outcome.exit_code, rb.exit_code
                );
                if !outcome.stderr.is_empty() {
                    println!("  stderr: {}", outcome.stderr.trim());
                }
                journal_chat_turn(
                    journal,
                    session_id,
                    model,
                    &format!("/approve {skill_id}/{action_id}"),
                    &format!(
                        "failed: forward_exit={:?}, rollback_exit={:?}",
                        outcome.exit_code, rb.exit_code
                    ),
                );
                Some(format!(
                    "[intervention result: {skill_id}/{action_id}, failed, rolled back]\n\
                     forward exit={:?}\nrollback exit={:?}",
                    outcome.exit_code, rb.exit_code
                ))
            } else {
                println!(
                    "  → {skill_id}/{action_id} failed (exit {:?}). No rollback available.",
                    outcome.exit_code
                );
                if !outcome.stderr.is_empty() {
                    println!("  stderr: {}", outcome.stderr.trim());
                }
                journal_chat_turn(
                    journal,
                    session_id,
                    model,
                    &format!("/approve {skill_id}/{action_id}"),
                    &format!("failed: exit={:?}, no_rollback", outcome.exit_code),
                );
                Some(format!(
                    "[intervention result: {skill_id}/{action_id}, failed, exit={:?}, no rollback]",
                    outcome.exit_code
                ))
            }
        }
        Err(e) => {
            println!("  → Error running {skill_id}/{action_id}: {e}");
            Some(format!(
                "[intervention error: {skill_id}/{action_id}] {e}\n"
            ))
        }
    }
}

/// Format a probe result for LLM context injection.
pub fn format_probe_result(
    result: std::result::Result<russell_skills::dispatch::RunOutcome, russell_core::CoreError>,
    skill_id: &str,
    action_id: &str,
) -> Option<String> {
    match result {
        Ok(outcome) => {
            if outcome.exit_code == Some(0) {
                if !outcome.stdout.is_empty() {
                    println!("  {}", outcome.stdout.trim());
                }
                println!("  → Probe {skill_id}/{action_id} complete.");
            } else {
                println!(
                    "  → {skill_id}/{action_id} exited with code {:?}.",
                    outcome.exit_code
                );
                if !outcome.stderr.is_empty() {
                    println!("  stderr: {}", outcome.stderr.trim());
                }
            }
            let exit_str = match outcome.exit_code {
                Some(code) => format!("exit={code}"),
                None => "killed/timeout".to_string(),
            };
            let mut report = format!("[probe result: {skill_id}/{action_id}, {exit_str}]\n");
            if !outcome.stdout.is_empty() {
                let stdout = outcome.stdout.trim();
                if stdout.len() > 3000 {
                    report.push_str(&stdout[..3000]);
                    report.push_str("\n… (output truncated)\n");
                } else {
                    report.push_str(stdout);
                    report.push('\n');
                }
            }
            if !outcome.stderr.is_empty() {
                let stderr = outcome.stderr.trim();
                let stderr_truncated = if stderr.len() > 1000 {
                    format!("{}… (truncated)", &stderr[..1000])
                } else {
                    stderr.to_string()
                };
                report.push_str(&format!("stderr: {stderr_truncated}\n"));
            }
            Some(report)
        }
        Err(e) => {
            println!("  → Error running probe {skill_id}/{action_id}: {e}");
            Some(format!("[probe error: {skill_id}/{action_id}] {e}\n"))
        }
    }
}

/// Format an intervention result for LLM context injection.
pub fn format_intervention_result(
    outcome: &russell_skills::dispatch::RunOutcome,
    skill_id: &str,
    action_id: &str,
) -> String {
    let exit_str = match outcome.exit_code {
        Some(code) => format!("exit={code}"),
        None => "killed/timeout".to_string(),
    };
    let mut report = format!("[intervention result: {skill_id}/{action_id}, {exit_str}]\n");
    if !outcome.stdout.is_empty() {
        let stdout = outcome.stdout.trim();
        if stdout.len() > 3000 {
            report.push_str(&stdout[..3000]);
            report.push_str("\n… (output truncated)\n");
        } else {
            report.push_str(stdout);
            report.push('\n');
        }
    }
    if !outcome.stderr.is_empty() {
        let stderr = outcome.stderr.trim();
        let stderr_truncated = if stderr.len() > 1000 {
            format!("{}… (truncated)", &stderr[..1000])
        } else {
            stderr.to_string()
        };
        report.push_str(&format!("stderr: {stderr_truncated}\n"));
    }
    report
}

/// Journal a chat turn as a help-session event for audit.
pub fn journal_chat_turn(
    journal: &JournalWriter,
    session_id: &str,
    model: &str,
    user_msg: &str,
    assistant_msg: &str,
) {
    let ts_unix = russell_core::time::now_unix();
    let ts = russell_core::time::now_rfc3339();
    let evidence_ref = format!("memory/chats/{session_id}.json");
    let input = HelpSessionInput {
        id: session_id,
        ts_unix,
        ts: &ts,
        backend: "okapi",
        model: Some(model),
        note: Some(user_msg),
        prompt_chars: user_msg.len() as i64,
        response_chars: assistant_msg.len() as i64,
        latency_ms: None,
        status: HelpSessionStatus::Ok,
        error_kind: None,
        evidence_ref: &evidence_ref,
    };
    let _ = journal.append_help_session(&input);
}

/// Execute a raw shell command (ADR-0050).
/// Runs `bash -c <command>` directly, journals the result,
/// and returns the output for LLM context injection.
pub async fn execute_shell_command(
    journal: &JournalWriter,
    action: &ResolvedAction,
    session_id: &str,
    model: &str,
    timeout: std::time::Duration,
) -> Option<String> {
    let (command, risk, needs_sudo) = match action {
        ResolvedAction::ShellCommand {
            command,
            risk,
            needs_sudo,
            ..
        } => (command.clone(), *risk, *needs_sudo),
        _ => return None,
    };

    // Build the subprocess command.
    // If sudo is needed, we wrap with sudo -S and read password.
    let mut cmd = if needs_sudo {
        let mut c = tokio::process::Command::new("sudo");
        c.arg("-S").arg("--").arg("bash").arg("-c").arg(&command);
        c
    } else {
        let mut c = tokio::process::Command::new("bash");
        c.arg("-c").arg(&command);
        c
    };
    cmd.stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .stdin(std::process::Stdio::piped());

    // If sudo, prompt for password — but only if the credential cache has expired.
    if needs_sudo {
        // Check if sudo timestamp is still valid (set by execute_pending_action
        // via sudo -v, or from a prior shell command).
        let cached = tokio::process::Command::new("sudo")
            .args(["-n", "--", "true"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .await
            .is_ok_and(|s| s.success());

        let password = if cached {
            // Timestamp is valid — sudo -S will accept an empty/newline stdin.
            String::new()
        } else {
            eprint!("  \u{2192} Sudo password for shell command: ");
            let _ = std::io::stderr().flush();
            let pw = rpassword::read_password().unwrap_or_default();
            if pw.is_empty() {
                println!("  \u{2192} Empty password. Aborting.");
                return None;
            }
            pw
        };
        // Spawn and pipe password.
        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                println!("  \u{2192} Failed to spawn: {e}");
                return Some(format!("[shell error: failed to spawn] {e}\n"));
            }
        };
        if let Some(ref mut stdin) = child.stdin {
            use tokio::io::AsyncWriteExt;
            let _ = stdin.write_all(format!("{password}\n").as_bytes()).await;
            let _ = stdin; // Drop stdin reference so process proceeds
        }
        // Wait with timeout.
        let outcome = match tokio::time::timeout(timeout, child.wait_with_output()).await {
            Ok(Ok(output)) => output,
            Ok(Err(e)) => {
                println!("  \u{2192} Execution failed: {e}");
                return Some(format!("[shell error: execution failed] {e}\n"));
            }
            Err(_) => {
                println!("  \u{2192} Timed out after {:?}.", timeout);
                return Some(format!("[shell error: timed out after {:?}]\n", timeout));
            }
        };
        let exit_code = outcome.status.code();
        let stdout = String::from_utf8_lossy(&outcome.stdout);
        let stderr = String::from_utf8_lossy(&outcome.stderr);
        return format_shell_result(
            &command, exit_code, &stdout, &stderr, risk, journal, session_id, model,
        );
    }

    // Non-sudo path.
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            println!("  \u{2192} Failed to spawn: {e}");
            return Some(format!("[shell error: failed to spawn] {e}\n"));
        }
    };
    // Close stdin.
    if let Some(ref mut stdin) = child.stdin {
        use tokio::io::AsyncWriteExt;
        let _ = stdin.shutdown().await;
    }
    let outcome = match tokio::time::timeout(timeout, child.wait_with_output()).await {
        Ok(Ok(output)) => output,
        Ok(Err(e)) => {
            println!("  \u{2192} Execution failed: {e}");
            return Some(format!("[shell error: execution failed] {e}\n"));
        }
        Err(_) => {
            println!("  \u{2192} Timed out after {:?}.", timeout);
            return Some(format!("[shell error: timed out after {:?}]\n", timeout));
        }
    };
    let exit_code = outcome.status.code();
    let stdout = String::from_utf8_lossy(&outcome.stdout);
    let stderr = String::from_utf8_lossy(&outcome.stderr);
    format_shell_result(
        &command, exit_code, &stdout, &stderr, risk, journal, session_id, model,
    )
}

/// Format a shell command result for LLM context injection and journal it.
#[allow(clippy::too_many_arguments)]
fn format_shell_result(
    command: &str,
    exit_code: Option<i32>,
    stdout: &str,
    stderr: &str,
    risk: russell_skills::RiskBand,
    journal: &JournalWriter,
    session_id: &str,
    model: &str,
) -> Option<String> {
    let exit_str = match exit_code {
        Some(code) => format!("exit={code}"),
        None => "killed/timeout".to_string(),
    };

    if exit_code == Some(0) {
        if !stdout.is_empty() {
            println!("  {}", stdout.trim());
        }
        println!("  \u{2192} Shell command complete.");
    } else {
        println!("  \u{2192} Shell command exited with code {:?}.", exit_code);
        if !stderr.is_empty() {
            println!("  stderr: {}", stderr.trim());
        }
    }

    // Journal the shell execution.
    journal_chat_turn(
        journal,
        session_id,
        model,
        &format!("shell: {command}"),
        &format!("{exit_str}, risk={:?}", risk),
    );

    // Build result for LLM context.
    let mut report = format!("[shell result: {command}, {exit_str}, risk={:?}]\n", risk);
    if !stdout.is_empty() {
        let stdout_trimmed = stdout.trim();
        if stdout_trimmed.len() > 3000 {
            report.push_str(&stdout_trimmed[..3000]);
            report.push_str("\n\u{2026} (output truncated)\n");
        } else {
            report.push_str(stdout_trimmed);
            report.push('\n');
        }
    }
    if !stderr.is_empty() {
        let stderr_trimmed = stderr.trim();
        let stderr_truncated = if stderr_trimmed.len() > 1000 {
            format!("{}\u{2026} (truncated)", &stderr_trimmed[..1000])
        } else {
            stderr_trimmed.to_string()
        };
        report.push_str(&format!("stderr: {stderr_truncated}\n"));
    }
    Some(report)
}

/// Format a remote MCP tool result for LLM context injection.
/// This produces the `[remote tool result: ...]` prefix that
/// `is_action_result_in_history` detects.
pub fn format_remote_tool_result(tool_name: &str, output: &str) -> String {
    let mut report = format!("[remote tool result: {tool_name}, status=ok]\n");
    if !output.is_empty() {
        let trimmed = output.trim();
        if trimmed.len() > 3000 {
            report.push_str(&trimmed[..3000]);
            report.push_str("\n… (output truncated)\n");
        } else {
            report.push_str(trimmed);
            report.push('\n');
        }
    }
    report
}

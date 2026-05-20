// SPDX-License-Identifier: MIT OR Apache-2.0
//! Action execution for the chat REPL — local skill dispatch.
//!
//! Handles probes and interventions via the skill dispatcher with
//! IDRS compliance (journaling, rollback, evidence bundles).

use russell_core::journal::{HelpSessionInput, HelpSessionStatus, JournalWriter};
use russell_core::paths::Paths;
use russell_meta::action::ResolvedAction;
use russell_skills::RiskBand;

use super::consent::PendingAction;

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
            max_auto_risk,
            requires_human,
            rollback_id,
            rollback_cmd,
            rollback_is_reboot,
            ..
        } => (
            *risk,
            *needs_sudo,
            *max_auto_risk,
            *requires_human,
            rollback_id.clone(),
            rollback_cmd.clone(),
            *rollback_is_reboot,
        ),
        ResolvedAction::KaskTool { .. } => {
            println!("  → Internal error: KaskTool routed to local dispatcher.");
            return None;
        }
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
    if needs_sudo {
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
                dispatcher.sudo_password = Some(russell_skills::dispatch::SudoCredential::new(password));
            }
            _ => {
                println!("  → Wrong sudo password. Aborting action.");
                return None;
            }
        }
    }

    let step_type = if is_probe {
        StepType::Probe
    } else {
        StepType::Intervention
    };

    if is_probe {
        // Probes: use run_and_journal (read-only, no rollback needed).
        let result = dispatcher
            .run_and_journal(
                journal,
                &evidence_base,
                action.cmd(),
                &skill_id,
                &action_id,
                step_type,
                risk.as_str(),
                Some(timeout),
            )
            .await;

        // Update registry telemetry.
        let probe_success = result.as_ref().is_ok_and(|o| o.exit_code == Some(0) && !o.timed_out);
        let probe_duration_ms = result.as_ref().map(|o| o.duration.as_millis() as u64).unwrap_or(0);
        let probe_error = result.as_ref().err().map(|e| e.to_string());
        let registry_path = paths.state.join("registry").join("local-cache.yaml");
        let _ = RegistryCache::with_update(&registry_path, |cache| {
            cache.record_execution(&skill_id, probe_success, probe_duration_ms, probe_error.as_deref());
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
            action.cmd(),
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
                Some(format!("exit {:?}, stderr: {}", outcome.exit_code, outcome.stderr.trim()))
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

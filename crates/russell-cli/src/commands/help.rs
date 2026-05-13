// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell jack` — Jack's cry-for-help channel.

use anyhow::{Context, Result};
use russell_core::journal::JournalWriter;
use russell_core::paths::Paths;

pub async fn run(paths: &Paths, note: Option<&str>) -> Result<()> {
    let writer = JournalWriter::open(&paths.journal())
        .with_context(|| format!("opening journal {}", paths.journal().display()))?;

    let outcome = russell_doctor::run_help(paths, &writer, note)
        .await
        .context("running Doctor help flow")?;

    // Print the response without the ACTION line (we handle it separately).
    let response = outcome.response.trim_end();
    let (body, action_spec) = split_action_line(response);
    println!("{body}");
    println!();

    // If Jack proposed an action, resolve it.
    if let Some(spec) = action_spec {
        let skills = russell_skills::load_all(&paths.skills()).unwrap_or_default();
        match resolve_action_type(spec, &skills) {
            Some(ResolvedAction::Probe(skill, probe)) => {
                // Probes are read-only — execute immediately.
                println!("  → Running probe: {}/{}…", skill.id, probe.id);
                execute_probe(paths, &writer, skill, probe).await;
            }
            Some(ResolvedAction::Intervention(skill, iv)) => {
                let sudo_tag = if iv.needs_sudo { " [needs sudo]" } else { "" };
                println!(
                    "  → Jack proposes: {}/{} (risk: {:?}{})",
                    skill.id, iv.id, iv.risk, sudo_tag,
                );
                println!("  → Switch to `russell chat` and I'll run it — just say 'ok' when I ask.");
                println!();
            }
            None => {}
        }
    }

    println!(
        "  [jack via {} · session {} · bundle {}]",
        outcome.backend,
        outcome.session_id,
        outcome.evidence_dir.display()
    );

    // ADR-0020: skip_reason replaces fell_back
    if let Some(sr) = outcome.skip_reason {
        let msg = match sr {
            russell_doctor::help::SkipReason::OfflineFallback => {
                "  [offline fallback engaged — Ollama unreachable or LLM call failed]"
            }
            russell_doctor::help::SkipReason::ThresholdSkip => {
                "  [below escalation threshold — rule-based summary returned]"
            }
        };
        println!("{msg}");
    }

    Ok(())
}

/// A resolved ACTION — either a probe or an intervention.
enum ResolvedAction<'a> {
    Probe(&'a russell_skills::Skill, &'a russell_skills::Probe),
    Intervention(&'a russell_skills::Skill, &'a russell_skills::Intervention),
}

/// Resolve an ACTION spec, checking probes first (read-only), then interventions.
fn resolve_action_type<'a>(
    spec: &str,
    skills: &'a [russell_skills::Skill],
) -> Option<ResolvedAction<'a>> {
    let (skill_id, action_id) = spec.split_once('/')?;
    let skill_id = skill_id.trim();
    let action_id = action_id.trim();
    let skill = skills.iter().find(|s| s.id == skill_id)?;

    // Check probes first.
    if let Some(probe) = skill.probes.iter().find(|p| p.id == action_id) {
        return Some(ResolvedAction::Probe(skill, probe));
    }
    // Then interventions.
    let iv = skill.interventions.iter().find(|i| i.id == action_id)?;
    Some(ResolvedAction::Intervention(skill, iv))
}

/// Execute a probe immediately (read-only, risk: none).
async fn execute_probe(
    paths: &Paths,
    journal: &JournalWriter,
    skill: &russell_skills::Skill,
    probe: &russell_skills::Probe,
) {
    use russell_skills::dispatch::{Dispatcher, DryRun, StepType};
    use std::time::Duration;

    let skill_dir = paths.skills().join(&skill.id);
    let evidence_base = paths.evidence();
    let timeout = Duration::from_secs(30);

    let mut dispatcher = Dispatcher::new(&skill_dir);
    dispatcher.probe_timeout = timeout;
    dispatcher.dry_run = DryRun::Disabled;
    dispatcher.max_auto_risk = skill.safety.max_auto_risk;

    let result = dispatcher
        .run_and_journal(
            journal,
            &evidence_base,
            &probe.cmd,
            &skill.id,
            &probe.id,
            StepType::Probe,
            "none",
            Some(timeout),
        )
        .await;

    match result {
        Ok(outcome) => {
            if outcome.exit_code == Some(0) {
                if !outcome.stdout.is_empty() {
                    println!("  {}", outcome.stdout.trim());
                }
                println!("  → Probe {}/{} complete.", skill.id, probe.id);
            } else {
                println!(
                    "  → Probe {}/{} exited with code {:?}.",
                    skill.id, probe.id, outcome.exit_code
                );
                if !outcome.stderr.is_empty() {
                    println!("  stderr: {}", outcome.stderr.trim());
                }
            }
        }
        Err(e) => {
            println!("  → Failed to run probe: {e}");
        }
    }
    println!();
}

/// Split a response into body text and an optional ACTION: spec.
/// The ACTION line must be the last line of the response.
fn split_action_line(response: &str) -> (&str, Option<&str>) {
    let trimmed = response.trim_end();
    if let Some(pos) = trimmed.rfind("\nACTION:") {
        let (body, action) = trimmed.split_at(pos);
        let spec = action.trim().strip_prefix("ACTION:").map(|s| s.trim());
        (body.trim_end(), spec)
    } else if trimmed.starts_with("ACTION:") {
        (
            "(action proposed without explanation)",
            Some(trimmed.strip_prefix("ACTION:").unwrap().trim()),
        )
    } else {
        (trimmed, None)
    }
}

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

    // If Jack proposed an action, resolve it and display guidance.
    if let Some(spec) = action_spec {
        let skills = russell_skills::load_all(&paths.skills()).unwrap_or_default();
        if let Some((skill, iv)) = resolve_action(spec, &skills) {
            let sudo_tag = if iv.needs_sudo { " [needs sudo]" } else { "" };
            println!(
                "  → Jack proposes: {}/{} (risk: {:?}{})",
                skill.id, iv.id, iv.risk, sudo_tag,
            );
            if iv.needs_sudo {
                println!("  → To execute: russell chat     (then /approve with sudo password)");
            } else {
                println!("  → To execute: russell skill run {}/{}", skill.id, iv.id);
            }
            println!();
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

/// Resolve an ACTION spec into a skill and intervention.
fn resolve_action<'a>(
    spec: &str,
    skills: &'a [russell_skills::Skill],
) -> Option<(&'a russell_skills::Skill, &'a russell_skills::Intervention)> {
    let (skill_id, iv_id) = spec.split_once('/')?;
    let skill_id = skill_id.trim();
    let iv_id = iv_id.trim();
    let skill = skills.iter().find(|s| s.id == skill_id)?;
    let iv = skill.interventions.iter().find(|i| i.id == iv_id)?;
    Some((skill, iv))
}

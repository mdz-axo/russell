// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell skill` — manage installed skills: list, run, stats, check,
//! install, prune, restore, and retire.
//!
//! Phase 3/4 per ADR-0023 and skill self-management strategy.

use anyhow::{Context, Result};
use russell_core::paths::Paths;
use russell_skills::{Skill, load_all};
use std::time::Duration;

/// Parse a duration string like "180s", "5m", "1h" to a Duration.
fn parse_duration(s: &str) -> Option<Duration> {
    let s = s.trim();
    if let Some(num) = s.strip_suffix('s') {
        return Some(Duration::from_secs(num.parse().ok()?));
    }
    if let Some(num) = s.strip_suffix('m') {
        return Some(Duration::from_secs(
            num.parse::<u64>().ok()?.saturating_mul(60),
        ));
    }
    if let Some(num) = s.strip_suffix('h') {
        return Some(Duration::from_secs(
            num.parse::<u64>().ok()?.saturating_mul(3600),
        ));
    }
    None
}

/// List loaded skills.
pub fn list(paths: &Paths) -> Result<()> {
    super::skill_lifecycle::list_skills(paths)
}

/// Run a probe from a loaded skill by ID.
pub async fn run(paths: &Paths, id: &str, dry_run: bool) -> Result<()> {
    let (skill_id, step_id) = parse_skill_ref(id)?;
    let skills_dir = paths.skills();
    let skills = load_all(&skills_dir).context("loading skills")?;

    let skill = skills
        .iter()
        .find(|s| s.id == skill_id)
        .with_context(|| format!("skill '{}' not found", skill_id))?;

    let cmd: &[String] = if let Some(p) = skill.probes.iter().find(|p| p.id == step_id) {
        &p.cmd
    } else if let Some(i) = skill.interventions.iter().find(|i| i.id == step_id) {
        &i.cmd
    } else {
        anyhow::bail!("step '{}' not found in skill '{}'", step_id, skill_id);
    };

    let skill_dir = skills_dir.join(skill_id);
    let mut dispatcher = russell_skills::dispatch::Dispatcher::new(&skill_dir);
    if dry_run {
        dispatcher.dry_run = russell_skills::dispatch::DryRun::Enabled;
        println!("[DRY RUN] would run:");
        for arg in cmd {
            print!("{arg} ");
        }
        println!("\nin: {}", skill_dir.display());
        return Ok(());
    }

    let timeout_override = step_timeout(skill, step_id);
    let outcome = dispatcher
        .run(cmd, timeout_override)
        .await
        .with_context(|| format!("running {}/{}", skill_id, step_id))?;

    let probe_success = outcome.exit_code == Some(0) && !outcome.timed_out;
    let probe_duration_ms = outcome.duration.as_millis() as u64;
    let probe_error = if !probe_success {
        Some(outcome.stderr.clone())
    } else {
        None
    };
    let registry_path = paths.state.join("registry").join("local-cache.yaml");
    let _ = russell_skills::registry::RegistryCache::with_update(&registry_path, |cache| {
        cache.record_execution(
            skill_id,
            probe_success,
            probe_duration_ms,
            probe_error.as_deref(),
        );
    });

    println!("--- stdout ---");
    print!("{}", outcome.stdout);
    if !outcome.stderr.is_empty() {
        println!("--- stderr ---");
        print!("{}", outcome.stderr);
    }
    if outcome.timed_out {
        println!("[TIMED OUT after {:?}]", outcome.duration);
    }
    println!(
        "[exit: {:?}, elapsed: {:?}]",
        outcome.exit_code, outcome.duration
    );

    Ok(())
}

fn parse_skill_ref(id: &str) -> Result<(&str, &str)> {
    let mut parts = id.splitn(2, '/');
    let skill = parts.next().context("missing skill id in reference")?;
    let step = parts.next().context("missing step id (use <skill>/<id>)")?;
    if skill.is_empty() || step.is_empty() {
        anyhow::bail!("invalid skill reference: '{id}' (use <skill>/<id>)");
    }
    Ok((skill, step))
}

fn step_timeout(skill: &Skill, step_id: &str) -> Option<Duration> {
    if let Some(p) = skill.probes.iter().find(|p| p.id == step_id) {
        parse_duration(&p.timeout)
    } else if let Some(i) = skill.interventions.iter().find(|i| i.id == step_id) {
        parse_duration(&i.timeout)
    } else {
        None
    }
}

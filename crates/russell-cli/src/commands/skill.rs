// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell skill` — list and run skill probes.
//!
//! Phase 3A per ADR-0023.

use anyhow::{Context, Result};
use russell_core::paths::Paths;
use russell_skills::{Skill, load_all};

/// Enumeration of the loaded skills and their probes/interventions.
pub fn list(paths: &Paths) -> Result<()> {
    let skills_dir = paths.skills();
    let skills = load_all(&skills_dir).context("loading skills")?;

    if skills.is_empty() {
        println!(
            "No skills loaded. Place skill directories under {}",
            skills_dir.display()
        );
        return Ok(());
    }

    for s in &skills {
        print_skill(s);
    }

    Ok(())
}

/// Run a probe from a loaded skill by ID (format: `<skill-id>/<probe-id>`).
pub async fn run(paths: &Paths, id: &str, dry_run: bool) -> Result<()> {
    let (skill_id, step_id) = parse_skill_ref(id)?;
    let skills_dir = paths.skills();
    let skills = load_all(&skills_dir).context("loading skills")?;

    let skill = skills
        .iter()
        .find(|s| s.id == skill_id)
        .with_context(|| format!("skill '{}' not found", skill_id))?;

    // Look up in probes first.
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

    let outcome = dispatcher
        .run(cmd, None)
        .await
        .with_context(|| format!("running {}/{}", skill_id, step_id))?;

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

fn print_skill(s: &Skill) {
    println!("{}", s.id);
    if !s.symptoms.is_empty() {
        println!("  symptoms: {}", s.symptoms.join(", "));
    }
    for p in &s.probes {
        println!("  probe: {} ({})", p.id, p.cmd.join(" "));
    }
    for i in &s.interventions {
        println!(
            "  intervention: {} ({}) [risk: {:?}]",
            i.id,
            i.cmd.join(" "),
            i.risk
        );
    }
    println!();
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

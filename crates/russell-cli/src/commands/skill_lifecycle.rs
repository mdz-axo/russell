// SPDX-License-Identifier: MIT OR Apache-2.0
//! Skill lifecycle management — shared operations for install, prune, restore, retire.
//!
//! This module consolidates skill lifecycle operations to avoid C7 violations
//! (divergent implementations). Both `russell skill` CLI commands and the
//! workshop REPL use these shared functions.

use anyhow::{Context, Result};
use russell_core::journal::JournalWriter;
use russell_core::paths::Paths;
use russell_core::time::now_date_iso8601;
use russell_skills::registry::{LifecycleStatus, RegistryCache, RegistryEntry, SkillSource};

/// Install or activate a skill by name.
pub fn install_skill(paths: &Paths, name: &str, verbose: bool) -> Result<()> {
    let skills_dir = paths.skills();
    let registry_path = paths.state.join("registry").join("local-cache.yaml");

    let skill_dir = skills_dir.join(name);
    if !skill_dir.exists() || !skill_dir.join("manifest.yaml").exists() {
        anyhow::bail!(
            "Skill '{name}' not found on disk. Use 'russell workshop build {name}' to create it."
        );
    }

    let mut from_status: Option<LifecycleStatus> = None;
    RegistryCache::with_update(&registry_path, |registry| {
        if let Some(entry) = registry.skills.get_mut(name) {
            if entry.status.is_loadable() {
                if verbose {
                    println!(
                        "{name} is already {} (v{}).",
                        entry.status.as_str(),
                        entry.version
                    );
                }
                return;
            }
            from_status = Some(entry.status);
            if verbose {
                println!("Installing {name} (v{})...", entry.version);
                println!("  Status: {} → installed", entry.status.as_str());
            }
            entry.status = LifecycleStatus::Installed;
            entry.installed = now_date_iso8601();
            if verbose {
                println!("  Skill will be available on next load.");
            }
        } else {
            from_status = None;
            let version = "0.1.0".to_string();
            let today = now_date_iso8601();
            registry.upsert(
                name,
                RegistryEntry::new_default(
                    LifecycleStatus::Installed,
                    version.clone(),
                    &today,
                    vec![],
                    SkillSource::Manual,
                    &today,
                    false,
                ),
            );
            if verbose {
                println!("{name} registered (v{version}).");
            }
        }
    })?;

    let journal = JournalWriter::open(&paths.journal()).context("opening journal for audit")?;
    RegistryCache::journal_transition(
        &journal,
        name,
        from_status,
        LifecycleStatus::Installed,
        Some("install via CLI"),
    );
    Ok(())
}

/// Deprecate a skill (mark as deprecated but keep files).
pub fn prune_skill(paths: &Paths, name: &str, verbose: bool) -> Result<()> {
    let registry_path = paths.state.join("registry").join("local-cache.yaml");

    let mut from_status: Option<LifecycleStatus> = None;
    RegistryCache::with_update(&registry_path, |registry| {
        if let Some(entry) = registry.skills.get_mut(name) {
            if entry.status == LifecycleStatus::Deprecated
                || entry.status == LifecycleStatus::Retired
            {
                if verbose {
                    println!("{name} is already {}.", entry.status.as_str());
                }
                return;
            }
            from_status = Some(entry.status);
            let reason = entry
                .deprecation_reason
                .clone()
                .unwrap_or_else(|| "operator requested".into());
            if verbose {
                println!("Deprecating {name} (v{}): {}", entry.version, reason);
                println!("  Status: {} → deprecated", entry.status.as_str());
                println!("  Files remain on disk. Use 'restore' to undo.");
            }
            entry.status = LifecycleStatus::Deprecated;
            entry.deprecation_reason = Some(reason);
            if entry.superseded_by.is_none() {
                entry.superseded_by = Some("operator".into());
            }
        } else {
            if verbose {
                println!("Skill '{name}' not found in registry.");
            }
        }
    })?;

    let journal = JournalWriter::open(&paths.journal()).context("opening journal for audit")?;
    RegistryCache::journal_transition(
        &journal,
        name,
        from_status,
        LifecycleStatus::Deprecated,
        Some("pruned via CLI"),
    );
    Ok(())
}

/// Restore a deprecated skill to active status.
pub fn restore_skill(paths: &Paths, name: &str, verbose: bool) -> Result<()> {
    let registry_path = paths.state.join("registry").join("local-cache.yaml");

    let mut from_status: Option<LifecycleStatus> = None;
    RegistryCache::with_update(&registry_path, |registry| {
        if let Some(entry) = registry.skills.get_mut(name) {
            if entry.status.is_loadable() {
                if verbose {
                    println!("{name} is already {}.", entry.status.as_str());
                }
                return;
            }
            from_status = Some(entry.status);
            if verbose {
                println!("Restoring {name} (v{})...", entry.version);
                println!("  Status: {} → installed", entry.status.as_str());
            }
            entry.status = LifecycleStatus::Installed;
            entry.deprecation_reason = None;
            entry.superseded_by = None;
        } else {
            if verbose {
                println!("Skill '{name}' not found in registry.");
            }
        }
    })?;

    let journal = JournalWriter::open(&paths.journal()).context("opening journal for audit")?;
    RegistryCache::journal_transition(
        &journal,
        name,
        from_status,
        LifecycleStatus::Installed,
        Some("restored via CLI"),
    );
    Ok(())
}

/// Retire/delete a skill: remove from disk and registry.
pub fn retire_skill(paths: &Paths, name: &str, verbose: bool) -> Result<()> {
    let skills_dir = paths.skills();
    let registry_path = paths.state.join("registry").join("local-cache.yaml");

    let skill_dir = skills_dir.join(name);
    if !skill_dir.exists() {
        if verbose {
            println!("Skill '{name}' not found on disk.");
        }
        return Ok(());
    }

    let mut from_status: Option<LifecycleStatus> = None;
    RegistryCache::with_update(&registry_path, |registry| {
        if let Some(entry) = registry.skills.get(name) {
            from_status = Some(entry.status);
        }
        registry.skills.remove(name);
    })?;

    std::fs::remove_dir_all(&skill_dir)
        .with_context(|| format!("removing skill directory {}", skill_dir.display()))?;

    if verbose {
        println!("Retired {name}. Files removed from disk.");
    }

    let journal = JournalWriter::open(&paths.journal()).context("opening journal for audit")?;
    RegistryCache::journal_transition(
        &journal,
        name,
        from_status,
        LifecycleStatus::Retired,
        Some("retired via CLI"),
    );
    Ok(())
}

/// Check all skills in the registry for issues.
pub fn check_skills(paths: &Paths) -> Result<()> {
    let registry_path = paths.state.join("registry").join("local-cache.yaml");
    let registry = RegistryCache::load(&registry_path).unwrap_or_default();

    if registry.skills.is_empty() {
        println!("No skills in registry.");
        return Ok(());
    }

    let skills_dir = paths.skills();
    let mut issues = Vec::new();

    for (id, entry) in &registry.skills {
        let skill_dir = skills_dir.join(id);
        let has_manifest = skill_dir.join("manifest.yaml").exists();

        if !has_manifest && entry.status.is_loadable() {
            issues.push(format!(
                "{id} (v{}): status={} but manifest missing",
                entry.version,
                entry.status.as_str()
            ));
        }

        if entry.recent_probe_failures > 0 || entry.recent_intervention_failures > 0 {
            issues.push(format!(
                "{id} (v{}): {} probe failures, {} intervention failures",
                entry.version, entry.recent_probe_failures, entry.recent_intervention_failures
            ));
        }

        if let Some(ref reason) = entry.deprecation_reason
            && entry.status == LifecycleStatus::Deprecated
        {
            issues.push(format!("{id} (v{}): deprecated — {reason}", entry.version));
        }
    }

    if issues.is_empty() {
        println!("All {} skills healthy.", registry.skills.len());
    } else {
        println!("Found {} issue(s):", issues.len());
        for issue in &issues {
            println!("  - {issue}");
        }
    }

    Ok(())
}

/// Print skill statistics/telemetry.
pub fn print_skill_stats(paths: &Paths, json: bool) -> Result<()> {
    let registry_path = paths.state.join("registry").join("local-cache.yaml");
    let registry = RegistryCache::load(&registry_path).unwrap_or_default();

    if registry.skills.is_empty() {
        if json {
            println!("[]");
        } else {
            println!("No skills in registry.");
        }
        return Ok(());
    }

    if json {
        #[derive(serde::Serialize)]
        struct SkillStat {
            skill: String,
            status: String,
            version: String,
            probe_runs: u64,
            recent_probe_failures: u64,
            intervention_runs: u64,
            recent_intervention_failures: u64,
            avg_duration_ms: Option<f64>,
            last_probe_run_at: Option<String>,
            last_error: Option<String>,
            coverage_score: Option<f64>,
        }
        let stats: Vec<SkillStat> = registry
            .skills
            .iter()
            .map(|(id, entry)| SkillStat {
                skill: id.clone(),
                status: entry.status.as_str().to_string(),
                version: entry.version.clone(),
                probe_runs: entry.probe_runs,
                recent_probe_failures: entry.recent_probe_failures,
                intervention_runs: entry.intervention_runs,
                recent_intervention_failures: entry.recent_intervention_failures,
                avg_duration_ms: entry.avg_probe_duration_ms,
                last_probe_run_at: entry.last_probe_run_at.clone(),
                last_error: entry.last_error.clone(),
                coverage_score: entry.coverage_score,
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&stats)?);
    } else {
        println!(
            "{:<30} {:<12} {:<8} {:<8} {:<10} {:<10}",
            "SKILL", "STATUS", "PROBES", "FAILS", "INTERVS", "I-FAILS"
        );
        println!(
            "{:-<30} {:-<12} {:-<8} {:-<8} {:-<10} {:-<10}",
            "", "", "", "", "", ""
        );
        for (id, entry) in &registry.skills {
            println!(
                "{:<30} {:<12} {:<8} {:<8} {:<10} {:<10}",
                id,
                entry.status.as_str(),
                entry.probe_runs,
                entry.recent_probe_failures,
                entry.intervention_runs,
                entry.recent_intervention_failures
            );
        }
    }

    Ok(())
}

/// List skills with their lifecycle status.
pub fn list_skills(paths: &Paths) -> Result<()> {
    let skills_dir = paths.skills();
    let skills = russell_skills::load_all(&skills_dir).context("loading skills")?;

    if skills.is_empty() {
        println!(
            "No skills loaded. Place skill directories under {}",
            skills_dir.display()
        );
        return Ok(());
    }

    for s in &skills {
        println!("{} v{} — {:?}", s.id, s.version, s.kind);
    }

    Ok(())
}

/// Build a new skill skeleton on disk.
pub fn build_skill(paths: &Paths, name: &str) -> Result<()> {
    let skills_dir = paths.skills();
    let skill_dir = skills_dir.join(name);

    if skill_dir.exists() {
        anyhow::bail!("Skill '{name}' already exists at {}", skill_dir.display());
    }

    std::fs::create_dir_all(&skill_dir)
        .with_context(|| format!("creating skill directory {}", skill_dir.display()))?;

    let today = now_date_iso8601();
    let manifest = format!(
        r#"# Skill manifest for {name}
id: {name}
version: 0.1.0
authored: {today}
min_harness_version: 0.20.0

kind: actionable

symptoms: []

applies_when:
  - os_family: linux

probes:
  - id: health
    cmd: ["echo", "health check"]
    timeout: 30s

interventions: []

safety:
  risk_band: none
  idempotent: true
  dry_run_support: true
  rollback_support: none_needed
"#
    );

    std::fs::write(skill_dir.join("manifest.yaml"), manifest)
        .with_context(|| format!("writing manifest for {name}"))?;

    std::fs::write(
        skill_dir.join("KNOWLEDGE.md"),
        format!("# {name} Knowledge\n\nAdd skill-specific knowledge here.\n"),
    )
    .ok();

    println!("Created skill skeleton: {}", skill_dir.display());
    println!("  Edit manifest.yaml to define probes and interventions.");
    println!("  Then run: russell skill install {name}");

    Ok(())
}

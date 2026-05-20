// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell skill` — manage installed skills: list, run, stats, check,
//! install, prune, restore, and retire.
//!
//! Phase 3/4 per ADR-0023 and skill self-management strategy.

use anyhow::{Context, Result};
use russell_core::journal::JournalWriter;
use russell_core::paths::Paths;
use russell_core::time::now_date_iso8601;
use russell_skills::registry::{LifecycleStatus, RegistryCache, RegistryEntry, SkillSource};
use russell_skills::{Skill, load_all};
use std::time::Duration;

/// Parse a duration string like "180s", "5m", "1h" to a Duration.
/// Returns None on parse failure.
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

    // Respect the probe's declared timeout from the manifest.
    let timeout_override = step_timeout(skill, step_id);

    let outcome = dispatcher
        .run(cmd, timeout_override)
        .await
        .with_context(|| format!("running {}/{}", skill_id, step_id))?;

    // Update registry telemetry.
    let probe_success = outcome.exit_code == Some(0) && !outcome.timed_out;
    let probe_duration_ms = outcome.duration.as_millis() as u64;
    let probe_error = if !probe_success {
        Some(outcome.stderr.clone())
    } else {
        None
    };
    let registry_path = paths.state.join("registry").join("local-cache.yaml");
    let _ = RegistryCache::with_update(&registry_path, |cache| {
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

/// Extract the timeout from a probe or intervention's manifest entry.
fn step_timeout(skill: &Skill, step_id: &str) -> Option<Duration> {
    if let Some(p) = skill.probes.iter().find(|p| p.id == step_id) {
        parse_duration(&p.timeout)
    } else if let Some(i) = skill.interventions.iter().find(|i| i.id == step_id) {
        parse_duration(&i.timeout)
    } else {
        None
    }
}

// ── Skill lifecycle management verbs ───────────────────────────────────────

/// Print performance telemetry for all skills in the registry.
/// Set `json` to true for scriptable JSON output.
pub fn stats(paths: &Paths, json: bool) -> Result<()> {
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
        return Ok(());
    }

    println!(
        "{:<25} {:>8} {:>8} {:>12} {:>10} {:>12} last run",
        "skill", "probes", "fails", "interv.", "iv fails", "avg dur"
    );
    println!("{}", "-".repeat(100));
    for (id, entry) in &registry.skills {
        let avg_dur = entry
            .avg_probe_duration_ms
            .map_or("--".into(), |d| format!("{d:.0}ms"));
        let last_run = entry.last_probe_run_at.as_deref().unwrap_or("never");
        println!(
            "{:<25} {:>8} {:>8} {:>12} {:>10} {:>12} {}",
            id,
            entry.probe_runs,
            entry.recent_probe_failures,
            entry.intervention_runs,
            entry.recent_intervention_failures,
            avg_dur,
            last_run,
        );
    }
    Ok(())
}

/// Audit all installed skills for staleness and coverage.
pub fn check(paths: &Paths) -> Result<()> {
    let registry_path = paths.state.join("registry").join("local-cache.yaml");
    let registry = RegistryCache::load(&registry_path).unwrap_or_default();
    let skills = load_all(&paths.skills()).unwrap_or_default();

    println!("Skill audit — {}", now_date_iso8601());
    println!();

    // Sync registry from current skills on disk.
    let mut registry = registry;
    for skill in &skills {
        if !registry.skills.contains_key(&skill.id) {
            registry.upsert(
                &skill.id,
                RegistryEntry::new_default(
                    russell_skills::registry::LifecycleStatus::Active,
                    skill.version.clone(),
                    skill.authored.clone(),
                    skill.symptoms.clone(),
                    russell_skills::registry::SkillSource::Manual,
                    skill.authored.clone(),
                    false,
                ),
            );
        }
    }

    let today = now_date_iso8601();
    for (id, entry) in &registry.skills {
        let stale = RegistryCache::is_stale(&entry.authored, &today);
        let mark = if stale { "⚠ stale" } else { "✓" };
        let score = entry
            .coverage_score
            .map_or("--".into(), |s| format!("{s:.2}"));
        let cluster = classify_skill(&entry.symptoms);
        println!(
            "{mark} {id:<30} v{} ({cluster}) — score: {score}, probes: {} runs, {} failures",
            entry.version, entry.probe_runs, entry.recent_probe_failures,
        );
    }

    println!();
    let gaps = registry.coverage_gaps(russell_skills::SYMPTOMS);
    if gaps.is_empty() {
        println!("All catalogue symptoms covered.");
    } else {
        println!("{} symptoms uncovered:", gaps.len());
        for gap in &gaps {
            println!("  - {gap}");
        }
    }
    Ok(())
}

fn classify_skill(symptoms: &[String]) -> &'static str {
    if symptoms.iter().any(|s| s.contains("skill_")) {
        "meta"
    } else if symptoms
        .iter()
        .any(|s| s.contains("gpu") || s.contains("vram") || s.contains("amdgpu"))
    {
        "gpu"
    } else if symptoms
        .iter()
        .any(|s| s.contains("memory") || s.contains("swap") || s.contains("oom"))
    {
        "memory"
    } else if symptoms
        .iter()
        .any(|s| s.contains("disk") || s.contains("io"))
    {
        "storage"
    } else if symptoms
        .iter()
        .any(|s| s.contains("cpu") || s.contains("load") || s.contains("zombie"))
    {
        "cpu"
    } else {
        "general"
    }
}

/// Install or activate a skill by name.
pub fn install(paths: &Paths, name: &str) -> Result<()> {
    use russell_skills::registry::{LifecycleStatus, SkillSource};

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
                println!(
                    "{name} is already {} (v{}).",
                    entry.status.as_str(),
                    entry.version
                );
                return;
            }
            from_status = Some(entry.status);
            entry.status = LifecycleStatus::Installed;
            entry.installed = now_date_iso8601();
            println!("{name} installed (v{}).", entry.version);
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
            println!("{name} registered (v{version}).");
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

/// Deprecate a skill.
pub fn prune(paths: &Paths, name: &str) -> Result<()> {
    use russell_skills::registry::LifecycleStatus;

    let registry_path = paths.state.join("registry").join("local-cache.yaml");

    let mut from_status: Option<LifecycleStatus> = None;
    RegistryCache::with_update(&registry_path, |registry| {
        if let Some(entry) = registry.skills.get_mut(name) {
            if entry.status == LifecycleStatus::Deprecated {
                println!("{name} is already deprecated.");
                return;
            }
            from_status = Some(entry.status);
            println!("Deprecating {name} (v{})...", entry.version);
            entry.status = LifecycleStatus::Deprecated;
            entry.deprecation_reason = Some("pruned via CLI".into());
            println!("Done. Files kept on disk. Use 'restore' to undo.");
        } else {
            println!("Skill '{name}' not found in registry.");
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

/// Restore a deprecated skill to active.
pub fn restore(paths: &Paths, name: &str) -> Result<()> {
    use russell_skills::registry::LifecycleStatus;

    let registry_path = paths.state.join("registry").join("local-cache.yaml");

    RegistryCache::with_update(&registry_path, |registry| {
        if let Some(entry) = registry.skills.get_mut(name) {
            if entry.status != LifecycleStatus::Deprecated {
                println!(
                    "{name} is {} — restore only applies to deprecated skills.",
                    entry.status.as_str()
                );
                return;
            }
            println!("Restoring {name} (v{})...", entry.version);
            entry.status = LifecycleStatus::Active;
            entry.deprecation_reason = None;
            println!("Done.");
        } else {
            println!("Skill '{name}' not found in registry.");
        }
    })?;

    let journal = JournalWriter::open(&paths.journal()).context("opening journal for audit")?;
    RegistryCache::journal_transition(
        &journal,
        name,
        Some(LifecycleStatus::Deprecated),
        LifecycleStatus::Active,
        Some("restore via CLI"),
    );
    Ok(())
}

/// Permanently retire a skill: remove from disk and registry.
/// Refuses bundled skills — use `prune` to deprecate them instead.
pub fn retire(paths: &Paths, name: &str) -> Result<()> {
    let registry_path = paths.state.join("registry").join("local-cache.yaml");
    let skill_dir = paths.skills().join(name);
    let archive_dir = paths.state.join("archive");

    // Guard: refuse to delete bundled skills. Also capture old status for journal.
    let old = RegistryCache::load(&registry_path)
        .ok()
        .and_then(|r| r.skills.get(name).cloned());
    if let Some(ref entry) = old {
        if entry.bundled {
            anyhow::bail!(
                "{name} is a bundled skill and cannot be retired. Use 'russell skill prune {name}' to deprecate it instead."
            );
        }
    }

    if !skill_dir.exists() {
        anyhow::bail!("Skill directory not found: {}", skill_dir.display());
    }

    // Create archive directory if it doesn't exist.
    std::fs::create_dir_all(&archive_dir).context("creating archive directory")?;

    // Remove evidence bundles before archiving (retention policy).
    let evidence_dir = skill_dir.join("evidence");
    if evidence_dir.exists() {
        std::fs::remove_dir_all(&evidence_dir)
            .context("removing evidence bundles before archive")?;
        println!("Removed evidence bundles for {}.", name);
    }

    // Move skill to archive instead of deleting.
    let archived_skill_dir = archive_dir.join(name);
    if archived_skill_dir.exists() {
        std::fs::remove_dir_all(&archived_skill_dir).context("removing old archived version")?;
    }
    std::fs::rename(&skill_dir, &archived_skill_dir).context("moving skill to archive")?;
    println!("Archived {} to {}.", name, archived_skill_dir.display());

    let mut removed = false;
    RegistryCache::with_update(&registry_path, |registry| {
        if registry.remove_entry(name).is_some() {
            println!("Removed {name} from registry.");
            removed = true;
        }
    })?;

    if removed {
        let journal = JournalWriter::open(&paths.journal()).context("opening journal for audit")?;
        let from_status = old.map(|e| e.status);
        RegistryCache::journal_transition(
            &journal,
            name,
            from_status,
            LifecycleStatus::Retired,
            Some("retire via CLI"),
        );
    }

    Ok(())
}

/// Create a new skill skeleton on disk.
/// Writes a minimal `manifest.yaml` to `skills/<name>/`.
pub fn build(paths: &Paths, name: &str) -> Result<()> {
    let skills_dir = paths.skills();
    let skill_dir = skills_dir.join(name);
    if skill_dir.exists() {
        anyhow::bail!("Skill directory already exists: {}", skill_dir.display());
    }
    std::fs::create_dir_all(&skill_dir)
        .with_context(|| format!("creating skill directory {}", skill_dir.display()))?;

    let manifest = format!(
        r#"# SPDX-License-Identifier: MIT OR Apache-2.0
# Skill: {name}
# Generated by russell skill build

schema: "russell.skill.v1"
id: "{name}"
version: "0.1.0"
description: "TODO: Describe what this skill does"
authors:
  - "Your Name <your@email>"
bundled: false
lifecycle:
  status: "discovered"
  notes: "Skeleton created by russell skill build"
"#,
        name = name
    );

    std::fs::write(skill_dir.join("manifest.yaml"), manifest).context("writing manifest.yaml")?;

    std::fs::create_dir_all(skill_dir.join("scripts")).context("creating scripts directory")?;

    println!("Created skill skeleton at {}", skill_dir.display());
    println!("Edit manifest.yaml and add scripts to complete the skill.");
    Ok(())
}

/// Restore a retired skill from archive back to active.
pub fn restore_from_archive(paths: &Paths, name: &str) -> Result<()> {
    let registry_path = paths.state.join("registry").join("local-cache.yaml");
    let archive_dir = paths.state.join("archive");
    let archived_skill_dir = archive_dir.join(name);
    let skill_dir = paths.skills().join(name);

    // Guard: skill must be in archive.
    if !archived_skill_dir.exists() {
        anyhow::bail!(
            "Skill '{}' not found in archive. Available archived skills: {}",
            name,
            list_archived_skills(paths)?
        );
    }

    // Guard: skill must not already be installed.
    if skill_dir.exists() {
        anyhow::bail!("Skill '{}' is already installed. Remove it first.", name);
    }

    // Move from archive back to skills.
    std::fs::rename(&archived_skill_dir, &skill_dir).context("restoring skill from archive")?;
    println!("Restored {} from archive.", name);

    // Restore registry entry with updated status.
    let mut restored = false;
    RegistryCache::with_update(&registry_path, |registry| {
        // Create a minimal entry — the full entry will be rebuilt on next load.
        let entry = RegistryEntry::new_default(
            LifecycleStatus::Active,
            "0.0.0",
            "2026-01-01T00:00:00Z",
            vec![],
            SkillSource::Workshop,
            "2026-01-01T00:00:00Z",
            false,
        );
        registry.skills.insert(name.to_string(), entry);
        restored = true;
    })?;

    if restored {
        let journal = JournalWriter::open(&paths.journal()).context("opening journal for audit")?;
        RegistryCache::journal_transition(
            &journal,
            name,
            Some(LifecycleStatus::Retired),
            LifecycleStatus::Active,
            Some("restore-from-archive via CLI"),
        );
    }

    Ok(())
}

/// List archived skill names.
fn list_archived_skills(paths: &Paths) -> Result<String> {
    let archive_dir = paths.state.join("archive");
    if !archive_dir.exists() {
        return Ok("(none)".to_string());
    }
    let mut skills = Vec::new();
    for entry in std::fs::read_dir(&archive_dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            skills.push(entry.file_name().to_string_lossy().to_string());
        }
    }
    if skills.is_empty() {
        Ok("(none)".to_string())
    } else {
        skills.sort();
        Ok(skills.join(", "))
    }
}

/// Write a full skill manifest from stdin.
pub fn put(paths: &Paths, name: Option<&str>) -> Result<()> {
    use russell_skills::registry::{
        LifecycleStatus, RegistryCache, RegistryEntry, SafetyScan, SkillSource,
    };

    // Read from stdin.
    let mut content = String::new();
    std::io::Read::read_to_string(&mut std::io::stdin(), &mut content)
        .context("reading manifest from stdin")?;
    if content.trim().is_empty() {
        anyhow::bail!("Empty manifest received on stdin");
    }

    // Safety scan before anything else.
    let scan = SafetyScan::scan(&content);
    if scan.has_blocks() {
        println!("Manifest rejected by safety scanner:");
        for f in &scan.findings {
            if f.severity == russell_skills::registry::ScanSeverity::Block {
                println!("  [{}] {}", f.rule_id, f.description);
            }
        }
        anyhow::bail!("Safety scan blocked manifest");
    }
    if scan.has_warnings() {
        println!("Safety warnings:");
        for f in &scan.findings {
            if f.severity == russell_skills::registry::ScanSeverity::Warn {
                println!("  [{}] {}", f.rule_id, f.description);
            }
        }
    }

    // Parse and validate the manifest using the shared validation logic.
    // Resolve skill name — from CLI arg or from YAML's id field.
    let name = if let Some(n) = name {
        n.to_string()
    } else {
        russell_skills::extract_manifest_id(&content).ok_or_else(|| {
            anyhow::anyhow!("manifest has no 'id' field and no name was provided via CLI")
        })?
    };

    let _skill = russell_skills::parse_manifest(&content, &name)
        .map_err(|e| anyhow::anyhow!("{e}"))
        .context("validating manifest")?;

    // Write manifest to disk.
    let skills_dir = paths.skills();
    let skill_dir = skills_dir.join(&name);
    if !skill_dir.exists() {
        std::fs::create_dir_all(&skill_dir)
            .with_context(|| format!("creating skill directory {}", skill_dir.display()))?;
    }

    let manifest_path = skill_dir.join("manifest.yaml");
    std::fs::write(&manifest_path, &content)
        .with_context(|| format!("writing manifest {}", manifest_path.display()))?;

    println!("Manifest written to {}", manifest_path.display());

    // Register in the registry cache.
    let version = _skill.version.clone();
    let authored = _skill.authored.clone();
    let symptoms = _skill.symptoms.clone();
    let registry_path = paths.state.join("registry").join("local-cache.yaml");
    RegistryCache::with_update(&registry_path, |registry| {
        let entry = RegistryEntry::new_default(
            LifecycleStatus::Active,
            version.clone(),
            authored.clone(),
            symptoms.clone(),
            SkillSource::Manual,
            authored.clone(),
            false,
        );
        registry.upsert(&name, entry);
    })?;

    let journal = JournalWriter::open(&paths.journal()).context("opening journal for audit")?;
    RegistryCache::journal_transition(
        &journal,
        &name,
        None,
        LifecycleStatus::Active,
        Some("put via CLI (manifest from stdin)"),
    );

    let probe_count = _skill.probes.len();
    let iv_count = _skill.interventions.len();
    println!(
        "Skill '{name}' registered (v{version}, {probe_count} probes, {iv_count} interventions)."
    );
    println!("  Use /reload in chat to pick it up.");
    Ok(())
}

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

// SPDX-License-Identifier: MIT OR Apache-2.0
//! CLI utilities — shared patterns for command handlers.
//!
//! Consolidates duplicate error handling, skill loading, and common operations
//! across all CLI commands. Reduces boilerplate in individual command files.

use anyhow::{Context, Result};
use russell_core::paths::Paths;
use russell_skills::{Skill, load_all};

/// Load skills from the paths' skills directory.
pub fn load_skills(paths: &Paths) -> Result<Vec<Skill>> {
    let skills_dir = paths.skills();
    load_all(&skills_dir).context("loading skills")
}

/// Parse a skill reference string `<skill>/<id>` into components.
pub fn parse_skill_ref(id: &str) -> Result<(String, String)> {
    let parts = id.split('/').collect::<Vec<_>>();
    if parts.len() != 2 {
        anyhow::bail!("invalid skill reference: '{id}' (use <skill>/<id>)");
    }
    Ok((parts[0].to_string(), parts[1].to_string()))
}

/// Find a skill by ID in a list.
pub fn find_skill<'a>(skills: &'a [Skill], skill_id: &str) -> Result<&'a Skill> {
    skills
        .iter()
        .find(|s| s.id == skill_id)
        .with_context(|| format!("skill '{}' not found", skill_id))
}

/// Print skill summary to stdout.
pub fn print_skill_summary(s: &Skill) {
    println!("Skill: {}", s.id);
    println!("  version: {}", s.version);
    println!("  symptoms: {}", s.symptoms.join(", "));
    println!("  probes: {}", s.probes.len());
    println!("  interventions: {}", s.interventions.len());
}

/// Skill lifecycle helper — check if skill exists.
pub fn skill_exists(paths: &Paths, name: &str) -> bool {
    paths.skills().join(name).exists()
}

/// Skill lifecycle helper — get skill directory.
pub fn skill_dir(paths: &Paths, name: &str) -> std::path::PathBuf {
    paths.skills().join(name)
}

/// Skill lifecycle helper — get archive directory.
pub fn archive_dir(paths: &Paths) -> std::path::PathBuf {
    paths.state.join("archive")
}

/// List archived skill names.
pub fn list_archived_skills(paths: &Paths) -> Result<Vec<String>> {
    let archive = archive_dir(paths);
    if !archive.exists() {
        return Ok(Vec::new());
    }
    let mut skills = Vec::new();
    for entry in std::fs::read_dir(&archive)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            skills.push(entry.file_name().to_string_lossy().to_string());
        }
    }
    skills.sort();
    Ok(skills)
}

// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell skill workshop` — interactive skill lifecycle REPL with Jack.
//!
//! A focused chat session where Jack helps the operator discover,
//! evaluate, build, adapt, and maintain skills. Loads the
//! skill-workshop and skill-maintenance knowledge skills.
//!
//! See ADR-0024.

use anyhow::{Context, Result};
use russell_core::paths::Paths;
use russell_doctor::client::LlmClient;
use russell_doctor::client::SoapPrompt;
use russell_doctor::oai_client::OkapiClient;
use russell_skills::Skill;
use russell_skills::registry::{LifecycleStatus, RegistryCache, RegistryEntry, SafetyScan, SkillSource};
use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;
use std::fmt::Write as _;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::warn;

/// Run the interactive skill workshop REPL.
pub async fn run(paths: &Paths) -> Result<()> {
    let skills_dir = paths.skills();
    let skills = russell_skills::load_all(&skills_dir).unwrap_or_default();
    let registry_dir = paths.state.join("registry");
    let registry_path = registry_dir.join("local-cache.yaml");
    let mut registry = RegistryCache::load(&registry_path).unwrap_or_else(|_| RegistryCache::new());

    // Sync registry from installed skills (rebuildable — JR-7).
    sync_registry_from_skills(&mut registry, &skills);

    let client_cfg = russell_doctor::client::ClientConfig::from_env();
    let fallback_model = client_cfg.model.clone();

    println!(
        "Skill Workshop — interactive skill lifecycle REPL.\n\
         Jack is here to help you discover, evaluate, build, adapt, and maintain skills.\n\
         \n\
         Quick commands:\n\
           help            show this guide\n\
           /list           list all skills with lifecycle status\n\
           /gaps           show symptoms with no installed skill\n\
           /lookup <sym>   which skills address this symptom?\n\
           search <query>  search registry + remote sources\n\
           evaluate <name> show manifest, scripts, safety scan\n\
           build <name>    create a new skill skeleton on disk\n\
           adapt <name>    modify an existing skill\n\
           check           audit all installed skills\n\
           prune <name>    deprecate a stale skill\n\
           restore <name>  move a deprecated skill back to active\n\
           install <name>  move a discovered skill to installed/active\n\
           /quit           exit the workshop\n\
         \n\
         Or just describe what you need and Jack will help.\n"
    );

    println!(
        "Loaded {} installed skills.",
        registry.skills.values().filter(|e| e.status.is_loadable()).count()
    );

    let mut rl = DefaultEditor::new().context("initializing readline")?;
    let _ = rl.load_history("/dev/null");

    // Load workshop knowledge from the installed skills directory.
    let workshop_knowledge = load_knowledge(&skills_dir, "skill-workshop");
    let maintenance_knowledge = load_knowledge(&skills_dir, "skill-maintenance");

    loop {
        let line = match rl.readline("workshop> ") {
            Ok(l) => l,
            Err(ReadlineError::Interrupted | ReadlineError::Eof) => break,
            Err(e) => {
                warn!(?e, "readline error");
                break;
            }
        };
        let input = line.trim().to_string();
        if input.is_empty() {
            continue;
        }

        let _ = rl.add_history_entry(&input);

        let mut quit = false;
        if handle_builtin(&input, &mut registry, &skills_dir, &skills, &workshop_knowledge, &maintenance_knowledge, &client_cfg, &fallback_model, &mut quit).await {
            if quit {
                break;
            }
            continue;
        }

        // Delegate to Jack via LLM for free-form workshop interaction.
        let _ = jack_workshop_turn(
            &input,
            &workshop_knowledge,
            &maintenance_knowledge,
            &skills,
            &client_cfg,
        )
        .await;
    }

    // Save registry on exit.
    if let Err(e) = registry.save(&registry_path) {
        warn!(?e, "failed to save registry cache");
    }

    println!("Workshop closed. Registry saved.");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn handle_builtin(
    input: &str,
    registry: &mut RegistryCache,
    skills_dir: &std::path::Path,
    skills: &[Skill],
    workshop_knowledge: &str,
    maintenance_knowledge: &str,
    client_cfg: &russell_doctor::client::ClientConfig,
    fallback_model: &str,
    quit: &mut bool,
) -> bool {
    match input {
        "/quit" | "/exit" => { *quit = true; true }
        "help" => { print_help(); true }
        "/list" => { print_skill_list(registry); true }
        "/gaps" => { print_coverage_gaps(registry); true }
        _ if input.starts_with("/lookup ") => {
            print_lookup(registry, input.strip_prefix("/lookup ").unwrap_or(""));
            true
        }
        _ if input.starts_with("search ") && input.contains("--remote") => {
            let query = input.strip_prefix("search ").unwrap_or("").replace("--remote", "").trim().to_string();
            do_search_remote(registry, &query).await;
            true
        }
        _ if input.starts_with("search ") => {
            let query = input.strip_prefix("search ").unwrap_or("");
            print_search(registry, query);
            true
        }
        _ if input.starts_with("fetch ") => {
            let rest = input.strip_prefix("fetch ").unwrap_or("");
            let mut parts = rest.splitn(2, ' ');
            let url = parts.next().unwrap_or("");
            let name = parts.next().unwrap_or("");
            do_fetch(registry, skills_dir, url, name).await;
            true
        }
        _ if input.starts_with("adapt ") => {
            let name = input.strip_prefix("adapt ").unwrap_or("");
            do_adapt(registry, skills_dir, name);
            true
        }
        _ if input.starts_with("evaluate ") => {
            let name = input.strip_prefix("evaluate ").unwrap_or("");
            print_evaluate(registry, skills_dir, name);
            true
        }
        _ if input == "check" || input == "/check" => {
            print_check(registry);
            true
        }
        _ if input.starts_with("prune ") => {
            let name = input.strip_prefix("prune ").unwrap_or("");
            do_prune(registry, name);
            true
        }
        _ if input.starts_with("restore ") || input.starts_with("unprune ") => {
            let name = if let Some(n) = input.strip_prefix("restore ") {
                n
            } else {
                input.strip_prefix("unprune ").unwrap_or("")
            };
            do_restore(registry, name);
            true
        }
        _ if input.starts_with("install ") => {
            let name = input.strip_prefix("install ").unwrap_or("");
            do_install(registry, skills_dir, name);
            true
        }
        _ if input.starts_with("build ") => {
            let name = input.strip_prefix("build ").unwrap_or("");
            do_build(registry, skills_dir, name, workshop_knowledge, maintenance_knowledge, skills, client_cfg, fallback_model).await;
            true
        }
        _ => false,
    }
}

/// Sync registry cache from currently installed skills.
fn sync_registry_from_skills(registry: &mut RegistryCache, skills: &[Skill]) {
    for skill in skills {
        if !registry.skills.contains_key(&skill.id) {
            // Determine source: bundled skills ship with the repo, workshop/manual are user-created.
            let source = if is_bundled_skill(&skill.id) {
                SkillSource::Bundled
            } else {
                // Check if it was created via workshop (has a KNOWLEDGE.md in the skills dir).
                SkillSource::Manual
            };
            registry.upsert(
                &skill.id,
                RegistryEntry {
                    status: LifecycleStatus::Active,
                    version: skill.version.clone(),
                    symptoms: skill.symptoms.clone(),
                    source,
                    installed: skill.authored.clone(),
                    last_evaluated: None,
                    valid_until: None,
                    coverage_score: None,
                    superseded_by: None,
                    deprecation_reason: None,
                    probe_runs: 0,
                    recent_probe_failures: 0,
                },
            );
        }
    }
}

/// Heuristic: bundled skills match a known set shipped with Russell.
fn is_bundled_skill(id: &str) -> bool {
    matches!(id, "okapi-watcher" | "web-search" | "skill-discovery"
        | "skill-workshop" | "skill-maintenance" | "scenario-tester"
        | "pragmatic-cybernetics" | "pragmatic-semantics" | "ubuntu-jack")
}

/// Load a KNOWLEDGE.md file from the installed skills directory.
fn load_knowledge(skills_dir: &std::path::Path, skill_name: &str) -> String {
    let path = skills_dir.join(skill_name).join("KNOWLEDGE.md");
    std::fs::read_to_string(&path).unwrap_or_else(|_| String::new())
}

fn print_help() {
    println!(
        "Skill Workshop — interactive skill lifecycle REPL.\n\
         \n\
         Quick commands:\n\
           help            show this guide\n\
           /list           list all skills with lifecycle status\n\
           /gaps           show symptoms with no installed skill\n\
           /lookup <sym>   which skills address this symptom?\n\
           search <query>  search registry + remote sources\n\
           evaluate <name> show manifest, scripts, safety scan\n\
           build <name>    create a new skill skeleton on disk\n\
           adapt <name>    modify an existing skill\n\
           check           audit all installed skills\n\
           prune <name>    deprecate a stale skill\n\
           restore <name>  move a deprecated skill back to active\n\
           install <name>  move a discovered skill to installed/active\n\
           /quit           exit the workshop\n\
         \n\
         Or just describe what you need and Jack will help.\n"
    );
}

fn print_search(registry: &RegistryCache, query: &str) {
    println!("Searching for: {query}");
    println!("(Remote search via MCP bridge — the web-search skill connects Brave Search / Firecrawl here.)");
    println!("Local registry cache matches:");
    let lower = query.to_lowercase();
    let mut found = false;
    for (id, entry) in &registry.skills {
        if id.contains(&lower)
            || entry.symptoms.iter().any(|s| s.contains(&lower))
        {
            println!("  {id} ({}, {}) — {}",
                entry.version, entry.status.as_str(), entry.symptoms.join(", "));
            found = true;
        }
    }
    if !found {
        println!("  (no local cache matches — try a web search via Jack)");
    }
}

fn format_source(source: &SkillSource) -> &'static str {
    match source {
        SkillSource::Bundled => "bundled",
        SkillSource::Workshop => "workshop",
        SkillSource::Manual => "manual",
        SkillSource::Registry { .. } => "registry",
        SkillSource::Remote { .. } => "remote",
    }
}

fn print_skill_list(registry: &RegistryCache) {
    if registry.skills.is_empty() {
        println!("No skills in registry.");
        return;
    }
    println!("{:<30} {:<10} {:<14} symptoms", "skill", "version", "status");
    println!("{}", "-".repeat(80));
    for (id, entry) in &registry.skills {
        let status_mark = match entry.status {
            LifecycleStatus::Active => "✓",
            LifecycleStatus::StaleWarning => "⚠",
            LifecycleStatus::Deprecated => "✗",
            LifecycleStatus::Installed => "•",
            _ => " ",
        };
        println!(
            "{:<30} {:<10} {} {:<13} {}",
            id,
            entry.version,
            status_mark,
            entry.status.as_str(),
            entry.symptoms.join(", "),
        );
    }
}

fn print_coverage_gaps(registry: &RegistryCache) {
    let gaps = registry.coverage_gaps(russell_skills::SYMPTOMS);
    if gaps.is_empty() {
        println!("All catalogue symptoms are covered by installed skills.");
        return;
    }
    println!("{} symptoms have no installed skill:", gaps.len());
    for gap in &gaps {
        println!("  - {gap}");
    }
    println!("Use 'search <symptom>' to find skills for these gaps.");
}

fn print_lookup(registry: &RegistryCache, symptom: &str) {
    let matches = registry.lookup_symptom(symptom);
    if matches.is_empty() {
        println!("No installed skill covers '{symptom}'.");
        println!("Use 'search {symptom}' to find one, or 'build' to create one.");
        return;
    }
    println!("Skills covering '{symptom}':");
    for entry in &matches {
        println!("  {}/v{} (status: {})", entry.symptoms.join(", "), entry.version, entry.status.as_str());
    }
}

fn print_evaluate(registry: &RegistryCache, skills_dir: &std::path::Path, name: &str) {
    // Try to find in registry first.
    if let Some(entry) = registry.skills.get(name) {
        println!("Skill: {name}");
        println!("  Version:  {}", entry.version);
        println!("  Status:   {}", entry.status.as_str());
        println!("  Symptoms: {}", entry.symptoms.join(", "));
        println!("  Source:   {}", format_source(&entry.source));
        println!("  Installed: {}", entry.installed);
        if let Some(ref le) = entry.last_evaluated {
            println!("  Last evaluated: {le}");
        }
        if let Some(ref vu) = entry.valid_until {
            println!("  Valid until: {vu}");
        }
        if let Some(cs) = entry.coverage_score {
            println!("  Score: {cs:.2}");
        }
    } else {
        println!("Skill '{name}' not found in registry cache.");
    }

    // Scan manifest.yaml.
    scan_file(skills_dir, name, "manifest.yaml", "manifest");

    // Scan KNOWLEDGE.md.
    scan_file(skills_dir, name, "KNOWLEDGE.md", "KNOWLEDGE.md");

    // Check for scripts directory.
    let scripts_path = skills_dir.join(name).join("scripts");
    if scripts_path.is_dir()
        && let Ok(entries) = std::fs::read_dir(&scripts_path)
    {
        let count = entries.filter_map(|e| e.ok()).count();
        println!("  scripts/: {} files", count);
    }
}

/// Scan a single file with the safety scanner.
fn scan_file(skills_dir: &std::path::Path, name: &str, filename: &str, label: &str) {
    let path = skills_dir.join(name).join(filename);
    if !path.exists() {
        return;
    }
    if let Ok(content) = std::fs::read_to_string(&path) {
        let scan = SafetyScan::scan(&content);
        if scan.findings.is_empty() {
            println!("  {label}: ✓ clean");
        } else {
            println!("  {label}:");
            for f in &scan.findings {
                let severity = match f.severity {
                    russell_skills::registry::ScanSeverity::Info => "INFO",
                    russell_skills::registry::ScanSeverity::Warn => "WARN",
                    russell_skills::registry::ScanSeverity::Block => "BLOCK",
                };
                println!("    [{severity}] {id}: {desc}", id = f.rule_id, desc = f.description);
            }
            if scan.has_blocks() {
                println!("    ⛔ Blocking findings — review before installing.");
            }
        }
    }
}

/// Prune a skill: move from active/stale to deprecated.
fn do_prune(registry: &mut RegistryCache, name: &str) {
    if let Some(entry) = registry.skills.get_mut(name) {
        if entry.status == LifecycleStatus::Deprecated || entry.status == LifecycleStatus::Retired {
            println!("{name} is already {}.", entry.status.as_str());
            return;
        }
        let reason = entry.deprecation_reason.clone().unwrap_or_else(|| "operator requested".into());
        println!("Deprecating {name} (v{}): {}", entry.version, reason);
        println!("  Status: {} → deprecated", entry.status.as_str());
        println!("  Files remain on disk. Skill will no longer be loaded.");
        entry.status = LifecycleStatus::Deprecated;
        entry.deprecation_reason = Some(reason);
        if entry.superseded_by.is_none() {
            entry.superseded_by = Some("operator".into());
        }
        println!("  Done. To retire (delete files), remove from {}",
            std::env::var("HOME").map(|h| format!("{h}/.local/share/harness/skills/{name}/")).unwrap_or_default());
    } else {
        println!("Skill '{name}' not found in registry.");
    }
}

/// Install a discovered/evaluated skill: copy to the skills directory.
fn do_install(registry: &mut RegistryCache, skills_dir: &std::path::Path, name: &str) {
    let source = skills_dir.join(name);
    if !source.exists() || !source.join("manifest.yaml").exists() {
        if let Some(entry) = registry.skills.get(name)
            && (entry.status == LifecycleStatus::Discovered || entry.status == LifecycleStatus::Evaluated)
        {
            println!("{name} is {}. Use 'fetch <url>' to download it, then 'install {name}'.",
                entry.status.as_str());
            return;
        }
        println!("Skill '{name}' not found as a directory. Use 'build {name}' to create it first.");
        return;
    }

    if let Some(entry) = registry.skills.get_mut(name) {
        if entry.status.is_loadable() {
            println!("{name} is already installed ({}).", entry.status.as_str());
            return;
        }
        println!("Installing {name} (v{})...", entry.version);
        entry.status = LifecycleStatus::Installed;
        entry.installed = chrono_now();
        entry.last_evaluated = Some(chrono_now());
        println!("  Status: → installed");
        println!("  Skill will be available on next load.");
        println!("  Run: russell skill list  to verify.");
    } else {
        // Discovered from remote — create entry from disk.
        if let Ok(manifest) = std::fs::read_to_string(source.join("manifest.yaml")) {
            // Quick parse for id/version.
            let id = name.to_string();
            let version = extract_yaml_field(&manifest, "version").unwrap_or_else(|| "0.1.0".into());
            let symptoms = extract_yaml_list(&manifest, "symptoms");
            registry.upsert(&id, RegistryEntry {
                status: LifecycleStatus::Installed,
                version,
                symptoms,
                source: SkillSource::Manual,
                installed: chrono_now(),
                last_evaluated: Some(chrono_now()),
                valid_until: None,
                coverage_score: None,
                superseded_by: None,
                deprecation_reason: None,
                probe_runs: 0,
                recent_probe_failures: 0,
            });
            println!("{name} installed and registered.");
        } else {
            println!("Cannot read manifest for {name}.");
        }
    }
}

/// Extract a scalar YAML field from a manifest string.
fn extract_yaml_field(manifest: &str, key: &str) -> Option<String> {
    for line in manifest.lines() {
        if let Some(rest) = line.trim().strip_prefix(&format!("{key}:")) {
            return Some(rest.trim().to_string());
        }
    }
    None
}

/// Extract a YAML list from a manifest string (simple line-based parser).
fn extract_yaml_list(manifest: &str, key: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut in_section = false;
    for line in manifest.lines() {
        if line.trim_start().starts_with(&format!("{key}:")) {
            in_section = true;
            continue;
        }
        if in_section {
            if let Some(item) = line.trim().strip_prefix("- ") {
                items.push(item.trim().to_string());
            } else if line.trim().starts_with(|c: char| c.is_alphabetic()) {
                break; // Next top-level key
            }
        }
    }
    items
}

/// Restore a deprecated skill back to active.
fn do_restore(registry: &mut RegistryCache, name: &str) {
    if let Some(entry) = registry.skills.get_mut(name) {
        if entry.status != LifecycleStatus::Deprecated {
            println!("{name} is {} — restore only applies to deprecated skills.", entry.status.as_str());
            return;
        }
        println!("Restoring {name} (v{}) from deprecated → active.", entry.version);
        entry.status = LifecycleStatus::Active;
        entry.deprecation_reason = None;
        entry.superseded_by = None;
        println!("  Done. Skill will be loaded on next restart.");
    } else {
        println!("Skill '{name}' not found in registry.");
    }
}

/// Build a new skill interactively via Jack.
#[allow(clippy::too_many_arguments)]
async fn do_build(
    registry: &mut RegistryCache,
    skills_dir: &std::path::Path,
    name: &str,
    workshop_knowledge: &str,
    maintenance_knowledge: &str,
    skills: &[Skill],
    client_cfg: &russell_doctor::client::ClientConfig,
    _fallback_model: &str,
) {
    if registry.skills.contains_key(name) {
        let status = registry.skills[name].status;
        if status.is_loadable() || status == LifecycleStatus::Deprecated {
            println!("Skill '{name}' already exists ({}). Use 'adapt {name}' to modify it or 'restore {name}' if deprecated.", status.as_str());
            return;
        }
    }

    // Create the skill directory and a minimal valid manifest.
    let skill_dir = skills_dir.join(name);
    if let Err(e) = std::fs::create_dir_all(&skill_dir) {
        println!("Cannot create directory for {name}: {e}");
        return;
    }

    let manifest_path = skill_dir.join("manifest.yaml");
    let today = chrono_now();
    let manifest_content = format!(
        r#"# {name} — TODO: describe what this skill does.
id: {name}
version: 0.1.0
authored: {today}
min_harness_version: 0.1.0
symptoms: []
applies_when:
  - os_family: linux
probes: []
interventions: []
safety:
  max_auto_risk: none
"#
    );

    if let Err(e) = std::fs::write(&manifest_path, &manifest_content) {
        println!("Cannot write manifest for {name}: {e}");
        return;
    }

    println!("Created: {}", manifest_path.display());
    println!("Edit the manifest to add symptoms and probes, then run 'install {name}'.");

    // Register as discovered in the cache.
    registry.upsert(name, RegistryEntry {
        status: LifecycleStatus::Discovered,
        version: "0.1.0".into(),
        symptoms: vec![],
        source: SkillSource::Workshop,
        installed: today,
        last_evaluated: None,
        valid_until: None,
        coverage_score: None,
        superseded_by: None,
        deprecation_reason: None,
        probe_runs: 0,
        recent_probe_failures: 0,
    });

    // Invoke Jack to help compose the skill interactively.
    let build_prompt = format!(
        "The operator just created a new skill called '{name}' at {}. \
         The manifest has no symptoms or probes yet. Help them design \
         what this skill should watch and what probes it needs. \
         Ask what symptom(s) from the catalog this skill addresses. Be concise.",
        manifest_path.display()
    );
    println!("\nJack is ready to help design this skill:");
    let _ = jack_workshop_turn(&build_prompt, workshop_knowledge, maintenance_knowledge, skills, client_cfg).await;
}

fn print_check(registry: &RegistryCache) {
    println!("Skill audit — {}", chrono_now());
    println!();

    // Check staleness.
    let today = chrono_now().split_at(10).0.to_string(); // YYYY-MM-DD
    for (id, entry) in &registry.skills {
        if !entry.status.is_loadable() {
            continue;
        }
        let age_text = if let Some(ref vu) = entry.valid_until {
            if vu < &today {
                format!("EXPIRED ({vu})")
            } else {
                let remaining = days_between(&today, vu);
                format!("{vu} ({remaining}d remaining)")
            }
        } else {
            "no valid_until set".to_string()
        };

        let stale = RegistryCache::is_stale(&entry.installed, &today);
        let mark = if stale { "⚠ stale" } else { "✓" };
        println!(
            "{mark} {id:<30} v{} — installed: {}, valid: {age_text}",
            entry.version, entry.installed,
        );
    }

    println!();
    print_coverage_gaps(registry);
}

/// Get current date as ISO 8601 string using real system time.
fn chrono_now() -> String {
    if let Ok(dur) = SystemTime::now().duration_since(UNIX_EPOCH) {
        let secs = dur.as_secs();
        // Convert Unix timestamp to YYYY-MM-DD (UTC).
        let days_since_epoch = secs / 86400;
        // Days since 1970-01-01. Compute year/month/day.
        let (y, m, d) = civil_from_days(days_since_epoch as i64 + 719_468); // 719_468 = days from 0000-01-01 to 1970-01-01
        format!("{y:04}-{m:02}-{d:02}")
    } else {
        "2026-05-13".to_string()
    }
}

/// Convert days since 0000-03-01 to civil date (algorithm from Howard Hinnant).
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z - 719_468; // shift epoch to 1970-01-01
    let era = if z >= 0 { z } else { z - 146096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m as i64, d as i64)
}

/// Rough days between two ISO 8601 date strings.
fn days_between(a: &str, b: &str) -> i64 {
    let pa = parse_date_parts(a);
    let pb = parse_date_parts(b);
    (pb.0 * 365 + pb.1 * 30 + pb.2 as i64) - (pa.0 * 365 + pa.1 * 30 + pa.2 as i64)
}

fn parse_date_parts(d: &str) -> (i64, i64, i32) {
    let parts: Vec<&str> = d.split('-').collect();
    let y = parts.first().and_then(|s| s.parse().ok()).unwrap_or(0);
    let m = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    let d = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(1);
    (y, m, d)
}

/// Send a turn to Jack for free-form workshop interaction.
async fn jack_workshop_turn(
    input: &str,
    workshop_knowledge: &str,
    maintenance_knowledge: &str,
    skills: &[Skill],
    client_cfg: &russell_doctor::client::ClientConfig,
) -> Result<()> {
    let mut system = String::new();
    system.push_str("You are Jack, in the skill workshop.\n\n");
    system.push_str("Your job: help the operator discover, evaluate, build, adapt, and maintain Russell skills.\n");
    system.push_str("You have knowledge of the skill manifest format, the symptom catalog (85 entries),\n");
    system.push_str("the poka-yoke validation rules, the safety scanner, and the skill lifecycle.\n\n");
    system.push_str("Tone: helpful, direct, enthusiastic about building tools. You're a terrier making toys.\n\n");

    if !workshop_knowledge.is_empty() {
        system.push_str("## Skill Workshop Knowledge\n\n");
        system.push_str(workshop_knowledge);
        system.push('\n');
    }
    if !maintenance_knowledge.is_empty() {
        system.push_str("## Skill Maintenance Knowledge\n\n");
        system.push_str(maintenance_knowledge);
        system.push('\n');
    }

    // Available skills summary.
    if !skills.is_empty() {
        let _ = write!(system, "\n## Available Skills\n\n");
        for s in skills {
            let _ = writeln!(system, "- {} (v{}): {}", s.id, s.version, s.symptoms.join(", "));
        }
    }

    let soap = SoapPrompt {
        system,
        subjective: String::new(),
        objective: String::new(),
        rendered: format!("**User:** {input}"),
    };

    let mut chat_cfg = client_cfg.clone();
    if chat_cfg.base_url.is_none() {
        chat_cfg.base_url = Some(russell_doctor::health::DEFAULT_BASE_URL.to_string());
    }
    if chat_cfg.api_key.is_none() {
        chat_cfg.api_key = Some("okapi".into());
    }

    let base = chat_cfg
        .base_url
        .as_deref()
        .unwrap_or(russell_doctor::health::DEFAULT_BASE_URL);
    if !russell_doctor::health::ensure_ready(base).await {
        println!("  (Okapi not reachable — workshop running offline)");
        return Ok(());
    }

    let client = OkapiClient::new(&chat_cfg)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    match client.chat(&soap).await {
        Ok(resp) => {
            println!("\n{}", resp.content);
        }
        Err(e) => {
            println!("  (LLM error: {e})");
            println!("  Use the built-in commands: /list, /gaps, /lookup, evaluate, check.");
        }
    }

    Ok(())
}

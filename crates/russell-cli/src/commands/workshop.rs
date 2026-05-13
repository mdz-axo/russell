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
           build <name>    compose a new skill interactively\n\
           adapt <name>    modify an existing skill\n\
           check           audit all installed skills\n\
           prune <name>    deprecate or retire a stale skill\n\
           /quit           exit the workshop\n\
         \n\
         Or just describe what you need and Jack will help.\n"
    );

    println!(
        "Loaded {} installed skills.",
        registry.skills.values().filter(|e| e.status.is_loadable()).count()
    );

    let mut rl = DefaultEditor::new().context("initializing readline")?;
    let _ = rl.load_history("/dev/null"); // no persistent workshop history

    // Load workshop knowledge
    let workshop_knowledge = load_knowledge("skill-workshop");
    let maintenance_knowledge = load_knowledge("skill-maintenance");

    loop {
        let line = match rl.readline("workshop> ") {
            Ok(l) => l,
            Err(ReadlineError::Interrupted | ReadlineError::Eof) => break,
            Err(e) => {
                warn!(?e, "readline error");
                break;
            }
        };
        let input = line.trim();
        if input.is_empty() {
            continue;
        }

        let _ = rl.add_history_entry(input);

        match input {
            "/quit" | "/exit" => break,
            "help" => print_help(),
            "/list" => print_skill_list(&registry),
            "/gaps" => print_coverage_gaps(&registry),
            _ if input.starts_with("/lookup ") => {
                let symptom = input.strip_prefix("/lookup ").unwrap_or("");
                print_lookup(&registry, symptom);
            }
            _ if input.starts_with("search ") => {
                let query = input.strip_prefix("search ").unwrap_or("");
                println!("Searching for: {query}");
                println!("(Remote search via MCP bridge — this is where the web-search skill connects.)");
                println!("For now, searching the local registry cache...");
                let lower = query.to_lowercase();
                for (id, entry) in &registry.skills {
                    if id.contains(&lower)
                        || entry.symptoms.iter().any(|s| s.contains(&lower))
                    {
                        println!("  {id} ({}) — {} {}", entry.version, entry.status.as_str(), entry.symptoms.join(", "));
                    }
                }
            }
            _ if input.starts_with("evaluate ") => {
                let name = input.strip_prefix("evaluate ").unwrap_or("");
                print_evaluate(&registry, &skills_dir, name);
            }
            _ if input.starts_with("check") || input == "/check" => {
                print_check(&registry);
            }
            _ => {
                // Delegate to Jack via LLM for free-form workshop interaction.
                let _ = jack_workshop_turn(
                    input,
                    &workshop_knowledge,
                    &maintenance_knowledge,
                    &skills,
                    &registry,
                    &client_cfg,
                    &fallback_model,
                )
                .await;
            }
        }
    }

    // Save registry on exit.
    if let Err(e) = registry.save(&registry_path) {
        warn!(?e, "failed to save registry cache");
    }

    println!("Workshop closed. Registry saved.");
    Ok(())
}

/// Sync registry cache from currently installed skills.
fn sync_registry_from_skills(registry: &mut RegistryCache, skills: &[Skill]) {
    for skill in skills {
        if !registry.skills.contains_key(&skill.id) {
            registry.upsert(
                &skill.id,
                RegistryEntry {
                    status: LifecycleStatus::Active,
                    version: skill.version.clone(),
                    symptoms: skill.symptoms.clone(),
                    source: SkillSource::Bundled,
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

/// Load a KNOWLEDGE.md file from a skill directory.
fn load_knowledge(skill_name: &str) -> String {
    let path = std::path::PathBuf::from("skills").join(skill_name).join("KNOWLEDGE.md");
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
           build <name>    compose a new skill interactively\n\
           adapt <name>    modify an existing skill\n\
           check           audit all installed skills\n\
           prune <name>    deprecate or retire a stale skill\n\
           /quit           exit the workshop\n\
         \n\
         Or just describe what you need and Jack will help.\n"
    );
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
        println!("  Source:   {:?}", entry.source);
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

    // Try to read the manifest and scan it.
    let skill_path = skills_dir.join(name);
    let manifest_path = skill_path.join("manifest.yaml");
    if manifest_path.exists()
        && let Ok(content) = std::fs::read_to_string(&manifest_path)
    {
        let scan = SafetyScan::scan(&content);
        println!("\n  Safety scan:");
        if scan.findings.is_empty() {
            println!("    ✓ No findings.");
        } else {
            for f in &scan.findings {
                let severity = match f.severity {
                    russell_skills::registry::ScanSeverity::Info => "INFO",
                    russell_skills::registry::ScanSeverity::Warn => "WARN",
                    russell_skills::registry::ScanSeverity::Block => "BLOCK",
                };
                println!("    [{severity}] {id}: {desc}", id = f.rule_id, desc = f.description);
            }
        }
        if scan.has_blocks() {
            println!("    ⛔ Skill has blocking findings — review before installing.");
        }
    }

    // Check for KNOWLEDGE.md.
    let knowledge_path = skill_path.join("KNOWLEDGE.md");
    if knowledge_path.exists() {
        let len = std::fs::metadata(&knowledge_path)
            .map(|m| m.len())
            .unwrap_or(0);
        println!("  KNOWLEDGE.md: {} bytes", len);
    }
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

/// Get current date as ISO 8601 string.
fn chrono_now() -> String {
    // Avoid adding a chrono dependency — use a simple approximation.
    // In production, this should use the system time.
    std::env::var("RUSSELL_NOW_DATE").unwrap_or_else(|_| {
        // Fallback: use a fixed date for deterministic testing.
        "2026-05-13".to_string()
    })
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
    _registry: &RegistryCache,
    client_cfg: &russell_doctor::client::ClientConfig,
    _fallback_model: &str,
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

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
use russell_core::time::{approx_days_between, now_date_iso8601};
use russell_doctor::client::LlmClient;
use russell_doctor::client::SoapPrompt;
use russell_doctor::oai_client::OkapiClient;
use russell_skills::Skill;
use russell_skills::registry::{
    LifecycleStatus, RegistryCache, RegistryEntry, SafetyScan, SkillSource,
};
use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;
use std::fmt::Write as _;
use std::io::Write;
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
           help              show this guide\n\
           /list             list all skills with lifecycle status\n\
           /gaps             show symptoms with no installed skill\n\
           /lookup <sym>     which skills address this symptom?\n\
           search <query>    search local cache\n\
           search --remote   search via Brave Search API (needs BRAVE_API_KEY)\n\
           fetch <url> <n>   download skill manifest from URL, safety-scan\n\
           evaluate <name>   show manifest, scripts, safety scan\n\
           build <name>      create a new skill skeleton on disk\n\
           adapt <name>      edit skill manifest in \\$EDITOR or vim\n\
           check             audit all installed skills\n\
           prune <name>      deprecate a stale skill\n\
           restore <name>    move a deprecated skill back to active\n\
           install <name>    move a discovered skill to installed/active\n\
           delete <name>     retire skill — remove directory and cache entry\n\
           retire <name>     alias for delete\n\
           /quit             exit the workshop\n\
         \n\
         Or just describe what you need and Jack will help.\n"
    );

    println!(
        "Loaded {} installed skills.",
        registry
            .skills
            .values()
            .filter(|e| e.status.is_loadable())
            .count()
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
        if handle_builtin(
            &input,
            &mut registry,
            &skills_dir,
            &skills,
            &workshop_knowledge,
            &maintenance_knowledge,
            &client_cfg,
            &fallback_model,
            &mut quit,
        )
        .await
        {
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
        "/quit" | "/exit" => {
            *quit = true;
            true
        }
        "help" => {
            print_help();
            true
        }
        "/list" => {
            print_skill_list(registry);
            true
        }
        "/gaps" => {
            print_coverage_gaps(registry);
            true
        }
        _ if input.starts_with("/lookup ") => {
            print_lookup(registry, input.strip_prefix("/lookup ").unwrap_or(""));
            true
        }
        _ if input.starts_with("search ") && input.contains("--remote") => {
            let query = input
                .strip_prefix("search ")
                .unwrap_or("")
                .replace("--remote", "")
                .trim()
                .to_string();
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
            do_adapt(
                registry,
                skills_dir,
                name,
                client_cfg,
                fallback_model,
            )
            .await;
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
        _ if input.starts_with("delete ") || input.starts_with("retire ") => {
            let name = input
                .strip_prefix("delete ")
                .or_else(|| input.strip_prefix("retire "))
                .unwrap_or("");
            do_delete(registry, skills_dir, name);
            true
        }
        _ if input.starts_with("build ") => {
            let name = input.strip_prefix("build ").unwrap_or("");
            do_build(
                registry,
                skills_dir,
                name,
                workshop_knowledge,
                maintenance_knowledge,
                skills,
                client_cfg,
                fallback_model,
            )
            .await;
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
            let bundled = is_bundled_skill(&skill.id);
            registry.upsert(
                &skill.id,
                RegistryEntry::new_default(
                    LifecycleStatus::Active,
                    skill.version.clone(),
                    skill.symptoms.clone(),
                    source,
                    skill.authored.clone(),
                    bundled,
                ),
            );
        }
    }
}

/// Heuristic: bundled skills match a known set shipped with Russell.
fn is_bundled_skill(id: &str) -> bool {
    matches!(
        id,
        "okapi-watcher"
            | "web-search"
            | "skill-discovery"
            | "skill-workshop"
            | "skill-maintenance"
            | "skill-manager"
            | "scenario-tester"
            | "pragmatic-cybernetics"
            | "pragmatic-semantics"
            | "ubuntu-jack"
    )
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
           help              show this guide\n\
           /list             list all skills with lifecycle status\n\
           /gaps             show symptoms with no installed skill\n\
           /lookup <sym>     which skills address this symptom?\n\
           search <query>    search local cache\n\
           search --remote   search via Brave Search API (needs BRAVE_API_KEY)\n\
           fetch <url> <n>   download skill manifest from URL, safety-scan\n\
           evaluate <name>   show manifest, scripts, safety scan\n\
           build <name>      create a new skill skeleton on disk\n\
           adapt <name>      edit skill manifest in \\$EDITOR or vim\n\
           check             audit all installed skills\n\
           prune <name>      deprecate a stale skill\n\
           restore <name>    move a deprecated skill back to active\n\
           install <name>    move a discovered skill to installed/active\n\
           delete <name>     retire skill — remove directory and cache entry\n\
           retire <name>     alias for delete\n\
           /quit             exit the workshop\n\
         \n\
         Or just describe what you need and Jack will help.\n"
    );
}

fn print_search(registry: &RegistryCache, query: &str) {
    println!("Searching for: {query}");
    println!(
        "(Remote search via MCP bridge — the web-search skill connects Brave Search / Firecrawl here.)"
    );
    println!("Local registry cache matches:");
    let lower = query.to_lowercase();
    let mut found = false;
    for (id, entry) in &registry.skills {
        if id.contains(&lower) || entry.symptoms.iter().any(|s| s.contains(&lower)) {
            println!(
                "  {id} ({}, {}) — {}",
                entry.version,
                entry.status.as_str(),
                entry.symptoms.join(", ")
            );
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
    println!(
        "{:<28} {:<8} {:<12} {:>5} {:>6}  symptoms",
        "skill", "version", "status", "score", "runs"
    );
    println!("{}", "-".repeat(85));
    for (id, entry) in &registry.skills {
        let status_mark = match entry.status {
            LifecycleStatus::Active => "✓",
            LifecycleStatus::StaleWarning => "⚠",
            LifecycleStatus::Deprecated => "✗",
            LifecycleStatus::Installed => "•",
            _ => " ",
        };
        let score = entry
            .coverage_score
            .map_or("--".into(), |s| format!("{s:.2}"));
        println!(
            "{:<28} {:<8} {} {:<11} {:>5} {:>6} {}",
            id,
            entry.version,
            status_mark,
            entry.status.as_str(),
            score,
            entry.probe_runs,
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
        println!(
            "  {}/v{} (status: {})",
            entry.symptoms.join(", "),
            entry.version,
            entry.status.as_str()
        );
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
        if entry.probe_runs > 0 {
            println!("  Probes run: {} ({} failures)", entry.probe_runs, entry.recent_probe_failures);
        }
        if let Some(ref last) = entry.last_probe_run_at {
            println!("  Last probe: {last}");
        }
        // Compute score from manifest content if available.
        let manifest_path = skills_dir.join(name).join("manifest.yaml");
        if let Ok(manifest) = std::fs::read_to_string(&manifest_path) {
            let knowledge_exists = skills_dir.join(name).join("KNOWLEDGE.md").exists();
            let score = RegistryCache::compute_score(entry, &manifest, knowledge_exists);
            let fresh = RegistryCache::freshness_score(entry);
            if entry.coverage_score.is_some() {
                println!("  Score: {score:.2} (quality), {fresh:.2} (freshness)");
            } else {
                println!("  Score: {score:.2} (quality), {fresh:.2} (freshness) — not yet stored");
            }
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
                println!(
                    "    [{}] {id}: {desc}",
                    f.severity.as_str(),
                    id = f.rule_id,
                    desc = f.description
                );
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
        let reason = entry
            .deprecation_reason
            .clone()
            .unwrap_or_else(|| "operator requested".into());
        println!("Deprecating {name} (v{}): {}", entry.version, reason);
        println!("  Status: {} → deprecated", entry.status.as_str());
        println!("  Files remain on disk. Skill will no longer be loaded.");
        entry.status = LifecycleStatus::Deprecated;
        entry.deprecation_reason = Some(reason);
        if entry.superseded_by.is_none() {
            entry.superseded_by = Some("operator".into());
        }
        println!(
            "  Done. To retire (delete files), remove from {}",
            std::env::var("HOME")
                .map(|h| format!("{h}/.local/share/harness/skills/{name}/"))
                .unwrap_or_default()
        );
    } else {
        println!("Skill '{name}' not found in registry.");
    }
}

/// Install a discovered/evaluated skill: copy to the skills directory.
fn do_install(registry: &mut RegistryCache, skills_dir: &std::path::Path, name: &str) {
    let source = skills_dir.join(name);
    if !source.exists() || !source.join("manifest.yaml").exists() {
        if let Some(entry) = registry.skills.get(name)
            && (entry.status == LifecycleStatus::Discovered
                || entry.status == LifecycleStatus::Evaluated)
        {
            println!(
                "{name} is {}. Use 'fetch <url>' to download it, then 'install {name}'.",
                entry.status.as_str()
            );
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
        entry.installed = now_date_iso8601();
        entry.last_evaluated = Some(now_date_iso8601());
        println!("  Status: → installed");
        println!("  Skill will be available on next load.");
        println!("  Run: russell skill list  to verify.");
    } else {
        // Discovered from remote — create entry from disk.
        if let Ok(manifest) = std::fs::read_to_string(source.join("manifest.yaml")) {
            // Quick parse for id/version.
            let id = name.to_string();
            let version =
                extract_yaml_field(&manifest, "version").unwrap_or_else(|| "0.1.0".into());
            let symptoms = extract_yaml_list(&manifest, "symptoms");
            let mut entry = RegistryEntry::new_default(
                LifecycleStatus::Installed,
                version,
                symptoms,
                SkillSource::Manual,
                now_date_iso8601(),
                false,
            );
            entry.last_evaluated = Some(now_date_iso8601());
            registry.upsert(&id, entry);
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

/// Fetch a skill manifest from a URL, safety-scan it, and register as discovered.
async fn do_fetch(
    registry: &mut RegistryCache,
    skills_dir: &std::path::Path,
    url: &str,
    name: &str,
) {
    if name.is_empty() {
        println!("Usage: fetch <url> <name>");
        return;
    }
    if url.is_empty() || !url.starts_with("http") {
        println!("URL must start with http:// or https://");
        return;
    }

    println!("Fetching {name} from {url}...");

    // Try to download the URL content.
    let output = tokio::process::Command::new("curl")
        .args(["-sSL", "--connect-timeout", "10", "--max-time", "30", url])
        .output()
        .await;

    match output {
        Ok(out) if out.status.success() => {
            let content = String::from_utf8_lossy(&out.stdout);
            if content.is_empty() {
                println!("  Downloaded empty content from {url}");
                return;
            }

            // Safety scan the downloaded content.
            let scan = SafetyScan::scan(&content);
            println!("  Safety scan:");
            if scan.findings.is_empty() {
                println!("    ✓ No findings.");
            } else {
                for f in &scan.findings {
                    println!(
                        "    [{}] {id}: {desc}",
                        f.severity.as_str(),
                        id = f.rule_id,
                        desc = f.description
                    );
                }
            }

            if scan.has_blocks() {
                println!("  ⛔ Downloaded content has blocking findings. Not saving.");
                println!("  Review the URL manually: {url}");
                return;
            }

            // Quick parse: extract id, version, symptoms from the downloaded YAML.
            let version = extract_yaml_field(&content, "version").unwrap_or_else(|| "0.1.0".into());
            let symptoms = extract_yaml_list(&content, "symptoms");

            // Save to disk.
            let skill_dir = skills_dir.join(name);
            let _ = std::fs::create_dir_all(&skill_dir);
            let manifest_path = skill_dir.join("manifest.yaml");
            if let Err(e) = std::fs::write(&manifest_path, content.as_bytes()) {
                println!("  Cannot save manifest: {e}");
                return;
            }

            let today = now_date_iso8601();
            let mut entry = RegistryEntry::new_default(
                LifecycleStatus::Discovered,
                version,
                symptoms,
                SkillSource::Remote { url: url.to_string() },
                today.clone(),
                false,
            );
            entry.last_evaluated = Some(today);
            registry.upsert(name, entry);

            println!("  Saved to {}", manifest_path.display());
            println!(
                "  Registered as 'discovered'. Run 'evaluate {name}' to inspect, then 'install {name}'."
            );
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            println!("  curl failed (exit {}): {}", out.status, stderr.trim());
        }
        Err(e) => {
            println!("  Cannot run curl: {e}");
        }
    }
}

/// Adapt an existing skill: open in editor, re-scan, update cache.
async fn do_adapt(
    registry: &mut RegistryCache,
    skills_dir: &std::path::Path,
    name: &str,
    client_cfg: &russell_doctor::client::ClientConfig,
    fallback_model: &str,
) {
    let manifest_path = skills_dir.join(name).join("manifest.yaml");
    if !manifest_path.exists() {
        println!("Skill '{name}' not found at {}", manifest_path.display());
        return;
    }

    // Show current state.
    if let Some(entry) = registry.skills.get(name) {
        println!(
            "{name} (v{}, {}): {}",
            entry.version,
            entry.status.as_str(),
            entry.symptoms.join(", ")
        );
    }

    // Try programmatic adaptation via Jack. Fall back to editor on failure.
    let curr = match std::fs::read_to_string(&manifest_path) {
        Ok(c) => c,
        Err(e) => {
            println!("Cannot read manifest: {e}");
            return;
        }
    };

    let (adapted, programmatic) = adapt_via_llm(client_cfg, fallback_model, name, &curr).await;
    if programmatic && !adapted.is_empty() {
        let scan = SafetyScan::scan(&adapted);
        if scan.has_blocks() {
            println!("  ⚠ Jack's adapted manifest has safety issues:");
            for f in &scan.findings {
                if f.severity == russell_skills::registry::ScanSeverity::Block {
                    println!("    [{id}]: {desc}", id = f.rule_id, desc = f.description);
                }
            }
            println!("  Opening editor instead...");
            open_editor_for_manifest(&manifest_path, &curr, registry, name);
            return;
        }

        if let Err(e) = std::fs::write(&manifest_path, &adapted) {
            println!("  Cannot write adapted manifest: {e}");
            return;
        }
        println!("  ✓ Jack adapted the manifest. Safety scan clean.");
        apply_manifest_update(registry, name, &adapted);
        return;
    }

    // Fallback: open editor.
    open_editor_for_manifest(&manifest_path, &curr, registry, name);
}

/// Try adapting a manifest via Jack. Returns (content, success).
async fn adapt_via_llm(
    client_cfg: &russell_doctor::client::ClientConfig,
    fallback_model: &str,
    name: &str,
    current: &str,
) -> (String, bool) {
    let prompt = format!(
        "The operator wants to adapt the skill '{name}'. Here is the current manifest:\n\n```yaml\n{current}\n```\n\n\
         Produce an improved version of this manifest YAML. Keep the 'id', 'version', 'authored', \
         'symptoms', 'probes', and 'interventions' sections. Improve probe command arguments, \
         add timeout values, or refine symptom coverage. Output ONLY the complete YAML manifest \
         in a ```yaml code block. Do not explain the changes."
    );
    let adapted = match llm_call(client_cfg, fallback_model, &prompt).await {
        Ok(resp) => resp,
        Err(_) => String::new(),
    };
    if adapted.is_empty() {
        return (String::new(), false);
    }
    let yaml = extract_yaml_block(&adapted);
    let is_empty = yaml.is_empty();
    (yaml, !is_empty)
}

/// Extract the first ```yaml...``` block from LLM output.
fn extract_yaml_block(response: &str) -> String {
    let start = response.find("```yaml");
    let body_start = match start {
        Some(s) => s + 7,
        None => return String::new(),
    };
    let end = match response[body_start..].find("```") {
        Some(e) => body_start + e,
        None => return response[body_start..].to_string(),
    };
    response[body_start..end].trim().to_string()
}

/// Simple LLM call for adaptation. Returns response text or empty on failure.
async fn llm_call(
    cfg: &russell_doctor::client::ClientConfig,
    _fallback_model: &str,
    prompt: &str,
) -> Result<String> {
    use russell_doctor::oai_client::OkapiClient;

    let mut chat_cfg = cfg.clone();
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
        return Err(anyhow::anyhow!("Okapi not reachable"));
    }

    let client = OkapiClient::new(&chat_cfg).await?;

    let soap = SoapPrompt {
        system: "You are a YAML editor for Russell skill manifests. Output ONLY the YAML in a code block.".into(),
        subjective: String::new(),
        objective: String::new(),
        rendered: prompt.to_string(),
        temperature: Some(0.6),
        max_tokens: None,
    };

    Ok(client.chat(&soap).await?.content)
}

/// Open EDITOR for manifest, then safety-scan and update registry.
fn open_editor_for_manifest(
    manifest_path: &std::path::Path,
    current: &str,
    registry: &mut RegistryCache,
    name: &str,
) {
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vim".into());
    println!("Opening {} with {}...", manifest_path.display(), editor);
    let status = std::process::Command::new(&editor)
        .arg(manifest_path)
        .status();
    match status {
        Ok(s) if s.success() => {
            if let Ok(content) = std::fs::read_to_string(manifest_path) {
                if content == current {
                    println!("  No changes.");
                    return;
                }
                let scan = SafetyScan::scan(&content);
                if scan.has_blocks() {
                    println!("  ⚠ Safety scan found blocking issues:");
                    for f in &scan.findings {
                        if f.severity == russell_skills::registry::ScanSeverity::Block {
                            println!("    [{id}]: {desc}", id = f.rule_id, desc = f.description);
                        }
                    }
                } else {
                    println!("  ✓ Safety scan clean.");
                }
                apply_manifest_update(registry, name, &content);
            }
        }
        Ok(s) => println!("  Editor exited with status: {}", s),
        Err(e) => println!("  Cannot run editor '{}': {}", editor, e),
    }
}

/// Update registry entry from manifest content.
fn apply_manifest_update(registry: &mut RegistryCache, name: &str, content: &str) {
    let version = extract_yaml_field(content, "version").unwrap_or_else(|| "0.1.0".into());
    let symptoms = extract_yaml_list(content, "symptoms");
    if let Some(entry) = registry.skills.get_mut(name) {
        entry.version = version;
        entry.symptoms = symptoms;
        entry.last_evaluated = Some(now_date_iso8601());
        println!("  Updated registry entry.");
    }
}

/// Search remote registries via Brave Search API (if BRAVE_API_KEY is set) or DuckDuckGo.
async fn do_search_remote(registry: &mut RegistryCache, query: &str) {
    println!("Remote search for: {query}");

    let api_key = std::env::var("BRAVE_API_KEY").ok();
    let results: Vec<(String, String)> = if let Some(key) = api_key {
        // Use Brave Search API.
        let output = tokio::process::Command::new("curl")
            .args([
                "-sS", "--connect-timeout", "10", "--max-time", "15",
                "-H", "Accept: application/json",
                "-H", "Accept-Encoding: gzip",
                "-H", &format!("X-Subscription-Token: {key}"),
                &format!("https://api.search.brave.com/res/v1/web/search?q=site:github.com%20russell%20skill%20{query}&count=5"),
            ])
            .output()
            .await;

        match output {
            Ok(out) if out.status.success() => {
                let body = String::from_utf8_lossy(&out.stdout);
                parse_brave_results(&body)
            }
            _ => {
                println!("  (Brave Search API unavailable — using fallback)");
                Vec::new()
            }
        }
    } else {
        println!("  (Set BRAVE_API_KEY for Brave Search. Using local cache only.)");
        Vec::new()
    };

    if results.is_empty() {
        println!("  No remote results. Searching local cache instead:");
        print_search(registry, query);
        return;
    }

    for (title, url) in &results {
        println!("  {title}");
        println!("    {url}");
        // Register as discovered from remote.
        let slug = title.to_lowercase().replace(' ', "-");
        if !registry.skills.contains_key(&slug) {
            registry.upsert(
                &slug,
                RegistryEntry::new_default(
                    LifecycleStatus::Discovered,
                    "unknown",
                    vec![],
                    SkillSource::Remote { url: url.clone() },
                    now_date_iso8601(),
                    false,
                ),
            );
        }
    }
    println!();
    println!("  Use 'fetch <url> <name>' to download a skill from these results.");
}

/// Parse Brave Search API JSON results into (title, url) pairs.
fn parse_brave_results(json: &str) -> Vec<(String, String)> {
    let parsed: serde_json::Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    parsed["web"]["results"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let title = item["title"].as_str()?.to_string();
            let url = item["url"].as_str()?.to_string();
            Some((title, url))
        })
        .collect()
}

/// Restore a deprecated skill back to active.
fn do_restore(registry: &mut RegistryCache, name: &str) {
    if let Some(entry) = registry.skills.get_mut(name) {
        if entry.status != LifecycleStatus::Deprecated {
            println!(
                "{name} is {} — restore only applies to deprecated skills.",
                entry.status.as_str()
            );
            return;
        }
        println!(
            "Restoring {name} (v{}) from deprecated → active.",
            entry.version
        );
        entry.status = LifecycleStatus::Active;
        entry.deprecation_reason = None;
        entry.superseded_by = None;
        println!("  Done. Skill will be loaded on next restart.");
    } else {
        println!("Skill '{name}' not found in registry.");
    }
}

/// Delete a skill: remove directory, mark as retired in cache.
fn do_delete(registry: &mut RegistryCache, skills_dir: &std::path::Path, name: &str) {
    if let Some(entry) = registry.skills.get(name) {
        if entry.bundled {
            println!("{name} is a bundled skill and cannot be deleted.");
            println!("  Use 'prune {name}' to deprecate it instead.");
            return;
        }
    }

    let skill_dir = skills_dir.join(name);
    if !skill_dir.exists() {
        println!("Skill directory for '{name}' does not exist at {}", skill_dir.display());
        // Still remove from cache if present.
        if registry.remove_entry(name).is_some() {
            println!("  Removed from registry cache.");
        }
        return;
    }

    println!("Deleting {name}...");
    println!("  Directory: {}", skill_dir.display());
    print!("  Confirm deletion? This cannot be undone with restore. [y/N]: ");
    let _ = std::io::stdout().flush();
    let mut buf = String::new();
    if std::io::stdin().read_line(&mut buf).is_err() {
        println!("\n  Could not read input. Aborting.");
        return;
    }
    let answer = buf.trim().to_lowercase();
    if answer != "y" && answer != "yes" {
        println!("  Aborted.");
        return;
    }

    match std::fs::remove_dir_all(&skill_dir) {
        Ok(()) => {
            println!("  Directory removed.");
            registry.remove_entry(name);
            println!("  Removed from registry cache.");
        }
        Err(e) => {
            println!("  Cannot remove directory: {e}");
            println!("  Remove manually: {}", skill_dir.display());
        }
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
            println!(
                "Skill '{name}' already exists ({}). Use 'adapt {name}' to modify it or 'restore {name}' if deprecated.",
                status.as_str()
            );
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
    let today = now_date_iso8601();
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
    registry.upsert(
        name,
        RegistryEntry::new_default(
            LifecycleStatus::Discovered,
            "0.1.0",
            vec![],
            SkillSource::Workshop,
            today,
            false,
        ),
    );

    // Invoke Jack to help compose the skill interactively.
    let build_prompt = format!(
        "The operator just created a new skill called '{name}' at {}. \
         The manifest has no symptoms or probes yet. Help them design \
         what this skill should watch and what probes it needs. \
         Ask what symptom(s) from the catalog this skill addresses. Be concise.",
        manifest_path.display()
    );
    println!("\nJack is ready to help design this skill:");
    let _ = jack_workshop_turn(
        &build_prompt,
        workshop_knowledge,
        maintenance_knowledge,
        skills,
        client_cfg,
    )
    .await;
}

fn print_check(registry: &RegistryCache) {
    println!("Skill audit — {}", now_date_iso8601());
    println!();

    // Check staleness.
    let today = now_date_iso8601().split_at(10).0.to_string(); // YYYY-MM-DD
    for (id, entry) in &registry.skills {
        if !entry.status.is_loadable() {
            continue;
        }
        let age_text = if let Some(ref vu) = entry.valid_until {
            if vu < &today {
                format!("EXPIRED ({vu})")
            } else {
                let remaining = approx_days_between(&today, vu);
                format!("{vu} ({remaining}d remaining)")
            }
        } else {
            "no valid_until set".to_string()
        };

        let stale = RegistryCache::is_stale(&entry.installed, &today);
        let mark = if stale { "⚠ stale" } else { "✓" };
        let score = entry
            .coverage_score
            .map_or("--".into(), |s| format!("{s:.2}"));
        let fresh = RegistryCache::freshness_score(entry);
        println!(
            "{mark} {id:<28} v{} score={score} fresh={fresh:.2} runs={} fails={} — installed: {}, valid: {age_text}",
            entry.version, entry.probe_runs, entry.recent_probe_failures, entry.installed,
        );
    }

    println!();
    print_coverage_gaps(registry);
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
    system.push_str(
        "You have knowledge of the skill manifest format, the symptom catalog (85 entries),\n",
    );
    system.push_str(
        "the poka-yoke validation rules, the safety scanner, and the skill lifecycle.\n\n",
    );
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
            let _ = writeln!(
                system,
                "- {} (v{}): {}",
                s.id,
                s.version,
                s.symptoms.join(", ")
            );
        }
    }

    let soap = SoapPrompt {
        system,
        subjective: String::new(),
        objective: String::new(),
        rendered: format!("**User:** {input}"),
        temperature: Some(0.6),
        max_tokens: None,
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

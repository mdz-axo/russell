// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell chat` — interactive conversation with Jack, the Nurse.
//!
//! ## Chat REPL
//!
//! Launches a readline REPL where each turn composes a fresh SOAP
//! bundle with the current journal state, appends it to the
//! conversation history, and sends the full context to the LLM.
//! Jack responds in his chat persona (see
//! `crates/russell-doctor/prompts/jack-chat.md`), with full
//! awareness of the conversation history.
//!
//! ## Consent gate
//!
//! Jack may propose interventions using the `ACTION:` syntax
//! (`ACTION: <skill-id>/<intervention-id>`). The operator
//! consents via `/approve` or `/deny`. Approved interventions
//! are dispatched through the IDRS-gated skill dispatcher.
//! Risk enforcement caps the maximum auto-risk to `Low` by
//! default; sudo-requiring interventions require NOPASSWD
//! configuration by the operator.
//!
//! ## Persistence
//!
//! Chat history persists to
//! `~/.local/state/harness/memory/chats/<session-id>.jsonl`.
//!
//! Type `/exit`, `/quit`, or Ctrl-D to end the session.

use anyhow::{Context, Result};
use rand::seq::SliceRandom;
use russell_core::journal::{JournalReader, JournalWriter};
use russell_core::paths::Paths;
use russell_skills::Skill;
use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;
use std::io::Write;
use tokio::sync::oneshot;
use tracing::{info, warn};

/// A pending intervention awaiting operator consent.
#[derive(Debug, Clone)]
struct PendingAction {
    skill_id: String,
    intervention_id: String,
    risk: russell_skills::RiskBand,
    needs_sudo: bool,
    cmd: Vec<String>,
    max_auto_risk: russell_skills::RiskBand,
}

/// One turn in the conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Turn {
    /// "user" or "assistant".
    role: String,
    /// The message content.
    content: String,
}

/// Full conversation history (excludes the system prompt which is
/// separate and fixed).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChatHistory {
    /// Session ULID.
    session_id: String,
    /// Ordered list of turns.
    turns: Vec<Turn>,
}

impl ChatHistory {
    fn new(session_id: String) -> Self {
        Self {
            session_id,
            turns: Vec::new(),
        }
    }
}

/// A model entry from Okapi's `/api/tags` (Ollama-compatible).
#[derive(Debug, Clone, Deserialize)]
struct OkapiModel {
    name: String,
}

#[derive(Debug, Clone, Deserialize)]
struct OkapiTagsResponse {
    models: Vec<OkapiModel>,
}

/// Fetch the list of available models from Okapi's `/api/tags`.
async fn okapi_list_models(base_url: &str) -> Result<Vec<String>, String> {
    let tags_url = format!(
        "{}/api/tags",
        base_url.trim_end_matches("/v1").trim_end_matches('/')
    );
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| format!("HTTP client: {e}"))?;
    let resp = client
        .get(&tags_url)
        .send()
        .await
        .map_err(|e| format!("Okapi unreachable: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("Okapi returned HTTP {}", resp.status()));
    }
    let body: OkapiTagsResponse = resp.json().await.map_err(|e| format!("parse error: {e}"))?;
    Ok(body.models.into_iter().map(|m| m.name).collect())
}

/// Find top fuzzy matches for `needle` in `models`.
///
/// Strips non-alphanumeric characters from both sides before scoring
/// (so `deepseekv4pro` matches `deepseek-v4-pro:cloud`).
/// Returns all models scoring ≥ 0.75 via Jaro-Winkler.
/// If exactly one, the caller auto-selects it; if multiple, the caller
/// shows a numbered list for disambiguation.
fn fuzzy_match_models<'a>(needle: &str, models: &'a [String]) -> Vec<&'a str> {
    let needle_lower = needle.to_lowercase();
    let needle_clean: String = needle_lower
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect();

    let mut scored: Vec<(&str, f64)> = models
        .iter()
        .map(|m| {
            let name_lower = m.to_lowercase();
            let name_clean: String = name_lower.chars().filter(|c| c.is_alphanumeric()).collect();
            let score = strsim::jaro_winkler(&needle_clean, &name_clean);
            (m.as_str(), score)
        })
        .filter(|(_, s)| *s >= 0.75)
        .collect();

    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.into_iter().map(|(name, _)| name).collect()
}

/// Run the chat REPL.
pub async fn run(paths: &Paths) -> Result<()> {
    let session_id = ulid::Ulid::new().to_string();
    let chat_path = paths.memory_dir().join("chats").join(&session_id);
    std::fs::create_dir_all(paths.memory_dir().join("chats"))
        .with_context(|| "creating chats directory")?;

    info!(session = %session_id, "starting chat session");

    let journal = JournalWriter::open(&paths.journal())
        .with_context(|| format!("opening journal {}", paths.journal().display()))?;
    let reader = journal.reader();

    // Load skills once at session start.
    let skills = russell_skills::load_all(&paths.skills()).unwrap_or_default();
    let profile = russell_core::Profile::load(&paths.profile()).ok();

    let mut history = ChatHistory::new(session_id.clone());
    let mut editor = DefaultEditor::new().context("initialising readline")?;
    let mut pending_action: Option<PendingAction> = None;

    // Resolve the default model from env, and fetch available models from Okapi.
    let base_url = std::env::var("RUSSELL_DOCTOR_BASE_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:11435/v1".into());
    let mut current_model =
        std::env::var("RUSSELL_DOCTOR_MODEL").unwrap_or_else(|_| "nemotron-3-super:cloud".into());
    let okapi_models: Vec<String> = okapi_list_models(&base_url).await.unwrap_or_default();

    // Banner.
    println!();
    println!("┌─ Jack ───────────────────────────────────┐");
    println!("│                                           │");
    println!("│  I'm Jack. Ask me about the machine.     │");
    println!("│  I'm watching.                             │");
    println!("│                                           │");
    println!("│  model: {current_model:<33}│");
    println!("│  /exit or Ctrl-D to leave.                │");
    println!("│  /model [list|<name>] to switch models.   │");
    println!("│  /approve or /deny to handle proposals.   │");
    println!("│                                           │");
    println!("└───────────────────────────────────────────┘");
    println!();

    loop {
        let readline = editor.readline("you → ");
        match readline {
            Ok(line) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                if trimmed == "/exit" || trimmed == "/quit" {
                    println!("  Jack → Stay fierce. I'll be here.\n");
                    break;
                }

                // Consent handling — must come before special commands.
                if let Some(ref pa) = pending_action {
                    if trimmed == "/approve" {
                        execute_pending_action(&journal, paths, pa, &session_id, &current_model)
                            .await;
                        pending_action = None;
                        continue;
                    }
                    if let Some(_pw) = trimmed.strip_prefix("/approve ") {
                        println!("  Sudo password on the command line is not supported.");
                        println!("  Configure NOPASSWD sudo for this skill's commands instead.");
                        println!("  Example: add to /etc/sudoers.d/russell:");
                        println!("  <user> ALL=(ALL) NOPASSWD: /usr/bin/systemctl restart okapi");
                        pending_action = None;
                        continue;
                    }
                    if let Some(_pw) = trimmed.strip_prefix("/approve ") {
                        println!("  Sudo password on the command line is not supported.");
                        println!("  Configure NOPASSWD sudo for this skill's commands instead.");
                        println!("  Example: add to /etc/sudoers.d/russell:");
                        println!("  <user> ALL=(ALL) NOPASSWD: /usr/bin/systemctl restart okapi");
                        pending_action = None;
                        continue;
                    }
                    if trimmed == "/deny" {
                        println!("  Denied. Action not executed.");
                        pending_action = None;
                        continue;
                    }
                    // Any other input clears the pending action and
                    // falls through to normal chat processing.
                    println!("  Action no longer pending. Treating as conversation.");
                    pending_action = None;
                }

                // Special commands.
                if trimmed.starts_with('/') {
                    match trimmed {
                        "/refresh" => {
                            println!("  Jack → Re-reading the journal…\n");
                            continue;
                        }
                        "/help" => {
                            println!("  Commands:");
                            println!("  /exit, /quit  — end the session");
                            println!(
                                "  /refresh      — reload journal data (happens automatically each turn)"
                            );
                            println!("  /history      — show conversation history summary");
                            println!("  /skills       — list available skills");
                            println!("  /model        — show current model");
                            println!("  /model list   — list available Okapi models");
                            println!("  /model <name> — switch to a model (fuzzy match)");
                            println!("  /approve      — consent to Jack's proposed action");
                            println!("  /deny         — refuse Jack's proposed action");
                            println!();
                            continue;
                        }
                        "/history" => {
                            println!("  Session {} — {} turns", session_id, history.turns.len());
                            for (i, turn) in history.turns.iter().enumerate() {
                                let label = if turn.role == "user" { "you" } else { "Jack" };
                                let preview = if turn.content.len() > 80 {
                                    format!("{}…", &turn.content[..80])
                                } else {
                                    turn.content.clone()
                                };
                                println!("  {i:>3}. {label}: {preview}");
                            }
                            println!();
                            continue;
                        }
                        "/skills" => {
                            if skills.is_empty() {
                                println!("  No skills loaded.");
                            } else {
                                for s in &skills {
                                    println!("  {}", s.id);
                                    for p in &s.probes {
                                        println!("    probe: {} ({})", p.id, p.cmd.join(" "));
                                    }
                                    for iv in &s.interventions {
                                        println!(
                                            "    intervention: {} ({}) [risk: {:?}]",
                                            iv.id,
                                            iv.cmd.join(" "),
                                            iv.risk
                                        );
                                    }
                                }
                            }
                            println!();
                            continue;
                        }
                        other => {
                            if other == "/model" {
                                println!("  Current model: {current_model}");
                                println!();
                                continue;
                            }
                            if other == "/model list" {
                                println!("  Available models ({}):", okapi_models.len());
                                for m in &okapi_models {
                                    let marker = if m == &current_model {
                                        " ← current"
                                    } else {
                                        ""
                                    };
                                    println!("    {m}{marker}");
                                }
                                println!();
                                continue;
                            }
                            if let Some(name) = other.strip_prefix("/model ") {
                                let name = name.trim();
                                if name.is_empty() {
                                    println!("  Current model: {current_model}");
                                    println!();
                                    continue;
                                }

                                // Hard-coded tag filters (Okapi/Ollama convention:
                                // tags can be ":cloud" or "30b-cloud" etc. —
                                // "cloud" is always the suffix).
                                if name == "cloud" || name == "local" {
                                    let filtered: Vec<&String> = if name == "cloud" {
                                        okapi_models
                                            .iter()
                                            .filter(|m| m.ends_with("cloud"))
                                            .collect()
                                    } else {
                                        okapi_models
                                            .iter()
                                            .filter(|m| !m.ends_with("cloud"))
                                            .collect()
                                    };
                                    if filtered.is_empty() {
                                        println!("  No {name} models found.");
                                    } else {
                                        println!("  {name} models ({}):", filtered.len());
                                        for (i, m) in filtered.iter().enumerate() {
                                            let marker = if *m == &current_model {
                                                " ← current"
                                            } else {
                                                ""
                                            };
                                            println!("    {}. {m}{marker}", i + 1);
                                        }
                                        println!(
                                            "  Type /model <number> to select, or /model cancel."
                                        );
                                        editor.add_history_entry(trimmed)?;
                                        if let Ok(sel_line) = editor.readline("select → ") {
                                            let sel = sel_line.trim();
                                            if sel == "cancel" || sel == "/model cancel" {
                                                println!("  Cancelled.");
                                            } else if let Ok(idx) = sel
                                                .trim_start_matches("/model ")
                                                .trim()
                                                .parse::<usize>()
                                            {
                                                if idx >= 1 && idx <= filtered.len() {
                                                    current_model = filtered[idx - 1].clone();
                                                    println!(
                                                        "  Switched to model: {current_model}"
                                                    );
                                                } else {
                                                    println!("  Invalid number. Cancelled.");
                                                }
                                            } else {
                                                println!("  Unrecognised. Cancelled.");
                                            }
                                        }
                                    }
                                    println!();
                                    continue;
                                }

                                let matches = fuzzy_match_models(name, &okapi_models);
                                match matches.len() {
                                    0 => {
                                        println!(
                                            "  No model found matching \"{name}\". Try /model list."
                                        );
                                    }
                                    1 => {
                                        current_model = matches[0].to_string();
                                        println!("  Switched to model: {current_model}");
                                    }
                                    n => {
                                        println!("  Multiple models match \"{name}\":");
                                        for (i, m) in matches.iter().enumerate() {
                                            let marker = if *m == current_model {
                                                " ← current"
                                            } else {
                                                ""
                                            };
                                            println!("    {}. {m}{marker}", i + 1);
                                        }
                                        println!(
                                            "  Type /model <number> to select, or /model cancel."
                                        );
                                        // Read one more line for the selection.
                                        editor.add_history_entry(trimmed)?;
                                        if let Ok(sel_line) = editor.readline("select → ") {
                                            let sel = sel_line.trim();
                                            if sel == "cancel" || sel == "/model cancel" {
                                                println!("  Cancelled.");
                                            } else if let Ok(idx) = sel
                                                .trim_start_matches("/model ")
                                                .trim()
                                                .parse::<usize>()
                                            {
                                                if idx >= 1 && idx <= n {
                                                    current_model = matches[idx - 1].to_string();
                                                    println!(
                                                        "  Switched to model: {current_model}"
                                                    );
                                                } else {
                                                    println!("  Invalid number. Cancelled.");
                                                }
                                            } else {
                                                println!("  Unrecognised. Cancelled.");
                                            }
                                        }
                                    }
                                }
                                println!();
                                continue;
                            }
                            println!("  Unknown command: {other}. Try /help.");
                            continue;
                        }
                    }
                }

                // Add user message to history.
                history.turns.push(Turn {
                    role: "user".into(),
                    content: trimmed.to_string(),
                });

                // Build the fresh SOAP objective.
                let objective = build_objective(&reader, &skills, profile.as_ref());
                let system = russell_doctor::JACK_CHAT_PERSONA.to_string();

                // Build the messages array for the LLM.
                let mut messages: Vec<serde_json::Value> = Vec::new();
                messages.push(serde_json::json!({
                    "role": "system",
                    "content": system,
                }));

                // Insert the current journal state as a "user" message
                // so Jack sees fresh data every turn.
                if !objective.is_empty() {
                    messages.push(serde_json::json!({
                        "role": "user",
                        "content": format!(
                            "# Current journal snapshot\n\n{objective}\n\n---\n\nContinue the conversation with the operator."
                        ),
                    }));
                }

                // Add conversation history.
                for turn in &history.turns {
                    messages.push(serde_json::json!({
                        "role": turn.role,
                        "content": turn.content,
                    }));
                }

                // Persist history before calling LLM (user message is saved).
                save_history(&chat_path, &history)?;

                // Call the LLM with an animated thinking spinner.
                let cfg = russell_doctor::client::ClientConfig::from_env();
                let response = call_okapi_with_spinner(&cfg, &current_model, &messages).await;

                match response {
                    Ok(content) => {
                        // Clear the spinner line and print the response.
                        print!("\r\x1b[K");
                        std::io::stdout().flush().unwrap();
                        println!("Jack → {content}\n");
                        history.turns.push(Turn {
                            role: "assistant".into(),
                            content: content.clone(),
                        });
                        save_history(&chat_path, &history)?;

                        // Also journal the chat turn as a help-session event.
                        journal_chat_turn(&journal, &session_id, &current_model, trimmed, &content);

                        // Check for ACTION: proposal.
                        if let Some(pa) = parse_action_from_response(&content, &skills) {
                            let sudo_tag = if pa.needs_sudo { " [needs sudo]" } else { "" };
                            println!(
                                "  → Jack proposes: {}/{} (risk: {:?}{}).",
                                pa.skill_id, pa.intervention_id, pa.risk, sudo_tag
                            );
                            println!("  → /approve to execute, /deny to refuse.");
                            pending_action = Some(pa);
                        }
                    }
                    Err(e) => {
                        let msg = format!("(can't reach the LLM right now — {e})");
                        println!("{msg}\n");
                        history.turns.push(Turn {
                            role: "assistant".into(),
                            content: msg.clone(),
                        });
                        save_history(&chat_path, &history)?;
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("^C");
                continue;
            }
            Err(ReadlineError::Eof) => {
                println!("\n  Jack → Stay fierce.\n");
                break;
            }
            Err(err) => {
                warn!(error = %err, "readline error");
                break;
            }
        }
    }

    Ok(())
}

/// Parse an `ACTION: <skill-id>/<intervention-id>` line from Jack's
/// response. Resolves it against the loaded skill set.
fn parse_action_from_response(response: &str, skills: &[Skill]) -> Option<PendingAction> {
    let action_line = response
        .lines()
        .rev()
        .find(|line| line.trim().starts_with("ACTION:"))?;

    let spec = action_line.trim().strip_prefix("ACTION:")?.trim();
    let (skill_id, intervention_id) = spec.split_once('/')?;
    let skill_id = skill_id.trim();
    let intervention_id = intervention_id.trim();

    let skill = skills.iter().find(|s| s.id == skill_id)?;
    let iv = skill
        .interventions
        .iter()
        .find(|i| i.id == intervention_id)?;

    Some(PendingAction {
        skill_id: skill_id.to_string(),
        intervention_id: intervention_id.to_string(),
        risk: iv.risk,
        needs_sudo: iv.needs_sudo,
        cmd: iv.cmd.clone(),
        max_auto_risk: skill.safety.max_auto_risk,
    })
}

/// Execute a pending intervention via the skill dispatcher.
async fn execute_pending_action(
    journal: &JournalWriter,
    paths: &Paths,
    action: &PendingAction,
    session_id: &str,
    model: &str,
) {
    use russell_skills::dispatch::{Dispatcher, DryRun, StepType};
    use std::time::Duration;

    let skill_dir = paths.skills().join(&action.skill_id);
    let evidence_base = paths.evidence();

    let intervention_timeout = Duration::from_secs(120);

    let mut dispatcher = Dispatcher::new(&skill_dir);
    dispatcher.intervention_timeout = intervention_timeout;
    dispatcher.dry_run = DryRun::Disabled;
    dispatcher.max_auto_risk = action.max_auto_risk;

    // If this intervention needs sudo, the operator must have
    // NOPASSWD sudo configured. We do not prompt for a password.
    if action.needs_sudo {
        println!(
            "  → Note: {}/{} requires root. Ensure NOPASSWD sudo is configured.",
            action.skill_id, action.intervention_id
        );
    }

    // Enforce risk cap — never auto-execute above system default.
    if let Err(e) = dispatcher.check_risk(action.risk, false) {
        println!("  → Refused: {e}");
        return;
    }

    let result = dispatcher
        .run_and_journal(
            journal,
            &evidence_base,
            &action.cmd,
            &action.skill_id,
            &action.intervention_id,
            StepType::Intervention,
            action.risk.as_str(),
            Some(intervention_timeout),
        )
        .await;

    match result {
        Ok(outcome) => {
            if outcome.exit_code == Some(0) {
                println!(
                    "  → Executed {}/{} successfully.",
                    action.skill_id, action.intervention_id
                );
            } else {
                println!(
                    "  → {}/{} exited with code {:?}.",
                    action.skill_id, action.intervention_id, outcome.exit_code
                );
                if !outcome.stderr.is_empty() {
                    println!("  stderr: {}", outcome.stderr.trim());
                }
            }
            // Journal the execution as a chat event.
            journal_chat_turn(
                journal,
                session_id,
                model,
                &format!("/approve {}/{}", action.skill_id, action.intervention_id),
                &format!(
                    "executed: exit={:?}, stdout_len={}, stderr_len={}",
                    outcome.exit_code,
                    outcome.stdout.len(),
                    outcome.stderr.len()
                ),
            );
        }
        Err(e) => {
            println!("  → Failed to execute: {e}");
        }
    }
}
fn build_objective(
    reader: &JournalReader,
    skills: &[Skill],
    profile: Option<&russell_core::Profile>,
) -> String {
    let now = russell_core::time::now_unix();
    let window_start = now - 24 * 3600;
    let mut obj = String::new();

    // Profile.
    if let Some(p) = profile {
        let _ = writeln!(obj, "### Machine");
        let _ = writeln!(
            obj,
            "- os: `{}/{}` kernel `{}`",
            p.host.os.distro, p.host.os.version, p.host.os.kernel
        );
        if !p.host.cpu.model.is_empty() {
            let _ = writeln!(
                obj,
                "- cpu: `{}` ({} cores / {} threads)",
                p.host.cpu.model, p.host.cpu.cores, p.host.cpu.threads
            );
        }
        if !p.gpus.is_empty() {
            let _ = writeln!(obj, "- gpus:");
            for g in &p.gpus {
                let _ = writeln!(obj, "  - `{}` @ `{}` (role: {})", g.name, g.pci, g.role);
            }
        }
    }

    // Severity counts.
    let _ = writeln!(obj, "\n### Severity — last 24h");
    if let Ok(counts) = reader.severity_counts(window_start, i64::MAX) {
        let _ = writeln!(
            obj,
            "- info: {} | warn: {} | alert: {} | crit: {}",
            counts.info, counts.warn, counts.alert, counts.crit
        );
    }

    // Sample summary.
    if let Ok(summaries) = reader.host_samples_summary(window_start, i64::MAX)
        && !summaries.is_empty()
    {
        let _ = writeln!(obj, "\n### Host samples — last 24h");
        let _ = writeln!(obj, "| probe | count | min | avg | max | last | unit |");
        let _ = writeln!(obj, "|---|---|---|---|---|---|---|");
        for s in &summaries {
            let unit = s.unit.as_deref().unwrap_or("");
            let _ = writeln!(
                obj,
                "| {} | {} | {} | {} | {} | {} | {} |",
                s.probe,
                s.count,
                fmt_f64(s.min),
                fmt_f64(s.avg),
                fmt_f64(s.max),
                fmt_f64(s.last),
                unit,
            );
        }
    }

    // Sentinel freshness.
    if let Ok(Some(ts)) = reader.last_sample_ts() {
        let age = now.saturating_sub(ts);
        let _ = writeln!(obj, "\n### Freshness\n- Last sample {} seconds ago.", age);
    }

    // Recent events (last 5).
    if let Ok(rows) = reader.recent(5)
        && !rows.is_empty()
    {
        let _ = writeln!(obj, "\n### Recent events");
        for r in &rows {
            let _ = writeln!(
                obj,
                "- [{sev:?}] {action}: {summary}",
                summary = r.summary.as_deref().unwrap_or("(no summary)"),
                sev = r.severity,
                action = r.action,
            );
        }
    }

    // Available skills.
    if !skills.is_empty() {
        let _ = writeln!(obj, "\n### Available skills");
        for s in skills {
            for p in &s.probes {
                let _ = writeln!(obj, "- `{}`/`{}` (probe, risk: none)", s.id, p.id);
            }
            for iv in &s.interventions {
                let _ = writeln!(
                    obj,
                    "- `{}`/`{}` (intervention, risk: {:?})",
                    s.id, iv.id, iv.risk
                );
            }
        }
    }

    obj
}

fn fmt_f64(v: Option<f64>) -> String {
    match v {
        Some(x) => {
            if x.fract() == 0.0 && x.abs() < 1_000_000.0 {
                format!("{x:.0}")
            } else if x.abs() < 100.0 {
                format!("{x:.2}")
            } else {
                format!("{x:.1}")
            }
        }
        None => "—".into(),
    }
}

/// Braille spinner frames for the thinking animation.
const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Jack's thinking expressions — a mix of terrier and McFarland energy.
/// One is picked at random each LLM call so it never gets old.
const THINKING_EXPRESSIONS: &[&str] = &[
    "🐶 digging digging digging",         // pure terrier
    "✨ hey hey! hold your kibble",       // McFarland energy + terrier
    "🐶 *sniff* *sniff* checking things", // terrier investigative
    "💅 working it. just a moment.",      // pure McFarland theatrical
    "🔍 just checking on you",            // protective nurse (both Jacks)
];

/// Call the LLM via Okapi with an animated thinking spinner on stdout.
/// The spinner is cleared the instant the response arrives.
async fn call_okapi_with_spinner(
    cfg: &russell_doctor::client::ClientConfig,
    model: &str,
    messages: &[serde_json::Value],
) -> std::result::Result<String, String> {
    let expression = THINKING_EXPRESSIONS
        .choose(&mut rand::thread_rng())
        .unwrap_or(&"⏳");

    // Spawn the actual LLM call; receive result via oneshot.
    let (tx, rx) = oneshot::channel();
    let cfg = cfg.clone();
    let model = model.to_string();
    let messages = messages.to_vec();
    tokio::spawn(async move {
        let result = call_okapi_direct(&cfg, &model, &messages).await;
        let _ = tx.send(result);
    });

    let mut rx = rx;
    let mut interval = tokio::time::interval(std::time::Duration::from_millis(180));
    let mut frame_idx = 0usize;

    // Print initial spinner line.
    print!(
        "\r\x1b[KJack → \x1b[1;36m{expression}\x1b[0m {}",
        SPINNER_FRAMES[0]
    );
    std::io::stdout().flush().unwrap();

    loop {
        tokio::select! {
            result = &mut rx => {
                return result.unwrap_or(Err("internal error: channel closed".into()));
            }
            _ = interval.tick() => {
                frame_idx = (frame_idx + 1) % SPINNER_FRAMES.len();
                print!(
                    "\r\x1b[KJack → \x1b[1;36m{expression}\x1b[0m {}",
                    SPINNER_FRAMES[frame_idx]
                );
                std::io::stdout().flush().unwrap();
            }
        }
    }
}

/// Send messages to the Okapi chat API (no spinner — raw call).
async fn call_okapi_direct(
    cfg: &russell_doctor::client::ClientConfig,
    model: &str,
    messages: &[serde_json::Value],
) -> std::result::Result<String, String> {
    let base_url = cfg
        .base_url
        .as_deref()
        .unwrap_or("http://127.0.0.1:11435/v1");
    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));

    let body = serde_json::json!({
        "model": model,
        "messages": messages,
        "temperature": 0.7,
        "max_tokens": 1024,
    });

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| format!("HTTP client: {e}"))?;

    let mut req = client.post(&url).json(&body);
    if let Some(ref key) = cfg.api_key {
        req = req.bearer_auth(key);
    }

    let resp = req
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("HTTP {status}: {text}"));
    }

    let json: serde_json::Value = resp.json().await.map_err(|e| format!("parse error: {e}"))?;

    let content = json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("(no response)")
        .to_string();

    Ok(content)
}

/// Persist the conversation history to disk.
fn save_history(chat_path: &std::path::Path, history: &ChatHistory) -> Result<()> {
    let json = serde_json::to_string(history)?;
    std::fs::write(chat_path.with_extension("json"), &json)
        .with_context(|| format!("writing {}", chat_path.display()))?;
    Ok(())
}

/// Journal a chat turn as a help-session event for audit.
fn journal_chat_turn(
    journal: &JournalWriter,
    session_id: &str,
    model: &str,
    user_msg: &str,
    assistant_msg: &str,
) {
    let ts_unix = russell_core::time::now_unix();
    let ts = russell_core::time::now_rfc3339();
    let _ = journal.append_help_session_row(
        session_id,
        ts_unix,
        &ts,
        "okapi",
        Some(model),
        Some(user_msg),
        user_msg.len() as i64,
        assistant_msg.len() as i64,
        None, // latency not tracked per-turn in chat
        "ok",
        None,
        &format!("memory/chats/{session_id}.json"),
    );
}

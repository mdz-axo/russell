// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell chat` — interactive conversation with Jack.
//!
//! Launches a readline REPL. Each turn composes a fresh SOAP
//! bundle with the current journal state, appends it to the
//! conversation history, and sends the full context to the LLM.
//! Jack responds in his chat persona, with full awareness of
//! the conversation history.
//!
//! The chat history is persisted to
//! `~/.local/state/harness/memory/chats/<session-id>.jsonl`.
//!
//! Type `/exit`, `/quit`, or Ctrl-D to end the session.

use anyhow::{Context, Result};
use russell_core::journal::{JournalReader, JournalWriter};
use russell_core::paths::Paths;
use russell_skills::Skill;
use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;
use std::io::Write;
use tracing::{info, warn};

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

/// An Ollama model entry from `/api/tags`.
#[derive(Debug, Clone, Deserialize)]
struct OllamaModel {
    name: String,
}

#[derive(Debug, Clone, Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaModel>,
}

/// Fetch the list of available models from Ollama's `/api/tags`.
async fn ollama_list_models(base_url: &str) -> Result<Vec<String>, String> {
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
        .map_err(|e| format!("Ollama unreachable: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("Ollama returned HTTP {}", resp.status()));
    }
    let body: OllamaTagsResponse = resp.json().await.map_err(|e| format!("parse error: {e}"))?;
    Ok(body.models.into_iter().map(|m| m.name).collect())
}

/// Find top fuzzy matches for `needle` in `models`.
///
/// Two-layer strategy:
/// 1. **Exact/substring match** — if one model contains the needle as a
///    case-insensitive substring and is the *only* such match, return it.
/// 2. **Jaro-Winkler fuzzy** — score all models; if one clear winner
///    (≥0.9 with >0.1 gap), return it; otherwise return all above 0.5
///    for disambiguation.
fn fuzzy_match_models<'a>(needle: &str, models: &'a [String]) -> Vec<&'a str> {
    let needle_lower = needle.to_lowercase();

    // --- Layer 1: case-insensitive substring match ---
    let substr_matches: Vec<&str> = models
        .iter()
        .map(|m| m.as_str())
        .filter(|m| m.to_lowercase().contains(&needle_lower))
        .collect();
    if substr_matches.len() == 1 {
        return vec![substr_matches[0]];
    }

    // --- Layer 2: Jaro-Winkler fuzzy scoring ---
    let mut scored: Vec<(&str, f64)> = models
        .iter()
        .map(|m| {
            let name_lower = m.to_lowercase();
            // Strip :cloud / :latest tags for a second (shorter) comparison.
            let short_lower = name_lower
                .trim_end_matches(":cloud")
                .trim_end_matches(":latest");
            let full = strsim::jaro_winkler(&needle_lower, &name_lower);
            let short = strsim::jaro_winkler(&needle_lower, short_lower);
            (m.as_str(), full.max(short))
        })
        .collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Perfect Jaro-Winkler (1.0) → auto-select regardless of gap.
    if let Some(first) = scored.first() {
        if first.1 >= 0.999 {
            return vec![first.0];
        }
    }

    // Clear winner: ≥0.9 and well ahead of #2.
    if let (Some(first), Some(second)) = (scored.first(), scored.get(1)) {
        if first.1 >= 0.9 && (first.1 - second.1) > 0.1 {
            return vec![first.0];
        }
    }

    // If substring already gave us matches, return those.
    if !substr_matches.is_empty() {
        return substr_matches;
    }

    // Otherwise return all fuzzy-scored above 0.5 (up to 10).
    scored
        .into_iter()
        .filter(|(_, s)| *s >= 0.5)
        .take(10)
        .map(|(name, _)| name)
        .collect()
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

    // Resolve the default model from env, and fetch available models from Ollama.
    let base_url = std::env::var("RUSSELL_DOCTOR_BASE_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:11434/v1".into());
    let mut current_model =
        std::env::var("RUSSELL_DOCTOR_MODEL").unwrap_or_else(|_| "deepseek-v4-pro:cloud".into());
    let ollama_models: Vec<String> = ollama_list_models(&base_url).await.unwrap_or_default();

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
                            println!("  /model list   — list available Ollama models");
                            println!("  /model <name> — switch to a model (fuzzy match)");
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
                                println!("  Available models ({}):", ollama_models.len());
                                for m in &ollama_models {
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
                                let matches = fuzzy_match_models(name, &ollama_models);
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

                // Call the LLM.
                print!("Jack → ");
                std::io::stdout().flush().unwrap();

                let cfg = russell_doctor::client::ClientConfig::from_env();
                let response = call_ollama(&cfg, &current_model, &messages).await;

                match response {
                    Ok(content) => {
                        println!("{content}\n");
                        history.turns.push(Turn {
                            role: "assistant".into(),
                            content: content.clone(),
                        });
                        save_history(&chat_path, &history)?;

                        // Also journal the chat turn as a help-session event.
                        journal_chat_turn(&journal, &session_id, &current_model, trimmed, &content);
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

/// Build the Objective section for the current turn.
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

/// Send messages to the Ollama chat API.
async fn call_ollama(
    cfg: &russell_doctor::client::ClientConfig,
    model: &str,
    messages: &[serde_json::Value],
) -> std::result::Result<String, String> {
    let base_url = cfg
        .base_url
        .as_deref()
        .unwrap_or("http://127.0.0.1:11434/v1");
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
        "ollama",
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

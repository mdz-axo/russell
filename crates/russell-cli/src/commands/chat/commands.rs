// SPDX-License-Identifier: MIT OR Apache-2.0
//! Slash-command handlers for the chat REPL.
//!
//! Each `/command` gets its own handler function, keeping the
//! main REPL loop thin.

use russell_skills::Skill;
use rustyline::DefaultEditor;

/// Prompt the operator to pick from a numbered list of models.
/// Returns `Some(zero-based-index)` on valid selection, `None` on cancel.
pub fn prompt_model_selection(
    editor: &mut DefaultEditor,
    history_entry: &str,
    count: usize,
) -> Option<usize> {
    println!("  Type a number to select, or 'cancel'.");
    let _ = editor.add_history_entry(history_entry);
    let sel_line = editor.readline("select → ").ok()?;
    let sel = sel_line.trim();
    if sel == "cancel" || sel == "/model cancel" {
        println!("  Cancelled.");
        return None;
    }
    match sel.trim_start_matches("/model ").trim().parse::<usize>() {
        Ok(idx) if idx >= 1 && idx <= count => Some(idx - 1),
        _ => {
            println!("  Invalid selection. Cancelled.");
            None
        }
    }
}

/// Fetch the list of available models from Okapi's `/api/tags`.
pub async fn okapi_list_models(base_url: &str) -> Result<Vec<String>, String> {
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
    let body: serde_json::Value = resp.json().await.map_err(|e| format!("parse error: {e}"))?;
    Ok(body["models"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| m["name"].as_str().map(str::to_string))
        .collect())
}

/// Handle `/help` command.
pub fn handle_help() {
    println!("  Commands:");
    println!("  /exit, /quit  — end the session");
    println!("  /refresh      — reload skills from disk");
    println!("  /reload       — same as /refresh");
    println!("  /history      — show conversation history summary");
    println!("  /skills       — list available skills");
    println!("  /model        — show current model");
    println!("  /model list   — list available Okapi models");
    println!("  /model <name> — switch to a model (fuzzy match)");
    println!("  /approve      — consent to Jack's proposed action");
    println!("                  (also: 'ok', 'yes', 'do it', 'go ahead')");
    println!("  /deny         — refuse Jack's proposed action");
    println!("                  (also: 'no', 'nope', 'cancel')");
    println!();
}

/// Handle `/history` command.
pub fn handle_history(session_id: &str, turns: &[super::history::Turn]) {
    println!("  Session {} — {} turns", session_id, turns.len());
    for (i, turn) in turns.iter().enumerate() {
        let label = if turn.role == "user" { "you" } else { "Jack" };
        let preview = if turn.content.len() > 80 {
            format!("{}…", &turn.content[..80])
        } else {
            turn.content.clone()
        };
        println!("  {i:>3}. {label}: {preview}");
    }
    println!();
}

/// Handle `/skills` command.
pub fn handle_skills(skills: &[Skill]) {
    if skills.is_empty() {
        println!("  No skills loaded.");
    } else {
        for s in skills {
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
}

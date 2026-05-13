// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell chat` — interactive REPL with Jack via Okapi.

use anyhow::{Context, Result};
use rand::seq::SliceRandom;
use russell_core::journal::{HelpSessionInput, HelpSessionStatus, JournalReader, JournalWriter};
use russell_core::paths::Paths;
use russell_doctor::action::{self, ResolvedAction};
use russell_doctor::client::LlmClient;
use russell_doctor::client::SoapPrompt;
use russell_doctor::oai_client::OkapiClient;
use russell_skills::Skill;
use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;
use std::io::Write;
use tokio::sync::oneshot;
use tracing::{info, warn};

/// A pending action (probe or intervention) awaiting operator consent.
/// Derived from [`ResolvedAction`] with UI-specific fields.
#[derive(Debug, Clone)]
struct PendingAction {
    action: ResolvedAction,
}

/// Returns true if the input looks like natural-language consent
/// ("ok", "yes", "do it", "go ahead", "sure", "yep", etc.).
fn is_affirmative(input: &str) -> bool {
    let lower = input.to_lowercase();
    let lower = lower.trim();
    matches!(
        lower,
        "ok" | "okay"
            | "yes"
            | "yep"
            | "yeah"
            | "yea"
            | "sure"
            | "do it"
            | "go ahead"
            | "go for it"
            | "approved"
            | "run it"
            | "execute"
            | "please"
            | "y"
            | "yes please"
            | "ok do it"
            | "lets go"
            | "let's go"
    )
}

/// Returns true if the input is a refusal.
fn is_refusal(input: &str) -> bool {
    let lower = input.trim();
    matches!(
        lower,
        "/deny"
            | "no"
            | "nope"
            | "cancel"
            | "nah"
            | "not now"
            | "later"
            | "hang on"
            | "wait"
            | "hold on"
    )
}

/// One turn in the conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Turn {
    role: String,
    content: String,
}

/// Full conversation history (excludes the system prompt which is
/// separate and fixed).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChatHistory {
    session_id: String,
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

    // Load skills at session start; can be reloaded with /reload.
    let mut skills = russell_skills::load_all(&paths.skills()).unwrap_or_default();
    let profile = russell_core::Profile::load(&paths.profile()).ok();

    let mut history = ChatHistory::new(session_id.clone());
    let mut editor = DefaultEditor::new().context("initialising readline")?;
    let mut pending_action: Option<PendingAction> = None;

    // Resolve the default model from env, and fetch available models from Okapi.
    let base_url = std::env::var("RUSSELL_DOCTOR_BASE_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:11435/v1".into());
    let mut current_model = std::env::var("RUSSELL_DOCTOR_MODEL").unwrap_or_default();
    let okapi_models: Vec<String> = okapi_list_models(&base_url).await.unwrap_or_default();

    // Auto-select a model if none configured.
    if current_model.is_empty() {
        if let Some(first) = okapi_models.first() {
            current_model = first.clone();
        } else {
            current_model = "default".into();
        }
    }

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
                    if trimmed == "/approve" || is_affirmative(trimmed) {
                        execute_pending_action(&journal, paths, pa, &session_id, &current_model)
                            .await;
                        pending_action = None;
                        continue;
                    }
                    if let Some(_pw) = trimmed.strip_prefix("/approve ") {
                        println!("  → Approving. Use `/approve` without a password next time —");
                        println!("    Jack will prompt you securely if sudo is needed.");
                        execute_pending_action(&journal, paths, pa, &session_id, &current_model)
                            .await;
                        pending_action = None;
                        continue;
                    }
                    if is_refusal(trimmed) {
                        println!("  Denied. Action not executed.");
                        pending_action = None;
                        continue;
                    }
                    // Any other input clears the pending action and
                    // falls through to normal chat processing.
                    println!("  → Action proposal cleared. Reply to continue the conversation.");
                    pending_action = None;
                }

                // Special commands.
                if trimmed.starts_with('/') {
                    match trimmed {
                        "/refresh" | "/reload" => {
                            let prev_count = skills.len();
                            match russell_skills::load_all(&paths.skills()) {
                                Ok(fresh) => {
                                    skills = fresh;
                                    let now = skills.len();
                                    if now > prev_count {
                                        println!(
                                            "  → Skills reloaded. Now have {} loaded (was {}).",
                                            now, prev_count
                                        );
                                    } else if now == prev_count {
                                        println!(
                                            "  → Skills reloaded ({} loaded, unchanged).",
                                            now
                                        );
                                    }
                                }
                                Err(e) => {
                                    println!("  → Failed to reload skills: {e}");
                                }
                            }
                            continue;
                        }
                        "/help" => {
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

                                // Hard-coded tag filters (Okapi/Ollama convention).
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
                        match action::resolve(&content, &skills) {
                            Some(Ok(action)) => {
                                if action.is_probe() {
                                    // Probes are read-only — auto-execute immediately.
                                    println!(
                                        "  → Running probe: {}/{}…",
                                        action.skill_id(),
                                        action.action_id()
                                    );
                                    let pa = PendingAction { action };
                                    execute_pending_action(
                                        &journal,
                                        paths,
                                        &pa,
                                        &session_id,
                                        &current_model,
                                    )
                                    .await;
                                } else {
                                    match &action {
                                        ResolvedAction::Intervention {
                                            risk, needs_sudo, ..
                                        } => {
                                            let sudo_tag =
                                                if *needs_sudo { " [needs sudo]" } else { "" };
                                            println!(
                                                "  → Jack proposes: {}/{} (risk: {:?}{}).",
                                                action.skill_id(),
                                                action.action_id(),
                                                risk,
                                                sudo_tag
                                            );
                                        }
                                        _ => unreachable!(),
                                    }
                                    println!("  → Say 'ok' to approve, or 'no' to refuse.");
                                    pending_action = Some(PendingAction { action });
                                }
                            }
                            Some(Err(e)) => {
                                println!("  → {e}");
                            }
                            None => { /* normal, no action proposed */ }
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

/// Execute a pending action (probe or intervention) via the skill dispatcher.
async fn execute_pending_action(
    journal: &JournalWriter,
    paths: &Paths,
    pending: &PendingAction,
    session_id: &str,
    model: &str,
) {
    use russell_skills::dispatch::{Dispatcher, DryRun, StepType};
    use std::io::Write;
    use std::time::Duration;

    let action = &pending.action;
    let skill_id = action.skill_id().to_string();
    let action_id = action.action_id().to_string();
    let skill_dir = paths.skills().join(&skill_id);
    let evidence_base = paths.evidence();

    let timeout = if action.is_probe() {
        Duration::from_secs(30)
    } else {
        Duration::from_secs(120)
    };

    let is_probe = action.is_probe();
    let (risk, needs_sudo, max_auto_risk, requires_human) = match action {
        ResolvedAction::Probe { max_auto_risk, .. } => {
            (russell_skills::RiskBand::None, false, *max_auto_risk, false)
        }
        ResolvedAction::Intervention {
            risk,
            needs_sudo,
            max_auto_risk,
            requires_human,
            ..
        } => (*risk, *needs_sudo, *max_auto_risk, *requires_human),
    };

    let mut dispatcher = Dispatcher::new(&skill_dir);
    dispatcher.intervention_timeout = timeout;
    dispatcher.probe_timeout = timeout;
    dispatcher.dry_run = DryRun::Disabled;
    dispatcher.max_auto_risk = max_auto_risk;

    // If this action requires explicit human confirmation, prompt again.
    if requires_human {
        print!(
            "  → This action is marked as requiring explicit human confirmation. Proceed? [y/N]: "
        );
        let _ = std::io::stdout().flush();
        let mut buf = String::new();
        if std::io::stdin().read_line(&mut buf).is_err() {
            println!("  → Could not read input. Aborting.");
            return;
        }
        let answer = buf.trim().to_lowercase();
        if answer != "y" && answer != "yes" {
            println!("  → Aborted by operator.");
            return;
        }
    }

    // Enforce risk cap — probes are always risk: none so this passes.
    if let Err(e) = dispatcher.check_risk(risk, false) {
        println!("  → Refused: {e}");
        return;
    }

    // Prompt for sudo password if needed (secure terminal prompt, not CLI).
    if needs_sudo {
        eprint!("  → Sudo password for this action: ");
        let _ = std::io::stderr().flush();
        let password = rpassword::read_password().unwrap_or_default();
        if password.is_empty() {
            println!("  → Empty password. Aborting action.");
            return;
        }
        // Verify password by trying: sudo -S true
        use tokio::io::AsyncWriteExt;
        let mut verify = match tokio::process::Command::new("sudo")
            .args(["-S", "--", "true"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            Ok(child) => child,
            Err(_) => {
                println!("  → Could not run sudo. Aborting action.");
                return;
            }
        };
        if let Some(ref mut stdin) = verify.stdin {
            let _ = stdin.write_all(format!("{password}\n").as_bytes()).await;
        }
        match verify.wait().await {
            Ok(s) if s.success() => {
                dispatcher.sudo_password = Some(password);
            }
            _ => {
                println!("  → Wrong sudo password. Aborting action.");
                return;
            }
        }
    }

    let step_type = if is_probe {
        StepType::Probe
    } else {
        StepType::Intervention
    };

    let result = dispatcher
        .run_and_journal(
            journal,
            &evidence_base,
            action.cmd(),
            &skill_id,
            &action_id,
            step_type,
            risk.as_str(),
            Some(timeout),
        )
        .await;

    match result {
        Ok(outcome) => {
            if outcome.exit_code == Some(0) {
                if is_probe {
                    if !outcome.stdout.is_empty() {
                        println!("  {}", outcome.stdout.trim());
                    }
                    println!("  → Probe {skill_id}/{action_id} complete.");
                } else {
                    println!("  → Executed {skill_id}/{action_id} successfully.");
                }
            } else {
                println!(
                    "  → {skill_id}/{action_id} exited with code {:?}.",
                    outcome.exit_code
                );
                if !outcome.stderr.is_empty() {
                    println!("  stderr: {}", outcome.stderr.trim());
                }
            }
            let action_label = if is_probe { "probe" } else { "approve" };
            journal_chat_turn(
                journal,
                session_id,
                model,
                &format!("/{action_label} {skill_id}/{action_id}"),
                &format!(
                    "executed: exit={:?}, stdout_len={}, stderr_len={}",
                    outcome.exit_code,
                    outcome.stdout.len(),
                    outcome.stderr.len(),
                ),
            );
        }
        Err(e) => {
            println!("  → Error running {skill_id}/{action_id}: {e}");
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
const THINKING_EXPRESSIONS: &[&str] = &[
    "🐶 digging digging digging",
    "✨ hey hey! hold your kibble",
    "🐶 *sniff* *sniff* checking things",
    "💅 working it. just a moment.",
    "🔍 just checking on you",
];

/// Call the LLM via Okapi with an animated thinking spinner on stdout.
/// Routes through the [`LlmClient`] port rather than raw HTTP.
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
        let result = call_llm_via_port(&cfg, &model, &messages).await;
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

/// Send chat messages through the [`LlmClient`] port.
///
/// Flattens the multi-message array into a [`SoapPrompt`] and calls
/// [`OkapiClient::chat`]. This replaces the old `call_okapi_direct`
/// which bypassed the hexagonal port with raw `reqwest` calls.
async fn call_llm_via_port(
    cfg: &russell_doctor::client::ClientConfig,
    model: &str,
    messages: &[serde_json::Value],
) -> std::result::Result<String, String> {
    let system = messages
        .iter()
        .find(|m| m["role"] == "system")
        .and_then(|m| m["content"].as_str())
        .unwrap_or("")
        .to_string();

    // Flatten conversation into a rendered Markdown block.
    let mut rendered = String::new();
    for msg in messages.iter().filter(|m| m["role"] != "system") {
        let role = msg["role"].as_str().unwrap_or("unknown");
        let content = msg["content"].as_str().unwrap_or("");
        let label = if role == "user" { "User" } else { "Jack" };
        rendered.push_str(&format!("**{label}:** {content}\n\n"));
    }

    let soap = SoapPrompt {
        system,
        subjective: String::new(),
        objective: String::new(),
        rendered: rendered.trim_end().to_string(),
    };

    let mut chat_cfg = cfg.clone();
    chat_cfg.model = model.to_string();
    // Ensure we always point at Okapi.
    if chat_cfg.base_url.is_none() {
        chat_cfg.base_url = Some("http://127.0.0.1:11435/v1".into());
    }
    if chat_cfg.api_key.is_none() {
        chat_cfg.api_key = Some("okapi".into());
    }

    let client = OkapiClient::new(&chat_cfg).map_err(|e| format!("client error: {e}"))?;

    let resp = client.chat(&soap).await.map_err(|e| format!("{e}"))?;

    Ok(resp.content)
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
    let evidence_ref = format!("memory/chats/{session_id}.json");
    let input = HelpSessionInput {
        id: session_id,
        ts_unix,
        ts: &ts,
        backend: "okapi",
        model: Some(model),
        note: Some(user_msg),
        prompt_chars: user_msg.len() as i64,
        response_chars: assistant_msg.len() as i64,
        latency_ms: None,
        status: HelpSessionStatus::Ok,
        error_kind: None,
        evidence_ref: &evidence_ref,
    };
    let _ = journal.append_help_session(&input);
}

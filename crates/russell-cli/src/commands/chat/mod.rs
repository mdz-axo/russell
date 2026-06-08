// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell chat` — interactive REPL with Jack via Okapi.
//!
//! This module is split into focused submodules:
//! - [`execute`] — local skill execution (probes & interventions)
//! - [`objective`] — SOAP objective builder

pub mod execute;
pub mod objective;

use anyhow::{Context, Result};
use russell_core::journal::{JournalReader, JournalWriter};
use russell_core::paths::Paths;
use russell_meta::action::{self, ResolvedAction};
use russell_meta::client::LlmClient;
use russell_skills::RiskBand;
use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::time::Instant;
use tracing::{debug, info, warn};

use execute::journal_chat_turn;

// ─── Chat history types (from history.rs) ───────────────────────────────────

/// One turn in the conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Turn {
    pub role: String,
    pub content: String,
}

/// Full conversation history (excludes the system prompt which is
/// separate and fixed).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatHistory {
    pub session_id: String,
    pub turns: Vec<Turn>,
}

impl ChatHistory {
    pub fn new(session_id: String) -> Self {
        Self {
            session_id,
            turns: Vec::new(),
        }
    }
}

/// Persist the conversation history to disk.
///
/// Failures are logged but do **not** terminate the session —
/// losing chat history is annoying, not fatal.
fn save_history(chat_path: &std::path::Path, history: &ChatHistory) {
    use std::os::unix::fs::OpenOptionsExt;
    let dest = chat_path.with_extension("json");
    let json = match serde_json::to_string(history) {
        Ok(j) => j,
        Err(e) => {
            warn!("serializing chat history: {e}");
            return;
        }
    };
    if let Err(e) = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o664)
        .open(&dest)
        .and_then(|mut f| f.write_all(json.as_bytes()))
    {
        warn!("writing {}: {e}", dest.display());
    }
}

// ─── Consent handling (from consent.rs) ─────────────────────────────────────

/// Pending actions expire after this many seconds without operator
/// Default consent expiry in seconds. Can be overridden via
/// `[harness] consent_ttl_secs` in `rules.d/*.toml`.
const DEFAULT_CONSENT_EXPIRY_SECS: u64 = 300;

/// A pending action (probe or intervention) awaiting operator consent.
#[derive(Debug, Clone)]
pub struct PendingAction {
    action: ResolvedAction,
    stdin_content: Option<String>,
    proposed_at: Instant,
}

impl PendingAction {
    fn new(action: ResolvedAction, stdin_content: Option<String>) -> Self {
        Self {
            action,
            stdin_content,
            proposed_at: Instant::now(),
        }
    }

    fn is_expired(&self, ttl_secs: u64) -> bool {
        self.proposed_at.elapsed().as_secs() >= ttl_secs
    }

    fn describe(&self) -> String {
        format!("{}/{}", self.action.skill_id(), self.action.action_id())
    }
}

/// Returns true if the input looks like natural-language consent.
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
    matches!(
        input.trim(),
        "/deny" | "no" | "nope" | "cancel" | "nah" | "not now" | "later"
    )
}

/// Check if the most recent history turn is an action result that Jack
/// should interpret — probe result, intervention result, kask tool result,
/// or action parse error.
fn is_action_result_in_history(history: &ChatHistory) -> bool {
    history
        .turns
        .last()
        .map(|t| {
            t.role == "user"
                && (t.content.starts_with("[probe result:")
                    || t.content.starts_with("[probe error:")
                    || t.content.starts_with("[intervention result:")
                    || t.content.starts_with("[kask tool result:")
                    || t.content.starts_with("[action error:"))
        })
        .unwrap_or(false)
}

// ─── Slash command handlers (from commands.rs) ──────────────────────────────

/// Prompt the operator to pick from a numbered list of models.
fn prompt_model_selection(
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
    let body: serde_json::Value = resp.json().await.map_err(|e| format!("parse error: {e}"))?;
    Ok(body["models"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| m["name"].as_str().map(str::to_string))
        .collect())
}

/// Handle `/help` command.
fn handle_help() {
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
fn handle_history(session_id: &str, turns: &[Turn]) {
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
fn handle_skills(skills: &[russell_skills::Skill]) {
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

// ─── LLM spinner (from spinner.rs) ──────────────────────────────────────────

/// Braille spinner frames for the thinking animation.
const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Jack's thinking expressions.
const THINKING_EXPRESSIONS: &[&str] = &[
    "🐶 digging digging digging",
    "✨ hey hey! hold your kibble",
    "🐶 *sniff* *sniff* checking things",
    "💅 working it. just a moment.",
    "🔍 just checking on you",
];

/// Call the LLM via Okapi with an animated thinking spinner.
async fn call_okapi_with_spinner(
    cfg: &russell_meta::client::ClientConfig,
    model: &str,
    messages: &[serde_json::Value],
) -> std::result::Result<String, String> {
    use rand::seq::SliceRandom;
    use tokio::sync::oneshot;

    let expression = THINKING_EXPRESSIONS
        .choose(&mut rand::thread_rng())
        .unwrap_or(&"⏳");

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

/// Send chat messages through the LlmClient port.
async fn call_llm_via_port(
    cfg: &russell_meta::client::ClientConfig,
    model: &str,
    messages: &[serde_json::Value],
) -> std::result::Result<String, String> {
    use russell_meta::client::SoapPrompt;
    use russell_meta::oai_client::OkapiClient;

    let system = messages
        .iter()
        .find(|m| m["role"] == "system")
        .and_then(|m| m["content"].as_str())
        .unwrap_or("")
        .to_string();

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
        temperature: None,
        max_tokens: None,
    };

    let mut chat_cfg = cfg.clone();
    chat_cfg.model = model.to_string();
    if chat_cfg.base_url.is_none() {
        chat_cfg.base_url = Some(russell_meta::health::DEFAULT_BASE_URL.to_string());
    }
    if chat_cfg.api_key.is_none() {
        chat_cfg.api_key = Some("okapi".into());
    }

    let base = chat_cfg
        .base_url
        .as_deref()
        .unwrap_or(russell_meta::health::DEFAULT_BASE_URL);
    if !russell_meta::health::ensure_ready(base).await {
        return Err("can't reach Okapi (tried auto-start)".into());
    }

    let client = OkapiClient::new(&chat_cfg)
        .await
        .map_err(|e| format!("client error: {e}"))?;

    let resp = client.chat(&soap).await.map_err(|e| format!("{e}"))?;

    Ok(resp.content)
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
    let load_result = russell_skills::load_all(&paths.skills()).unwrap_or_else(|e| {
        warn!(error = %e, "failed to read skills directory");
        russell_skills::LoadResult {
            skills: Vec::new(),
            skipped: Vec::new(),
        }
    });
    if load_result.has_skipped() {
        eprintln!("{}", load_result.skipped_summary());
    }
    let mut skills = load_result.skills;
    let mut skipped = load_result.skipped;
    let profile = russell_core::Profile::load(&paths.profile()).ok();

    let mut history = ChatHistory::new(session_id.clone());
    let mut editor = DefaultEditor::new().context("initialising readline")?;
    let mut pending_action: Option<PendingAction> = None;

    // Load skill registry cache for telemetry display and lifecycle management.
    let registry_path = paths.state.join("registry").join("local-cache.yaml");
    let mut registry =
        russell_skills::registry::RegistryCache::load(&registry_path).unwrap_or_default();

    // Reconcile registry against disk (fix stale/orphan entries).
    if registry.reconcile(&skills)
        && let Err(e) = registry.save(&registry_path)
    {
        tracing::warn!(error = %e, "registry reconcile save failed");
    }

    // Load model config from the shared ClientConfig.
    let client_cfg = russell_meta::client::ClientConfig::from_env();
    let base_url = client_cfg
        .base_url
        .clone()
        .unwrap_or_else(|| "http://127.0.0.1:11435/v1".into());

    // Resolve the configured model name against Okapi's actual model list.
    let resolved =
        russell_meta::oai_client::resolve_and_correct_model(&client_cfg, &paths.config).await;
    if resolved != client_cfg.model {
        println!(
            "  Corrected: model \"{}\" → \"{}\" (env file updated)",
            client_cfg.model, resolved
        );
    }
    let mut current_model = resolved;

    // Okapi's model list is fetched lazily — only when the operator
    // uses `/model list` or `/model <name>`. Not at startup.
    let mut okapi_models: Vec<String> = Vec::new();
    let mut okapi_models_fetched = false;

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
                    // T5: Expiry guard — stale pending actions are
                    // discarded to prevent delayed approvals.
                    if pa.is_expired(DEFAULT_CONSENT_EXPIRY_SECS) {
                        println!(
                            "  → Pending action expired ({}s). Please re-propose.",
                            DEFAULT_CONSENT_EXPIRY_SECS
                        );
                        pending_action = None;
                        continue;
                    }
                    if trimmed == "/approve" || is_affirmative(trimmed) {
                        // T5: Echo confirmation of what will execute.
                        println!("  → Executing: {}", pa.describe());
                        let is_skill_mgr_mutation = pa.action.skill_id() == "skill-manager"
                            && matches!(
                                pa.action.action_id(),
                                "install" | "build" | "create-manifest" | "delete"
                            );
                        let action_result = execute_action(
                            &journal,
                            &pa.action,
                            &session_id,
                            &current_model,
                            paths,
                        )
                        .await;
                        if let Some(result_text) = action_result {
                            history.turns.push(Turn {
                                role: "user".into(),
                                content: result_text,
                            });
                            save_history(&chat_path, &history);
                        }
                        if is_skill_mgr_mutation {
                            match russell_skills::load_all(&paths.skills()) {
                                Ok(result) => {
                                    skills = result.skills;
                                    skipped = result.skipped;
                                    match russell_skills::registry::RegistryCache::load(
                                        &registry_path,
                                    ) {
                                        Ok(fresh_reg) => registry = fresh_reg,
                                        Err(e) => warn!(error = %e, "registry reload failed"),
                                    }
                                }
                                Err(e) => {
                                    warn!(error = %e, "skills reload after skill-manager intervention failed")
                                }
                            }
                        }
                        pending_action = None;

                        // Gap A: After executing an approved intervention, Jack
                        // should interpret the result — not just print it and
                        // wait for the operator to ask "what happened?"
                        if is_action_result_in_history(&history) {
                            history.turns.push(Turn {
                                role: "user".into(),
                                content: "(Continue — interpret the result above and respond.)"
                                    .to_string(),
                            });
                            save_history(&chat_path, &history);
                            for _ in 0..5 {
                                call_jack(
                                    &mut history,
                                    &chat_path,
                                    &journal,
                                    &session_id,
                                    &current_model,
                                    &reader,
                                    &skills,
                                    &skipped,
                                    profile.as_ref(),
                                    &registry,
                                    paths,
                                    &mut pending_action,
                                    &client_cfg,
                                )
                                .await?;
                                if let Ok(fresh) =
                                    russell_skills::registry::RegistryCache::load(&registry_path)
                                {
                                    registry = fresh;
                                }
                                if !is_action_result_in_history(&history) {
                                    break;
                                }
                                history.turns.push(Turn {
                                    role: "user".into(),
                                    content: "(Continue — interpret the result above and respond.)"
                                        .to_string(),
                                });
                                save_history(&chat_path, &history);
                            }
                        }
                        continue;
                    }
                    if let Some(_pw) = trimmed.strip_prefix("/approve ") {
                        println!("  → Approving. Use `/approve` without a password next time —");
                        println!("    Jack will prompt you securely if sudo is needed.");
                        let is_skill_mgr_mutation = pa.action.skill_id() == "skill-manager"
                            && matches!(
                                pa.action.action_id(),
                                "install" | "build" | "create-manifest" | "delete"
                            );
                        let action_result = execute_action(
                            &journal,
                            &pa.action,
                            &session_id,
                            &current_model,
                            paths,
                        )
                        .await;
                        if let Some(result_text) = action_result {
                            history.turns.push(Turn {
                                role: "user".into(),
                                content: result_text,
                            });
                            save_history(&chat_path, &history);
                        }
                        if is_skill_mgr_mutation {
                            match russell_skills::load_all(&paths.skills()) {
                                Ok(result) => {
                                    skills = result.skills;
                                    skipped = result.skipped;
                                    match russell_skills::registry::RegistryCache::load(
                                        &registry_path,
                                    ) {
                                        Ok(fresh_reg) => registry = fresh_reg,
                                        Err(e) => warn!(error = %e, "registry reload failed"),
                                    }
                                }
                                Err(e) => {
                                    warn!(error = %e, "skills reload after skill-manager intervention failed")
                                }
                            }
                        }
                        pending_action = None;

                        // Gap A (password variant): Same auto-interpretation loop
                        // as the regular /approve path above.
                        if is_action_result_in_history(&history) {
                            history.turns.push(Turn {
                                role: "user".into(),
                                content: "(Continue — interpret the result above and respond.)"
                                    .to_string(),
                            });
                            save_history(&chat_path, &history);
                            for _ in 0..5 {
                                call_jack(
                                    &mut history,
                                    &chat_path,
                                    &journal,
                                    &session_id,
                                    &current_model,
                                    &reader,
                                    &skills,
                                    &skipped,
                                    profile.as_ref(),
                                    &registry,
                                    paths,
                                    &mut pending_action,
                                    &client_cfg,
                                )
                                .await?;
                                if let Ok(fresh) =
                                    russell_skills::registry::RegistryCache::load(&registry_path)
                                {
                                    registry = fresh;
                                }
                                if !is_action_result_in_history(&history) {
                                    break;
                                }
                                history.turns.push(Turn {
                                    role: "user".into(),
                                    content: "(Continue — interpret the result above and respond.)"
                                        .to_string(),
                                });
                                save_history(&chat_path, &history);
                            }
                        }
                        continue;
                    }
                    if is_refusal(trimmed) {
                        let denied_action = match &pending_action {
                            Some(pa) => {
                                format!("{}/{}", pa.action.skill_id(), pa.action.action_id())
                            }
                            None => "unknown".to_string(),
                        };
                        println!("  Denied.");
                        pending_action = None;

                        // Gap 1: Cybernetic feedback closure — denial triggers
                        // LLM re-orientation so Jack can propose alternatives
                        // rather than the loop terminating with a broken feedback path.
                        history.turns.push(Turn {
                            role: "user".into(),
                            content: format!(
                                "The operator denied ACTION: {denied_action}. What else do you recommend?"
                            ),
                        });
                        save_history(&chat_path, &history);

                        // Rerun Jack with the denial context to get a new recommendation.
                        match call_jack(
                            &mut history,
                            &chat_path,
                            &journal,
                            &session_id,
                            &current_model,
                            &reader,
                            &skills,
                            &skipped,
                            profile.as_ref(),
                            &registry,
                            paths,
                            &mut pending_action,
                            &client_cfg,
                        )
                        .await
                        {
                            Ok(()) => {}
                            Err(_) => {
                                println!("  → Jack couldn't suggest an alternative right now.");
                            }
                        }
                        continue;
                    }
                    // Any other input clears the pending action.
                    println!("  → Action proposal cleared. Reply to continue the conversation.");
                    pending_action = None;
                }

                // Special commands.
                if trimmed.starts_with('/')
                    && handle_slash_command(
                        trimmed,
                        &mut skills,
                        &mut skipped,
                        &mut editor,
                        &mut current_model,
                        &mut okapi_models,
                        &mut okapi_models_fetched,
                        &base_url,
                        &session_id,
                        &history,
                        paths,
                    )
                    .await
                {
                    continue;
                }

                // Add user message to history.
                history.turns.push(Turn {
                    role: "user".into(),
                    content: trimmed.to_string(),
                });

                // Call Jack with the user's message as input.
                // After each call, check if a probe result was injected into
                // history (meaning a probe auto-executed). If so, re-call Jack
                // so he can interpret the result — closing the cybernetic loop
                // without requiring the operator to ask "what did you learn?"
                //
                // Safety: cap the loop at 5 iterations to prevent runaway
                // action-result chains from exhausting the token budget.
                let mut action_loop_count: u32 = 0;
                loop {
                    call_jack(
                        &mut history,
                        &chat_path,
                        &journal,
                        &session_id,
                        &current_model,
                        &reader,
                        &skills,
                        &skipped,
                        profile.as_ref(),
                        &registry,
                        paths,
                        &mut pending_action,
                        &client_cfg,
                    )
                    .await?;

                    // Reload skill registry to capture fresh telemetry (post-Jack execution).
                    if let Ok(fresh) = russell_skills::registry::RegistryCache::load(&registry_path)
                    {
                        registry = fresh;
                    }

                    // If the most recent turn is a user message containing an
                    // action result, Jack needs another turn to interpret it.
                    // Break when the last turn is an assistant message (Jack
                    // already responded) or a plain user message (the operator
                    // spoke — main loop will handle it).
                    if !is_action_result_in_history(&history) {
                        break;
                    }

                    action_loop_count += 1;
                    if action_loop_count >= 5 {
                        println!(
                            "  → Action chain limit reached (5). Stopping to preserve context budget."
                        );
                        break;
                    }

                    // Inject a prompt so Jack knows this is an auto-continuation,
                    // not a new operator message. This gives him context that he
                    // should interpret the result or error that just appeared.
                    history.turns.push(Turn {
                        role: "user".into(),
                        content: "(Continue — interpret the result or error above and respond.)"
                            .to_string(),
                    });
                    save_history(&chat_path, &history);
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

    // Safety flush: re-save registry on exit (dispatch already saves per-action).
    let _ = russell_skills::registry::RegistryCache::with_update(
        &registry_path,
        |_cache| { /* no-op — load+save for durability */ },
    );

    Ok(())
}

/// Unified action executor — dispatches to local skill execution.
async fn execute_action(
    journal: &JournalWriter,
    action: &ResolvedAction,
    session_id: &str,
    model: &str,
    paths: &Paths,
) -> Option<String> {
    let pa = PendingAction::new(action.clone(), None);
    execute::execute_pending_action(journal, paths, &pa, session_id, model).await
}

/// Handle a resolved ACTION: proposal from the LLM response.
#[allow(clippy::too_many_arguments)]
async fn handle_action_proposal(
    action: ResolvedAction,
    journal: &JournalWriter,
    session_id: &str,
    current_model: &str,
    paths: &Paths,
    history: &mut ChatHistory,
    chat_path: &std::path::Path,
    pending_action: &mut Option<PendingAction>,
    stdin_content: Option<String>,
) -> Result<()> {
    if action.is_probe() {
        // Probes are read-only — auto-execute immediately.
        println!(
            "  → Running probe: {}/{}…",
            action.skill_id(),
            action.action_id()
        );
        let pa = PendingAction::new(action, None);
        let probe_result =
            execute::execute_pending_action(journal, paths, &pa, session_id, current_model).await;
        if let Some(result_text) = probe_result {
            history.turns.push(Turn {
                role: "user".into(),
                content: result_text,
            });
            save_history(chat_path, history);
        }
    } else if action.is_shell_command() {
        // ADR-0050: Shell command proposed by Jack.
        // Always requires consent — even read-only commands show the
        // operator exactly what will run before it runs.
        if let ResolvedAction::ShellCommand {
            command,
            risk,
            needs_sudo,
            ..
        } = &action
        {
            let sudo_tag = if *needs_sudo { " [needs sudo]" } else { "" };
            println!(
                "  → Jack proposes shell: {} (risk: {:?}{}).",
                command, risk, sudo_tag
            );
            println!("  → Say 'ok' to approve, or 'no' to refuse.");
            *pending_action = Some(PendingAction::new(action, stdin_content));
        }
    } else {
        match &action {
            ResolvedAction::Intervention {
                risk, needs_sudo, ..
            } => {
                let sudo_tag = if *needs_sudo { " [needs sudo]" } else { "" };
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
        *pending_action = Some(PendingAction::new(action, stdin_content));
    }
    Ok(())
}

/// Handle slash commands. Returns `true` if the command was handled
/// (caller should `continue` the REPL loop), `false` if not recognized.
/// (caller should `continue` the REPL loop), `false` if not recognized.
#[allow(clippy::too_many_arguments)]
async fn handle_slash_command(
    trimmed: &str,
    skills: &mut Vec<russell_skills::Skill>,
    skipped: &mut Vec<russell_skills::SkippedSkill>,
    editor: &mut DefaultEditor,
    current_model: &mut String,
    okapi_models: &mut Vec<String>,
    okapi_models_fetched: &mut bool,
    base_url: &str,
    session_id: &str,
    history: &ChatHistory,
    paths: &Paths,
) -> bool {
    match trimmed {
        "/refresh" | "/reload" => {
            let prev_count = skills.len();
            match russell_skills::load_all(&paths.skills()) {
                Ok(result) => {
                    let has_skipped = result.has_skipped();
                    let skipped_count = result.skipped.len();
                    *skipped = result.skipped;
                    *skills = result.skills;
                    let now = skills.len();
                    if has_skipped {
                        println!(
                            "  → Skills reloaded. {} loaded, {} skipped:",
                            now, skipped_count
                        );
                        for s in skipped {
                            println!("    - {}: {}", s.id, s.reason);
                        }
                    } else if now > prev_count {
                        println!(
                            "  → Skills reloaded. Now have {} loaded (was {}).",
                            now, prev_count
                        );
                    } else if now == prev_count {
                        println!("  → Skills reloaded ({} loaded, unchanged).", now);
                    }
                }
                Err(e) => {
                    println!("  → Failed to reload skills: {e}");
                }
            }
            true
        }
        "/help" => {
            handle_help();
            true
        }
        "/history" => {
            handle_history(session_id, &history.turns);
            true
        }
        "/skills" => {
            handle_skills(skills);
            true
        }
        other => {
            if other == "/model" {
                println!("  Current model: {current_model}");
                println!();
                return true;
            }
            if other == "/model list" {
                // Lazy-fetch Okapi models on first use.
                if !*okapi_models_fetched {
                    *okapi_models = okapi_list_models(base_url).await.unwrap_or_default();
                    *okapi_models_fetched = true;
                }
                println!("  Available models ({}):", okapi_models.len());
                for m in okapi_models.iter() {
                    let marker = if m == current_model {
                        " ← current"
                    } else {
                        ""
                    };
                    println!("    {m}{marker}");
                }
                println!();
                return true;
            }
            if let Some(name) = other.strip_prefix("/model ") {
                let name = name.trim();
                if name.is_empty() {
                    println!("  Current model: {current_model}");
                    println!();
                    return true;
                }
                // Lazy-fetch Okapi models for switching.
                if !*okapi_models_fetched {
                    *okapi_models = okapi_list_models(base_url).await.unwrap_or_default();
                    *okapi_models_fetched = true;
                }
                // If Okapi is unreachable, trust the operator's input directly.
                if okapi_models.is_empty() {
                    *current_model = name.to_string();
                    println!(
                        "  Switched to model: {current_model} (unverified — Okapi unreachable)"
                    );
                    println!();
                    return true;
                }
                // Tag filters: `/model cloud` or `/model local`.
                if name == "cloud" || name == "local" {
                    let filtered: Vec<&String> = okapi_models
                        .iter()
                        .filter(|m| {
                            if name == "cloud" {
                                m.ends_with("cloud")
                            } else {
                                !m.ends_with("cloud")
                            }
                        })
                        .collect();
                    if filtered.is_empty() {
                        println!("  No {name} models found.");
                    } else {
                        println!("  {name} models ({}):", filtered.len());
                        for (i, m) in filtered.iter().enumerate() {
                            let marker = if *m == current_model {
                                " ← current"
                            } else {
                                ""
                            };
                            println!("    {}. {m}{marker}", i + 1);
                        }
                        if let Some(selected) =
                            prompt_model_selection(editor, trimmed, filtered.len())
                        {
                            *current_model = filtered[selected].clone();
                            println!("  Switched to model: {current_model}");
                        }
                    }
                    println!();
                    return true;
                }
                // Resolve the model name using the shared validator.
                let http = match reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(5))
                    .build()
                {
                    Ok(c) => c,
                    Err(e) => {
                        println!("  Can't resolve model: {e}");
                        println!();
                        return true;
                    }
                };
                // We need to block on the async resolve — use a nested block.
                let resolved =
                    russell_meta::oai_client::resolve_model_name(base_url, name, &http).await;
                if resolved == name {
                    println!("  No model found matching \"{name}\". Try /model list.");
                } else {
                    *current_model = resolved;
                    println!("  Switched to model: {current_model}");
                }
                println!();
                return true;
            }
            println!("  Unknown command: {other}. Try /help.");
            true
        }
    }
}

/// Call Jack (the LLM) with the current conversation state, handle
/// any ACTION: proposals in the response, and update history.
///
/// This is the shared LLM invocation path used by both the main chat
/// loop (new user input) and the denial re-orientation loop (Gap 1:
/// cybernetic feedback closure — when the operator denies an ACTION,
/// Jack is re-prompted with the denial to propose an alternative).
/// This is the shared LLM invocation path used by both the main chat
/// loop (new user input) and the denial re-orientation loop (Gap 1:
/// cybernetic feedback closure — when the operator denies an ACTION,
/// Jack is re-prompted with the denial to propose an alternative).
/// loop (new user input) and the denial re-orientation loop (Gap 1:
/// cybernetic feedback closure — when the operator denies an ACTION,
/// Jack is re-prompted with the denial to propose an alternative).
/// cybernetic feedback closure — when the operator denies an ACTION,
/// Jack is re-prompted with the denial to propose an alternative).
/// Jack is re-prompted with the denial to propose an alternative).
#[allow(clippy::too_many_arguments)]
async fn call_jack(
    history: &mut ChatHistory,
    chat_path: &std::path::Path,
    journal: &JournalWriter,
    session_id: &str,
    current_model: &str,
    reader: &JournalReader,
    skills: &[russell_skills::Skill],
    skipped: &[russell_skills::SkippedSkill],
    profile: Option<&russell_core::Profile>,
    registry: &russell_skills::registry::RegistryCache,
    paths: &Paths,
    pending_action: &mut Option<PendingAction>,
    _client_cfg: &russell_meta::client::ClientConfig,
) -> Result<()> {
    // Build the fresh SOAP objective.
    let objective = objective::build_objective(reader, skills, profile, registry);

    // Build system prompt: persona + relevance-scored KNOWLEDGE.md injection.
    // All applicable skill knowledge is injected (within token budget),
    // giving Jack full domain expertise in chat mode.
    let mut system = russell_meta::JACK_CHAT_PERSONA.to_string();

    // Inject skill load failures into Jack's context so he can act on them.
    // JR-2: observe > recommend > act. A broken skill is a signal, not noise.
    if !skipped.is_empty() {
        system.push_str("\n\n## Skill load failures\n\n");
        system.push_str("The following skills failed to load. Investigate why, report to the ");
        system.push_str("operator, and suggest fixes:\n\n");
        for s in skipped {
            system.push_str(&format!("- **{}**: {}\n", s.id, s.reason));
        }
        system.push_str("\nUse `russell skill-list` to review loaded and skipped skills. ");
        system.push_str("Common causes: unknown symptom names in manifest.yaml, missing `cmd` ");
        system.push_str(
            "in evaluation entries, or rollback_id referencing a non-existent intervention.\n",
        );
    }
    {
        use russell_meta::prompt_registry::{
            KnowledgeSlot, score_knowledge_relevance, select_knowledge,
        };

        // Derive active symptoms from recent events (same as compose_templated).
        let recent_events = reader.recent(20).unwrap_or_default();
        let active_symptoms: Vec<String> = recent_events
            .iter()
            .filter(|r| {
                let s = r.severity.as_str();
                s == "warn" || s == "alert" || s == "crit"
            })
            .filter_map(|r| r.module.as_ref())
            .flat_map(|m| {
                let mut syms = vec![];
                if let Some(probe) = m.strip_prefix("sentinel/threshold/") {
                    syms.push(probe.to_string());
                    for kw in probe.split('_') {
                        if kw.len() >= 3 {
                            syms.push(kw.to_string());
                        }
                    }
                } else if let Some(probe) = m.strip_prefix("sentinel/rate/") {
                    syms.push(probe.to_string());
                }
                syms
            })
            .collect();

        const KNOWLEDGE_BUDGET_TOKENS: usize = 3000;
        let mut slots: Vec<KnowledgeSlot> = Vec::new();
        for skill in skills {
            let applies = skill.applies_when.iter().any(|clause| {
                matches!(clause, russell_skills::AppliesWhen::Scalar { os_family: Some(os), .. } if os == "linux")
            });
            if !applies && !skill.applies_when.is_empty() {
                continue;
            }
            let knowledge_path = paths.skills().join(&skill.id).join("KNOWLEDGE.md");
            if let Ok(content) = std::fs::read_to_string(&knowledge_path) {
                if content.trim().is_empty() {
                    continue;
                }
                let skill_symptom_names: Vec<String> = skill
                    .symptoms
                    .iter()
                    .map(|s| s.name().to_string())
                    .collect();
                let relevance = score_knowledge_relevance(&skill_symptom_names, &active_symptoms);
                let token_estimate = content.len() / 4;
                slots.push(KnowledgeSlot {
                    skill_id: skill.id.clone(),
                    content,
                    relevance,
                    token_estimate,
                });
            }
        }
        let selected = select_knowledge(&slots, KNOWLEDGE_BUDGET_TOKENS);
        for slot in selected {
            system.push_str("\n\n---\n\n# Knowledge: ");
            system.push_str(&slot.skill_id);
            system.push_str("\n\n");
            system.push_str(&slot.content);
        }
    }

    // Build the messages array for the LLM.
    let mut messages: Vec<serde_json::Value> = Vec::new();
    messages.push(serde_json::json!({
        "role": "system",
        "content": system,
    }));

    // Insert the current journal state as a "user" message.
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

    // Gap 6: Enforce minimum reasoning budget in chat history.
    // If total prompt tokens approach the model's context window,
    // the history is summarized to preserve reasoning headroom.
    let total_prompt_tokens = estimate_message_tokens(&messages);
    const MIN_REASONING_BUDGET_TOKENS: usize = 2048;
    const CONTEXT_WINDOW_TOKENS: usize = 32768;
    let max_history_tokens = CONTEXT_WINDOW_TOKENS
        .saturating_sub(MIN_REASONING_BUDGET_TOKENS)
        .saturating_sub(system.len() / 4 + objective.len() / 4 + 200); // system + obj overhead

    if total_prompt_tokens > max_history_tokens {
        // Attenuate conversation history to preserve reasoning budget.
        // Remove oldest turns (keeping the system/objective messages first).
        let history_start = 2; // system + objective messages
        while estimate_message_tokens(&messages) > max_history_tokens
            && messages.len() > history_start + 2
        {
            messages.remove(history_start);
            if messages.len() > history_start + 1 {
                messages.remove(history_start);
            }
        }
        tracing::debug!(
            total = total_prompt_tokens,
            max_allowed = max_history_tokens,
            "chat history truncated to preserve reasoning budget"
        );
    }

    // Persist history before calling LLM.
    save_history(chat_path, history);

    // Call the LLM with an animated thinking spinner.
    let cfg = russell_meta::client::ClientConfig::from_env();
    let response = call_okapi_with_spinner(&cfg, current_model, &messages).await;

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
            save_history(chat_path, history);

            // Journal the chat turn as a help-session event.
            journal_chat_turn(journal, session_id, current_model, "(chat turn)", &content);

            // Check for ACTION: proposal.
            match action::resolve(&content, skills) {
                Some(Ok(mut action)) => {
                    // Append inline CLI arguments from the LLM response to
                    // the subprocess cmd (e.g. `Arguments --name swap-watcher`
                    // → appends `--name swap-watcher` to the manifest's cmd).
                    let inline_args = extract_inline_args(&content);
                    if !inline_args.is_empty() {
                        action.append_cmd_args(&inline_args);
                    }
                    let manifest = extract_manifest_block(&content);
                    handle_action_proposal(
                        action,
                        journal,
                        session_id,
                        current_model,
                        paths,
                        history,
                        chat_path,
                        pending_action,
                        manifest,
                    )
                    .await?;
                }
                Some(Err(e)) => {
                    // The action Jack proposed failed to parse (unknown skill,
                    // unknown action, malformed syntax, etc.). Push the
                    // error into history so Jack can see it in his next turn
                    // and interpret it for the operator.
                    let error_msg = format!("[action error: {e}]");
                    println!("  → {error_msg}");
                    history.turns.push(Turn {
                        role: "user".into(),
                        content: error_msg,
                    });
                    save_history(chat_path, history);
                    // The main REPL loop's auto-interpret mechanism will
                    // detect the [action error:] prefix in history and
                    // re-call Jack to interpret it.
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
            save_history(chat_path, history);
        }
    }
    Ok(())
}

/// Estimate the token count of a JSON messages array.
/// Rough heuristic: 1 token ≈ 4 characters of content.
fn estimate_message_tokens(messages: &[serde_json::Value]) -> usize {
    let mut total = 0;
    for msg in messages {
        if let Some(content) = msg.get("content").and_then(|c| c.as_str()) {
            total += content.len() / 4;
        }
    }
    total
}

/// Extract a `---manifest` … `---` block from an LLM response.
///
/// Jack may include manifest YAML content in his response using a
/// fenced block:
///
/// ```text
/// ACTION: skill-manager/create-manifest
/// ---manifest
/// id: my-skill
/// ...
/// ---
/// ```
/// id: my-skill
/// ...
/// ---
/// ```
/// ...
/// ---
/// ```
/// ---
/// ```
/// ```
fn extract_manifest_block(response: &str) -> Option<String> {
    let start_marker = "---manifest\n";
    let start = response.find(start_marker)?;
    let content_start = start + start_marker.len();
    let remainder = &response[content_start..];
    let end = if let Some(pos) = remainder.find("\n---\n") {
        pos + 1
    } else if remainder.starts_with("---\n") {
        0
    } else if remainder == "---" {
        remainder.len()
    } else if remainder.ends_with("\n---") {
        remainder.len() - 3
    } else {
        return None;
    };
    let content = remainder[..end].trim().to_string();
    if content.is_empty() {
        None
    } else {
        Some(content)
    }
}

/// Extract inline CLI arguments from an `Arguments` line in the LLM response.
///
/// Parses lines like `Arguments --name swap-watcher` or
/// `Arguments --name swap-watcher --flag value` into a Vec of individual
/// argument tokens. Handles quoted values.
///
/// Searches the entire response for the first `Arguments` line.
fn extract_inline_args(response: &str) -> Vec<String> {
    let args_line = response
        .lines()
        .find(|l| l.trim().starts_with("Arguments"))
        .map(|l| {
            let trimmed = l.trim();
            let after_prefix = trimmed.strip_prefix("Arguments").unwrap_or(trimmed);
            // Strip an optional colon separator ("Arguments: foo" → "foo").
            let after_colon = after_prefix.strip_prefix(':').unwrap_or(after_prefix);
            after_colon.trim().to_string()
        });

    let line = match args_line {
        Some(ref l) if !l.is_empty() => l,
        _ => return Vec::new(),
    };

    let mut args = Vec::new();
    let chars = line.chars().peekable();
    let mut current = String::new();
    let mut in_quote = false;

    for ch in chars {
        if ch == '"' || ch == '\'' {
            in_quote = !in_quote;
        } else if ch == ' ' && !in_quote {
            if !current.is_empty() {
                args.push(current.clone());
                current.clear();
            }
        } else {
            current.push(ch);
        }
    }
    if !current.is_empty() {
        args.push(current);
    }
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_inline_args_without_colon() {
        let response = "ACTION: skill-manager/build\nArguments script-scan";
        let args = extract_inline_args(response);
        assert_eq!(args, vec!["script-scan"]);
    }

    #[test]
    fn extract_inline_args_with_colon() {
        let response = "ACTION: skill-manager/build\nArguments: script-scan";
        let args = extract_inline_args(response);
        assert_eq!(args, vec!["script-scan"]);
    }

    #[test]
    fn extract_inline_args_with_multiple_args() {
        let response = "Arguments: --name swap-watcher --flag value";
        let args = extract_inline_args(response);
        assert_eq!(args, vec!["--name", "swap-watcher", "--flag", "value"]);
    }

    #[test]
    fn extract_inline_args_no_arguments_line() {
        let response = "ACTION: skill-manager/build";
        let args = extract_inline_args(response);
        assert!(args.is_empty());
    }

    #[test]
    fn extract_inline_args_quoted_values() {
        let response = "Arguments \"multi word value\" --flag ok";
        let args = extract_inline_args(response);
        assert_eq!(args, vec!["multi word value", "--flag", "ok"]);
    }

    // ── is_action_result_in_history tests ─────────────────────────────────

    #[test]
    fn action_result_detection_probe() {
        let mut h = ChatHistory::new("test".to_string());
        h.turns.push(Turn {
            role: "user".into(),
            content: "[probe result: sysadmin/probe-systemd-failed, exit=0]\nall_clear".to_string(),
        });
        assert!(is_action_result_in_history(&h));
    }

    #[test]
    fn action_result_detection_probe_error() {
        let mut h = ChatHistory::new("test".to_string());
        h.turns.push(Turn {
            role: "user".into(),
            content: "[probe error: sysadmin/probe-systemd-failed] timeout".to_string(),
        });
        assert!(is_action_result_in_history(&h));
    }

    #[test]
    fn action_result_detection_intervention() {
        let mut h = ChatHistory::new("test".to_string());
        h.turns.push(Turn {
            role: "user".into(),
            content: "[intervention result: okapi-watcher/restart-okapi, exit=0]\nok".to_string(),
        });
        assert!(is_action_result_in_history(&h));
    }

    #[test]
    fn action_result_detection_kask() {
        let mut h = ChatHistory::new("test".to_string());
        h.turns.push(Turn {
            role: "user".into(),
            content: "[kask tool result: brave_web_search, status=ok]\nfound results".to_string(),
        });
        assert!(is_action_result_in_history(&h));
    }

    #[test]
    fn action_result_detection_action_error() {
        let mut h = ChatHistory::new("test".to_string());
        h.turns.push(Turn {
            role: "user".into(),
            content: "[action error: 'kask' is not a loaded skill.]".to_string(),
        });
        assert!(is_action_result_in_history(&h));
    }

    #[test]
    fn action_result_detection_plain_user_message() {
        let mut h = ChatHistory::new("test".to_string());
        h.turns.push(Turn {
            role: "user".into(),
            content: "How's the system doing?".to_string(),
        });
        assert!(!is_action_result_in_history(&h));
    }

    #[test]
    fn action_result_detection_assistant_message() {
        let mut h = ChatHistory::new("test".to_string());
        h.turns.push(Turn {
            role: "assistant".into(),
            content: "The system looks fine.".to_string(),
        });
        assert!(!is_action_result_in_history(&h));
    }

    #[test]
    fn action_result_detection_empty_history() {
        let h = ChatHistory::new("test".to_string());
        assert!(!is_action_result_in_history(&h));
    }
}

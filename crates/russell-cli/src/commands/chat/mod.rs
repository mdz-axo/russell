// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell chat` — interactive REPL with Jack via Okapi.
//!
//! This module is split into focused submodules:
//! - [`commands`] — slash-command handlers
//! - [`consent`] — pending action state and affirmation/refusal parsing
//! - [`execute`] — local skill execution (probes & interventions)
//! - [`history`] — chat history persistence
//! - [`kask`] — Kask MCP tool execution (ADR-0025)
//! - [`objective`] — SOAP objective builder
//! - [`spinner`] — LLM call with animated spinner

pub mod commands;
pub mod consent;
pub mod execute;
pub mod history;
pub mod kask;
pub mod objective;
pub mod spinner;

use anyhow::{Context, Result};
use russell_core::journal::{JournalReader, JournalWriter};
use russell_core::paths::Paths;
use russell_meta::action::{self, ResolvedAction};
use russell_mcp::client::KaskMcpClient;
use russell_mcp::config::KaskMcpConfig;
use russell_mcp::registry::ToolRegistry;
use russell_skills::RiskBand;
use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;
use std::io::Write;
use tracing::{debug, info, warn};

use consent::{PendingAction, is_affirmative, is_refusal};
use execute::journal_chat_turn;
use history::{ChatHistory, Turn, save_history};

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

    // Initialize Kask MCP client (ADR-0025). Non-blocking — graceful
    // degradation if Kask is unreachable.
    let kask_config = KaskMcpConfig::from_env();
    let mut kask_client: Option<KaskMcpClient> = None;
    let mut kask_registry = ToolRegistry::new(kask_config.tool_ttl);
    let kask_cache_path = paths.memory_dir().join("kask-tools.cache.json");

    // Load stale-but-useful tool info from disk (ADR-0025 §5, first-boot resilience).
    let _ = kask_registry.load_from_disk(&kask_cache_path);

    if kask_config.validate().is_ok() {
        match KaskMcpClient::new(kask_config.clone()) {
            Ok(mut client) => {
                if let Ok(_init) = client.connect().await {
                    debug!(
                        server = ?client.server_name(),
                        "kask MCP connected"
                    );
                    // Populate tool registry.
                    if let Err(e) = kask_registry.refresh(&client).await {
                        debug!(error = %e, "kask tool registry initial refresh failed");
                    } else {
                        // Persist fresh tools to disk for next boot.
                        let _ = kask_registry.save_to_disk(&kask_cache_path);
                    }
                    kask_client = Some(client);
                } else {
                    debug!("kask MCP connect failed — tools unavailable this session");
                }
            }
            Err(e) => {
                debug!(error = %e, "kask MCP client construction failed");
            }
        }
    }

    let mut history = ChatHistory::new(session_id.clone());
    let mut editor = DefaultEditor::new().context("initialising readline")?;
    let mut pending_action: Option<PendingAction> = None;

    // Load skill registry cache for telemetry display and lifecycle management.
    let registry_path = paths.state.join("registry").join("local-cache.yaml");
    let mut registry = russell_skills::registry::RegistryCache::load(&registry_path)
        .unwrap_or_default();

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
                    if trimmed == "/approve" || is_affirmative(trimmed) {
                        let is_skill_mgr_mutation = pa.action.skill_id() == "skill-manager"
                            && matches!(
                                pa.action.action_id(),
                                "install" | "build" | "create-manifest" | "delete"
                            );
                        let action_result = execute_action(
                            &journal, &kask_client, &pa.action, &session_id,
                            &current_model, paths,
                        ).await;
                        if let Some(result_text) = action_result {
                            history.turns.push(Turn {
                                role: "user".into(),
                                content: result_text,
                            });
                            save_history(&chat_path, &history)?;
                        }
                        if is_skill_mgr_mutation {
                            match russell_skills::load_all(&paths.skills()) {
                                Ok(fresh) => {
                                    skills = fresh;
                                    match russell_skills::registry::RegistryCache::load(&registry_path) {
                                        Ok(fresh_reg) => registry = fresh_reg,
                                        Err(e) => warn!(error = %e, "registry reload failed"),
                                    }
                                }
                                Err(e) => warn!(error = %e, "skills reload after skill-manager intervention failed"),
                            }
                        }
                        pending_action = None;
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
                            &journal, &kask_client, &pa.action, &session_id,
                            &current_model, paths,
                        ).await;
                        if let Some(result_text) = action_result {
                            history.turns.push(Turn {
                                role: "user".into(),
                                content: result_text,
                            });
                            save_history(&chat_path, &history)?;
                        }
                        if is_skill_mgr_mutation {
                            match russell_skills::load_all(&paths.skills()) {
                                Ok(fresh) => {
                                    skills = fresh;
                                    match russell_skills::registry::RegistryCache::load(&registry_path) {
                                        Ok(fresh_reg) => registry = fresh_reg,
                                        Err(e) => warn!(error = %e, "registry reload failed"),
                                    }
                                }
                                Err(e) => warn!(error = %e, "skills reload after skill-manager intervention failed"),
                            }
                        }
                        pending_action = None;
                        continue;
                    }
                    if is_refusal(trimmed) {
                        let denied_action = match &pending_action {
                            Some(pa) => format!("{}/{}", pa.action.skill_id(), pa.action.action_id()),
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
                        save_history(&chat_path, &history)?;

                        // Rerun Jack with the denial context to get a new recommendation.
                        match call_jack(
                            &mut history,
                            &chat_path,
                            &journal,
                            &session_id,
                            &current_model,
                            &reader,
                            &skills,
                            profile.as_ref(),
                            &kask_registry,
                            &registry,
                            paths,
                            &mut pending_action,
                            &kask_client,
                            &client_cfg,
                        ).await {
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
                if trimmed.starts_with('/') {
                    if handle_slash_command(
                        trimmed,
                        &mut skills,
                        &mut editor,
                        &mut current_model,
                        &mut okapi_models,
                        &mut okapi_models_fetched,
                        &base_url,
                        &session_id,
                        &history,
                        paths,
                    ).await {
                        continue;
                    }
                }

                // Add user message to history.
                history.turns.push(Turn {
                    role: "user".into(),
                    content: trimmed.to_string(),
                });

                // Call Jack with the user's message as input.
                call_jack(
                    &mut history,
                    &chat_path,
                    &journal,
                    &session_id,
                    &current_model,
                    &reader,
                    &skills,
                    profile.as_ref(),
                    &kask_registry,
                    &registry,
                    paths,
                    &mut pending_action,
                    &kask_client,
                    &client_cfg,
                )
                .await?;

                // Reload skill registry to capture fresh telemetry (post-Jack execution).
                if let Ok(fresh) = russell_skills::registry::RegistryCache::load(&registry_path) {
                    registry = fresh;
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

/// Unified action executor — dispatches to kask or local skill execution.
async fn execute_action(
    journal: &JournalWriter,
    kask_client: &Option<KaskMcpClient>,
    action: &ResolvedAction,
    session_id: &str,
    model: &str,
    paths: &Paths,
) -> Option<String> {
    if action.is_kask_tool() {
        kask::execute_kask_tool(journal, kask_client, action, session_id, model, paths).await
    } else {
        let pa = PendingAction { action: action.clone(), stdin_content: None };
        execute::execute_pending_action(journal, paths, &pa, session_id, model).await
    }
}

/// Handle a resolved ACTION: proposal from the LLM response.
#[allow(clippy::too_many_arguments)]
async fn handle_action_proposal(
    action: ResolvedAction,
    journal: &JournalWriter,
    kask_client: &Option<KaskMcpClient>,
    session_id: &str,
    current_model: &str,
    paths: &Paths,
    history: &mut ChatHistory,
    chat_path: &std::path::Path,
    pending_action: &mut Option<PendingAction>,
    stdin_content: Option<String>,
) -> Result<()> {
    if action.is_kask_tool() {
        // Kask MCP tool — determine consent based on risk.
        let risk = action.risk_band();
        if risk == RiskBand::None {
            // Auto-execute (probe-equivalent).
            println!("  → Calling kask tool: {}…", action.action_id());
            let result = kask::execute_kask_tool(
                journal, kask_client, &action, session_id, current_model, paths,
            ).await;
            if let Some(result_text) = result {
                history.turns.push(Turn {
                    role: "user".into(),
                    content: result_text,
                });
                save_history(chat_path, history)?;
            }
        } else {
            // Requires consent.
            println!(
                "  → Jack proposes kask tool: {} (risk: {}).",
                action.action_id(), risk.as_str()
            );
            println!("  → Say 'ok' to approve, or 'no' to refuse.");
            *pending_action = Some(PendingAction { action, stdin_content: None });
        }
    } else if action.is_probe() {
        // Probes are read-only — auto-execute immediately.
        println!(
            "  → Running probe: {}/{}…",
            action.skill_id(), action.action_id()
        );
        let pa = PendingAction { action, stdin_content: None };
        let probe_result = execute::execute_pending_action(
            journal, paths, &pa, session_id, current_model,
        ).await;
        if let Some(result_text) = probe_result {
            history.turns.push(Turn {
                role: "user".into(),
                content: result_text,
            });
            save_history(chat_path, history)?;
        }
    } else {
        match &action {
            ResolvedAction::Intervention {
                risk, needs_sudo, ..
            } => {
                let sudo_tag = if *needs_sudo { " [needs sudo]" } else { "" };
                println!(
                    "  → Jack proposes: {}/{} (risk: {:?}{}).",
                    action.skill_id(), action.action_id(), risk, sudo_tag
                );
            }
            _ => unreachable!(),
        }
        println!("  → Say 'ok' to approve, or 'no' to refuse.");
        *pending_action = Some(PendingAction { action, stdin_content });
    }
    Ok(())
}

/// Handle slash commands. Returns `true` if the command was handled
/// (caller should `continue` the REPL loop), `false` if not recognized.
#[allow(clippy::too_many_arguments)]
async fn handle_slash_command(
    trimmed: &str,
    skills: &mut Vec<russell_skills::Skill>,
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
                Ok(fresh) => {
                    *skills = fresh;
                    let now = skills.len();
                    if now > prev_count {
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
            commands::handle_help();
            true
        }
        "/history" => {
            commands::handle_history(session_id, &history.turns);
            true
        }
        "/skills" => {
            commands::handle_skills(skills);
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
                    *okapi_models = commands::okapi_list_models(base_url).await.unwrap_or_default();
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
                    *okapi_models = commands::okapi_list_models(base_url).await.unwrap_or_default();
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
                        if let Some(selected) = commands::prompt_model_selection(
                            editor, trimmed, filtered.len(),
                        ) {
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
                let resolved = russell_meta::oai_client::resolve_model_name(
                    base_url, name, &http,
                ).await;
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
#[allow(clippy::too_many_arguments)]
async fn call_jack(
    history: &mut ChatHistory,
    chat_path: &std::path::Path,
    journal: &JournalWriter,
    session_id: &str,
    current_model: &str,
    reader: &JournalReader,
    skills: &[russell_skills::Skill],
    profile: Option<&russell_core::Profile>,
    kask_registry: &ToolRegistry,
    registry: &russell_skills::registry::RegistryCache,
    paths: &Paths,
    pending_action: &mut Option<PendingAction>,
    kask_client: &Option<KaskMcpClient>,
    _client_cfg: &russell_meta::client::ClientConfig,
) -> Result<()> {
    // Build the fresh SOAP objective.
    let objective = objective::build_objective(reader, skills, profile, kask_registry, registry);

    // Build system prompt: persona + relevance-scored KNOWLEDGE.md injection.
    // All applicable skill knowledge is injected (within token budget),
    // giving Jack full domain expertise in chat mode — matching what
    // `russell jack` receives in one-shot mode.
    let mut system = russell_meta::JACK_CHAT_PERSONA.to_string();
    {
        use russell_meta::prompt_registry::{
            KnowledgeSlot, select_knowledge, score_knowledge_relevance,
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
                        if kw.len() >= 3 { syms.push(kw.to_string()); }
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
                let relevance = score_knowledge_relevance(&skill.symptoms, &active_symptoms);
                let token_estimate = content.len() / 4;
                slots.push(KnowledgeSlot {
                    skill_id: skill.id.clone(),
                    content,
                    relevance,
                    token_estimate,
                });
            }
        }
        let selected = select_knowledge(&mut slots, KNOWLEDGE_BUDGET_TOKENS);
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
        while estimate_message_tokens(&messages) > max_history_tokens && messages.len() > history_start + 2 {
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
    save_history(chat_path, history)?;

    // Call the LLM with an animated thinking spinner.
    let cfg = russell_meta::client::ClientConfig::from_env();
    let response = spinner::call_okapi_with_spinner(&cfg, current_model, &messages).await;

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
            save_history(chat_path, history)?;

            // Journal the chat turn as a help-session event.
            journal_chat_turn(journal, session_id, current_model, "(chat turn)", &content);

            // Check for ACTION: proposal.
            let kask_tool_infos = kask::build_kask_tool_infos(kask_registry);
            match action::resolve_with_kask(&content, skills, &kask_tool_infos) {
                Some(Ok(mut action)) => {
                    // Append inline CLI arguments from the LLM response to
                    // the subprocess cmd (e.g. `Arguments --name swap-watcher`
                    // → appends `--name swap-watcher` to the manifest's cmd).
                    if !action.is_kask_tool() {
                        let inline_args = extract_inline_args(&content);
                        if !inline_args.is_empty() {
                            action.append_cmd_args(&inline_args);
                        }
                    }
                    let manifest = extract_manifest_block(&content);
                    handle_action_proposal(
                        action,
                        journal,
                        kask_client,
                        session_id,
                        current_model,
                        paths,
                        history,
                        chat_path,
                        pending_action,
                        manifest,
                    ).await?;
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
            save_history(chat_path, history)?;
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
        .map(|l| l.trim().strip_prefix("Arguments").unwrap_or(l).trim().to_string());

    let line = match args_line {
        Some(ref l) if !l.is_empty() => l,
        _ => return Vec::new(),
    };

    let mut args = Vec::new();
    let mut chars = line.chars().peekable();
    let mut current = String::new();
    let mut in_quote = false;

    while let Some(ch) = chars.next() {
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

// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell jack` — Jack's interactive session.
//!
//! Supports two modes:
//! - **Single-shot**: `russell jack --note "..."` — one LLM round-trip, print, exit (legacy)
//! - **Interactive**: `russell jack` — multi-turn REPL with consent flow
//!
//! Both modes exercise the same `SessionEngine` from `russell-session`,
//! ensuring functional equivalence with the ACP and API surfaces.

use std::io::{self, Write};

use anyhow::{Context, Result};
use russell_core::journal::JournalWriter;
use russell_core::paths::Paths;
use russell_meta::JACK_PERSONA;
use russell_session::{ConsentDecision, SessionEngine, TurnRole};

pub async fn run(paths: &Paths, note: Option<&str>) -> Result<()> {
    if let Some(note_text) = note {
        run_single_shot(paths, note_text).await
    } else {
        run_interactive(paths).await
    }
}

async fn run_single_shot(paths: &Paths, note: &str) -> Result<()> {
    let writer = JournalWriter::open(&paths.journal())
        .with_context(|| format!("opening journal {}", paths.journal().display()))?;

    let outcome = russell_meta::run_help(paths, &writer, Some(note), &[]).await?;

    println!("{}", outcome.response);
    println!("\n[evidence: {}]", outcome.evidence_dir.display());

    Ok(())
}

async fn run_interactive(paths: &Paths) -> Result<()> {
    let system_prompt = format!(
        "You are Jack, Russell's nurse persona.\n\n\
         {}\n\n\
         CLI Context:\n\
         - You are interacting directly with the operator via the CLI\n\
         - You observe the host, run probes, and recommend actions\n\
         - You NEVER emit shell commands — you rank intervention IDs\n\
         - You propose interventions; the operator consents; the dispatcher executes\n\
         - Type /quit or Ctrl-D to end the session\n\
         - Type /approve to approve a pending intervention\n\
         - Type /deny to deny a pending intervention\n\
         - Type /status to see session status\n\
         - Type /help for available commands",
        JACK_PERSONA
    );

    let mut engine = SessionEngine::new(&system_prompt);

    let create_resp = engine
        .create_session("jack")
        .map_err(|e| anyhow::anyhow!("failed to create session: {}", e))?;

    let session_id = create_resp.session_id;

    println!("Jack session started (id: {})", &session_id[..8]);
    println!("Type your message, /help for commands, /quit to exit.\n");

    let mut rl = rustyline::DefaultEditor::new()?;
    let prompt = "jack> ";

    loop {
        let line = match rl.readline(prompt) {
            Ok(line) => line,
            Err(rustyline::error::ReadlineError::Eof) => break,
            Err(rustyline::error::ReadlineError::Interrupted) => {
                println!("(interrupted — type /quit to exit)");
                continue;
            }
            Err(e) => return Err(e.into()),
        };

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let _ = rl.add_history_entry(trimmed);

        match trimmed {
            "/quit" | "/exit" => break,
            "/help" => {
                print_help();
                continue;
            }
            "/status" => {
                print_status(&engine, &session_id);
                continue;
            }
            "/approve" => {
                handle_consent(&mut engine, &session_id, ConsentDecision::Approve)?;
                continue;
            }
            "/deny" => {
                handle_consent(&mut engine, &session_id, ConsentDecision::Deny)?;
                continue;
            }
            _ => {}
        }

        if trimmed.starts_with('/') {
            println!("Unknown command: {}. Type /help for available commands.", trimmed);
            continue;
        }

        match engine.send_message(&session_id, trimmed) {
            Ok(resp) => {
                println!();
                println!("{}", resp.response);
                println!();

                if let Some(ref action) = resp.pending_action {
                    print_pending_action(action);
                }
            }
            Err(e) => {
                eprintln!("[error] {}", e);
            }
        }
    }

    engine
        .close_session(&session_id)
        .map_err(|e| anyhow::anyhow!("failed to close session: {}", e))?;

    println!("\nSession closed.");
    Ok(())
}

fn print_help() {
    println!("Available commands:");
    println!("  /help     — Show this help");
    println!("  /quit     — End the session");
    println!("  /exit     — End the session");
    println!("  /status   — Show session status");
    println!("  /approve  — Approve pending intervention");
    println!("  /deny     — Deny pending intervention");
    println!();
    println!("Or just type a message to talk to Jack.");
}

fn print_status(engine: &SessionEngine, session_id: &str) {
    match engine.get_session(session_id) {
        Ok(session) => {
            println!(
                "Session: {} | Turns: {} | State: {:?} | Persona: {}",
                &session.id[..8],
                session.turns.len(),
                session.state,
                session.persona
            );
            if session.pending_action.is_some() {
                println!("⚠ Pending action awaiting consent — /approve or /deny");
            }
        }
        Err(e) => eprintln!("[error] {}", e),
    }
}

fn print_pending_action(action: &russell_session::PendingAction) {
    println!();
    println!("⚠ INTERVENTION REQUIRES CONSENT");
    println!("  Skill:         {}", action.skill_id);
    println!("  Intervention:  {}", action.intervention_id);
    println!("  Risk:          {:?}", action.risk);
    println!("  Action ID:     {}", action.action_id);
    println!("  Type /approve to proceed, /deny to refuse.");
    println!();
}

fn handle_consent(
    engine: &mut SessionEngine,
    session_id: &str,
    decision: ConsentDecision,
) -> Result<()> {
    let session = engine
        .get_session(session_id)
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    let action_id = match session.pending_action.as_ref() {
        Some(action) => action.action_id.clone(),
        None => {
            println!("No pending action to respond to.");
            return Ok(());
        }
    };

    let request = russell_session::ConsentRequest {
        session_id: session_id.to_string(),
        action_id,
        decision,
        reason: None,
    };

    match engine.respond_consent(request) {
        Ok(resp) => {
            let label = match decision {
                ConsentDecision::Approve => "APPROVED",
                ConsentDecision::Deny => "DENIED",
            };
            println!("[{}] Intervention {}", label, resp.action_id);
            if let Some(result) = resp.result {
                println!("  Result: {}", result);
            }
            if let Some(error) = resp.error {
                println!("  Error: {}", error);
            }
        }
        Err(e) => eprintln!("[error] {}", e),
    }

    Ok(())
}

// SPDX-License-Identifier: MIT OR Apache-2.0
//! Confirm reflex arc interventions.
//!
//! The Andon cord: operator's explicit approval mechanism for reflex
//! interventions that exceed the auto-execution risk cap.
//!
//! ## Usage
//!
//! ```bash
//! russell confirm list                  # List pending reflex interventions
//! russell confirm <event-id>            # Approve a specific reflex intervention
//! russell confirm <event-id> --deny     # Deny a reflex intervention
//! ```

use anyhow::{Context, Result};
use russell_core::{Event, Severity, paths::Paths};

/// List pending reflex interventions awaiting confirmation.
pub fn list_pending(paths: &Paths) -> Result<()> {
    let journal = russell_core::JournalReader::new(paths.journal());

    // Fetch reflex_proposed events from the last 24 hours
    let now = russell_core::time::now_unix();
    let since = now - (24 * 60 * 60);

    let conn = journal.open_ro_conn()?;
    let mut stmt = conn.prepare(
        "SELECT id, json_extract(payload, '$.outputs.probe'), severity, json_extract(payload, '$.outputs.intervention'), summary
           FROM events
          WHERE action = 'reflex_proposed'
            AND ts_unix >= ?1 AND ts_unix <= ?2
          ORDER BY ts_unix DESC
          LIMIT 10",
    )?;

    let rows = stmt.query_map(rusqlite::params![since, now], |r| {
        let id: i64 = r.get(0)?;
        let probe: String = r.get(1)?;
        let severity: String = r.get(2)?;
        let intervention: String = r.get(3)?;
        let summary: String = r.get(4)?;
        Ok((id, probe, severity, intervention, summary))
    })?;

    let mut has_events = false;
    for row_result in rows {
        if !has_events {
            println!("Pending reflex interventions (last 24h):");
            println!();
            println!(
                "{:<8} {:<20} {:<10} {:<30}",
                "ID", "Probe", "Severity", "Intervention"
            );
            println!("{}", "-".repeat(78));
            has_events = true;
        }

        let (event_id, probe, severity, intervention, _summary) = row_result?;
        println!(
            "{:<8} {:<20} {:<10} {:<30}",
            event_id, probe, severity, intervention
        );
    }

    if !has_events {
        println!("No pending reflex interventions.");
    } else {
        println!();
        println!("Use 'russell confirm <ID>' to approve or 'russell confirm <ID> --deny' to deny.");
    }

    Ok(())
}

/// Approve or deny a specific reflex intervention by event ID.
pub fn confirm_event(paths: &Paths, event_id: i64, deny: bool) -> Result<()> {
    let journal = russell_core::JournalReader::new(paths.journal());

    // Fetch the specific event
    let event = journal
        .get_event(event_id)
        .context("failed to fetch event")?;

    if event.action != "reflex_proposed" {
        anyhow::bail!(
            "Event {} is not a reflex_proposed event (action: {})",
            event_id,
            event.action
        );
    }

    let probe = event
        .outputs
        .get("probe")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let intervention = event
        .outputs
        .get("intervention")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let severity = event.severity.as_str();

    if deny {
        println!("Denying reflex intervention:");
        println!("  Probe: {}", probe);
        println!("  Intervention: {}", intervention);
        println!("  Severity: {}", severity);
        println!();

        // Record the denial in the journal
        let mut denial_event = Event::new("reflex_denied", Severity::Info);
        denial_event.tier = Some("operator".into());
        denial_event.module = Some("reflex/andon".into());
        denial_event.summary = Some(format!(
            "Operator denied reflex arc for probe '{}' (intervention: {})",
            probe, intervention
        ));
        denial_event.outputs = event.outputs.clone();
        denial_event
            .outputs
            .insert("original_event_id".into(), serde_json::json!(event_id));

        let journal_writer = russell_core::JournalWriter::open(&paths.journal())?;
        journal_writer.append(&denial_event)?;

        println!("Recorded denial in journal.");
    } else {
        println!("Approving reflex intervention:");
        println!("  Probe: {}", probe);
        println!("  Intervention: {}", intervention);
        println!("  Severity: {}", severity);
        println!();

        // Record the approval in the journal
        let mut approval_event = Event::new("reflex_confirmed", Severity::Info);
        approval_event.tier = Some("operator".into());
        approval_event.module = Some("reflex/andon".into());
        approval_event.summary = Some(format!(
            "Operator approved reflex arc for probe '{}' (intervention: {})",
            probe, intervention
        ));
        approval_event.outputs = event.outputs.clone();
        approval_event
            .outputs
            .insert("original_event_id".into(), serde_json::json!(event_id));

        let journal_writer = russell_core::JournalWriter::open(&paths.journal())?;
        journal_writer.append(&approval_event)?;

        println!("Recorded approval in journal.");
        println!();
        println!("Note: To execute the intervention, use 'russell skill run <skill>/<action>'.");
    }

    Ok(())
}

/// Main entry point for the confirm command.
pub fn run(paths: &Paths, args: &[String]) -> Result<()> {
    if args.is_empty() {
        anyhow::bail!("Usage: russell confirm <list|<event-id>> [--deny]");
    }

    let first_arg = &args[0];

    // Check if the first arg is --deny (edge case)
    if first_arg == "--deny" || first_arg == "-d" {
        anyhow::bail!("Usage: russell confirm <list|<event-id>> [--deny]");
    }

    if first_arg == "list" {
        return list_pending(paths);
    }

    let event_id: i64 = first_arg.parse().context("event ID must be a number")?;

    let deny = args.iter().any(|arg| arg == "--deny" || arg == "-d");

    confirm_event(paths, event_id, deny)
}

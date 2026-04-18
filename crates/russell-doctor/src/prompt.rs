// SPDX-License-Identifier: MIT OR Apache-2.0
//! SOAP prompt composition.
//!
//! Reads the last 24h of samples + last 20 events from the
//! journal and renders a Markdown-formatted SOAP bundle the LLM
//! can read directly.

use std::fmt::Write as _;

use russell_core::journal::JournalReader;
use russell_core::{Profile, event::Scope};

use crate::client::SoapPrompt;
use crate::error::Result;

/// Build the SOAP prompt. The system prompt is always the
/// embedded Jack persona.
pub fn compose(
    reader: &JournalReader,
    profile: Option<&Profile>,
    note: Option<&str>,
) -> Result<SoapPrompt> {
    let now = russell_core::time::now_unix();
    let window_start = now - 24 * 3600;

    let subjective = match note {
        Some(n) if !n.trim().is_empty() => n.trim().to_string(),
        _ => "(no operator note)".to_string(),
    };

    let mut objective = String::new();
    writeln!(objective, "### Profile")?;
    match profile {
        Some(p) => {
            writeln!(objective, "- profile_id: `{}`", p.profile_id)?;
            writeln!(objective, "- authored_at: `{}`", p.authored_at)?;
            if !p.host.os.distro.is_empty() {
                writeln!(
                    objective,
                    "- host.os: `{}/{}` kernel `{}`",
                    p.host.os.distro, p.host.os.version, p.host.os.kernel
                )?;
            }
            if !p.host.cpu.model.is_empty() {
                writeln!(
                    objective,
                    "- host.cpu: `{}` ({} cores / {} threads)",
                    p.host.cpu.model, p.host.cpu.cores, p.host.cpu.threads
                )?;
            }
            if !p.gpus.is_empty() {
                writeln!(objective, "- gpus:")?;
                for g in &p.gpus {
                    writeln!(
                        objective,
                        "  - `{}` @ `{}` (role: {})",
                        g.name, g.pci, g.role
                    )?;
                }
            }
        }
        None => writeln!(objective, "- (no profile.json)")?,
    }

    writeln!(objective, "\n### Severity counts — last 24h")?;
    let counts = reader.severity_counts(window_start, i64::MAX)?;
    writeln!(
        objective,
        "- info: {} | warn: {} | alert: {} | crit: {}",
        counts.info, counts.warn, counts.alert, counts.crit
    )?;

    writeln!(objective, "\n### Most-recent events (up to 20)")?;
    let rows = reader.recent(20)?;
    if rows.is_empty() {
        writeln!(objective, "- (no events recorded)")?;
    } else {
        writeln!(
            objective,
            "| ts | severity | scope | module | action | summary |"
        )?;
        writeln!(objective, "|---|---|---|---|---|---|")?;
        for r in rows {
            let sev = match r.severity {
                russell_core::event::Severity::Info => "info",
                russell_core::event::Severity::Warn => "warn",
                russell_core::event::Severity::Alert => "alert",
                russell_core::event::Severity::Crit => "crit",
            };
            let scope = match r.scope {
                Scope::Host => "host",
                Scope::Self_ => "self",
            };
            writeln!(
                objective,
                "| {} | {} | {} | {} | {} | {} |",
                r.ts,
                sev,
                scope,
                r.module.as_deref().unwrap_or("-"),
                r.action,
                r.summary.as_deref().unwrap_or("")
            )?;
        }
    }

    writeln!(objective, "\n### Sentinel freshness")?;
    // Approximate by counting the most recent sample across any probe.
    let last_sample_age_s = last_sample_age(reader).unwrap_or(-1);
    if last_sample_age_s >= 0 {
        writeln!(
            objective,
            "- Last sample {} seconds ago.",
            last_sample_age_s
        )?;
    } else {
        writeln!(objective, "- No samples recorded.")?;
    }

    let mut rendered = String::new();
    writeln!(rendered, "# SOAP — russell help\n")?;
    writeln!(rendered, "## Subjective\n\n{subjective}\n")?;
    writeln!(rendered, "## Objective\n\n{objective}\n")?;
    writeln!(
        rendered,
        "## Assessment\n\n*(your job, Jack — fill this in based on the evidence above.)*\n"
    )?;
    writeln!(rendered, "## Plan\n\n*(your job, Jack — one next step.)*\n")?;

    Ok(SoapPrompt {
        system: crate::JACK_PERSONA.to_string(),
        subjective,
        objective,
        rendered,
    })
}

fn last_sample_age(reader: &JournalReader) -> Option<i64> {
    let ts = reader.last_sample_ts().ok().flatten()?;
    let now = russell_core::time::now_unix();
    Some(now - ts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use russell_core::event::{Event, Severity};
    use russell_core::journal::JournalWriter;

    #[test]
    fn compose_handles_empty_journal() {
        let tmp = tempfile::tempdir().unwrap();
        let db = tmp.path().join("journal.db");
        let w = JournalWriter::open(&db).unwrap();
        let reader = w.reader();
        let prompt = compose(&reader, None, None).unwrap();
        assert!(prompt.rendered.contains("## Subjective"));
        assert!(prompt.rendered.contains("(no operator note)"));
        assert!(prompt.rendered.contains("(no events recorded)"));
        assert!(prompt.system.contains("You are Jack"));
    }

    #[test]
    fn compose_includes_note_and_events() {
        let tmp = tempfile::tempdir().unwrap();
        let db = tmp.path().join("journal.db");
        let w = JournalWriter::open(&db).unwrap();
        let mut e = Event::new("observe", Severity::Warn);
        e.module = Some("daily/gpu-sanity".into());
        e.summary = Some("one vm fault".into());
        w.append(&e).unwrap();
        let reader = w.reader();
        let prompt = compose(&reader, None, Some("ollama is slow")).unwrap();
        assert!(prompt.rendered.contains("ollama is slow"));
        assert!(prompt.rendered.contains("daily/gpu-sanity"));
        assert!(prompt.rendered.contains("warn"));
    }
}

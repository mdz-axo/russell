// SPDX-License-Identifier: MIT OR Apache-2.0
//! Offline rule-based fallback.
//!
//! Jack is never silent. When the network is down, the API key
//! is missing, or any LLM call fails, the Nurse emits a short
//! deterministic summary in Jack's voice.

use std::fmt::Write as _;

use russell_core::journal::{JournalReader, SeverityCounts};

use crate::error::Result;

/// Compose the offline response given a journal snapshot.
pub fn summarise(reader: &JournalReader, note: Option<&str>) -> Result<String> {
    let now = russell_core::time::now_unix();
    let since_24h = now - 24 * 3600;
    let counts = reader.severity_counts(since_24h, i64::MAX)?;
    let rows = reader.recent(5)?;

    let mut out = String::new();
    writeln!(
        out,
        "Offline. No LLM today — here's what Jack can say on his own.\n"
    )?;

    if let Some(n) = note.filter(|n| !n.trim().is_empty()) {
        writeln!(out, "Note: {}\n", n.trim())?;
    }

    writeln!(
        out,
        "Last 24h: info {} · warn {} · alert {} · crit {}.",
        counts.info, counts.warn, counts.alert, counts.crit
    )?;

    match verdict(&counts) {
        Verdict::Clean => {
            writeln!(
                out,
                "Nothing notable. Machine looks fine from where I'm sitting."
            )?
        }
        Verdict::Soft => {
            writeln!(out, "Some warnings worth a look; nothing urgent.")?
        }
        Verdict::Hard => {
            writeln!(out, "Alerts in the window. Check the evidence.")?
        }
    }

    if !rows.is_empty() {
        writeln!(out, "\nMost recent:")?;
        for r in rows {
            let sev = match r.severity {
                russell_core::event::Severity::Info => "info",
                russell_core::event::Severity::Warn => "warn",
                russell_core::event::Severity::Alert => "alert",
                russell_core::event::Severity::Crit => "crit",
            };
            writeln!(
                out,
                "- [{}] {} · {} · {}",
                sev,
                r.ts,
                r.module.as_deref().unwrap_or("-"),
                r.summary.as_deref().unwrap_or(&r.action)
            )?;
        }
    }

    writeln!(
        out,
        "\nVerify Okapi is running (`systemctl --user status okapi`) and set \
         OPENROUTER_API_KEY in `~/.config/harness/russell.env` if you want \
         the remote fallback. Call me back when the phone's working."
    )?;
Ok(out)
}

#[derive(Debug, Clone, Copy)]
enum Verdict {
    Clean,
    Soft,
    Hard,
}

fn verdict(c: &SeverityCounts) -> Verdict {
    if c.crit > 0 || c.alert > 0 {
        Verdict::Hard
    } else if c.warn > 0 {
        Verdict::Soft
    } else {
        Verdict::Clean
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use russell_core::event::{Event, Severity};
    use russell_core::journal::JournalWriter;

    #[test]
    fn clean_fallback_mentions_nothing_notable() {
        let tmp = tempfile::tempdir().unwrap();
        let w = JournalWriter::open(&tmp.path().join("j.db")).unwrap();
        let s = summarise(&w.reader(), None).unwrap();
        assert!(s.contains("Nothing notable"));
        assert!(s.contains("Offline"));
    }

    #[test]
    fn hard_verdict_on_crit_event() {
        let tmp = tempfile::tempdir().unwrap();
        let w = JournalWriter::open(&tmp.path().join("j.db")).unwrap();
        w.append(&Event::new("observe", Severity::Crit)).unwrap();
        let s = summarise(&w.reader(), None).unwrap();
        assert!(s.contains("Alerts in the window"));
    }

    #[test]
    fn note_passed_through() {
        let tmp = tempfile::tempdir().unwrap();
        let w = JournalWriter::open(&tmp.path().join("j.db")).unwrap();
        let s = summarise(&w.reader(), Some("ollama is slow")).unwrap();
        assert!(s.contains("ollama is slow"));
    }
}

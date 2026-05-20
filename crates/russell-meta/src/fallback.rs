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
        Verdict::Clean => writeln!(
            out,
            "Nothing notable. Machine looks fine from where I'm sitting."
        )?,
        Verdict::Soft => writeln!(out, "Some warnings worth a look; nothing urgent.")?,
        Verdict::Hard => writeln!(out, "Alerts in the window. Check the evidence.")?,
    }

    if !rows.is_empty() {
        writeln!(out, "\nMost recent:")?;
        for r in rows {
            writeln!(
                out,
                "- [{}] {} · {} · {}",
                r.severity.as_str(),
                r.ts,
                r.module.as_deref().unwrap_or("-"),
                r.summary.as_deref().unwrap_or(&r.action)
            )?;
        }
    }

    writeln!(
        out,
        "\nVerify Okapi is running (`systemctl --user status okapi`). \
         Call me back when the phone's working."
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

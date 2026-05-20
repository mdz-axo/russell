// SPDX-License-Identifier: MIT OR Apache-2.0
//! Shared prompt builder functions — consolidated from prompt.rs and prompt_unified.rs.
//!
//! These builders are used by both legacy `compose_with_kask()` and new
//! `compose_templated()` paths. Consolidation reduces duplication (~400 lines saved).

use russell_core::journal::JournalReader;
use russell_core::profile::Profile;

use crate::error::Result;

/// Build the profile block for SOAP prompt.
pub fn build_profile_block(profile: Option<&Profile>) -> String {
    use std::fmt::Write;
    let mut block = String::new();
    match profile {
        Some(p) => {
            let _ = writeln!(
                block,
                "- host.os: `{}/{}` kernel `{}`",
                p.host.os.distro, p.host.os.version, p.host.os.kernel
            );
            let _ = writeln!(
                block,
                "- host.cpu: `{}` ({} cores / {} threads)",
                p.host.cpu.model, p.host.cpu.cores, p.host.cpu.threads
            );
            if !p.host.gfx.is_empty() {
                for g in &p.host.gfx {
                    let _ = writeln!(
                        block,
                        "  - `{}` @ `{}` (role: {})",
                        g.name, g.pci, g.role
                    );
                }
            }
        }
        None => block.push_str("- (no profile.json)"),
    }
    block
}

/// Build severity summary block from journal.
pub fn build_severity_block(reader: &JournalReader, window_start: i64) -> Result<String> {
    let counts = reader.severity_counts(window_start, i64::MAX)?;
    Ok(format!(
        "info {} · warn {} · alert {} · crit {}",
        counts.info, counts.warn, counts.alert, counts.crit
    ))
}

/// Build samples table from journal.
pub fn build_samples_table(reader: &JournalReader, window_start: i64) -> Result<String> {
    let samples = reader.recent_samples(window_start, 50)?;
    let mut table = String::from("| Probe | Last | Avg | Max | Count |\n|---|---|---|---|---|\n");
    for sample in samples {
        let avg = sample.avg.unwrap_or(sample.last);
        let max = sample.max.unwrap_or(sample.last);
        let count = sample.count;
        let _ = writeln!(
            table,
            "| {} | {} | {} | {} | {} |",
            sample.probe,
            fmt_f64(sample.last),
            fmt_f64(avg),
            fmt_f64(max),
            count
        );
    }
    Ok(table)
}

/// Build freshness block (time since last sample).
pub fn build_freshness_block(reader: &JournalReader) -> String {
    match last_sample_age(reader) {
        Some(age) => format!("Last sample: {}s ago", age),
        None => "No samples yet".into(),
    }
}

/// Build events table from journal.
pub fn build_events_table(reader: &JournalReader) -> Result<String> {
    let events = reader.recent(20)?;
    let mut table = String::from("| Severity | Module | Summary |\n|---|---|---|\n");
    for ev in events {
        let module = ev.module.as_deref().unwrap_or("-");
        let summary = ev.summary.as_deref().unwrap_or(&ev.action);
        let _ = writeln!(table, "| {} | {} | {} |", ev.severity.as_str(), module, summary);
    }
    Ok(table)
}

/// Build reflex block from journal.
pub fn build_reflex_block(reader: &JournalReader) -> Result<String> {
    let reflex = reader.recent_reflex_verdicts(5)?;
    if reflex.is_empty() {
        return Ok("No reflex verdicts yet".into());
    }
    let mut block = String::from("| Timestamp | Verdict | Breaches |\n|---|---|---|\n");
    for r in reflex {
        let _ = writeln!(block, "| {} | {} | {} |", r.ts, r.verdict, r.breaches);
    }
    Ok(block)
}

/// Get age of last sample in seconds.
fn last_sample_age(reader: &JournalReader) -> Option<i64> {
    let now = russell_core::time::now_unix();
    reader.last_sample_ts().ok().flatten().map(|ts| now.saturating_sub(ts))
}

/// Format optional f64 for display.
fn fmt_f64(v: Option<f64>) -> String {
    match v {
        Some(x) => format!("{:.2f}", x),
        None => "—".into(),
    }
}

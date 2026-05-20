// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell digest` — Markdown summary of recent activity.
//!
//! Supports two output formats:
//! - `stdout` (default): renders a digest to stdout.
//! - `daily-log`: writes `memory/daily/YYYY-MM-DD.md` per the
//!   [daily log template](../../../docs/templates/daily-log.md).
//!   ADR-0022.

use std::fmt::Write as _;

use anyhow::{Context, Result};
use russell_core::journal::JournalReader;
use russell_core::paths::Paths;
use russell_core::time;

pub fn run(paths: &Paths, since_hours: u32, format: &str) -> Result<()> {
    match format {
        "daily-log" => write_daily_log(paths),
        "stdout" => render_stdout(paths, since_hours),
        _ => render_stdout(paths, since_hours),
    }
}

/// Renders the digest to stdout (existing behaviour).
fn render_stdout(paths: &Paths, since_hours: u32) -> Result<()> {
    let now = time::now_unix();
    let since = now - i64::from(since_hours) * 3600;

    let mut out = String::new();
    writeln!(
        out,
        "# Russell digest — last {since_hours}h\n\n_Generated {}_\n",
        time::now_rfc3339()
    )?;

    // Profile summary.
    writeln!(out, "## Profile\n")?;
    let profile_path = paths.profile();
    if profile_path.exists() {
        match russell_core::Profile::load(&profile_path) {
            Ok(p) => {
                writeln!(out, "- profile_id: `{}`", p.profile_id)?;
                writeln!(out, "- authored_at: `{}`", p.authored_at)?;
                if let Some(end) = p.honeymoon_ends_at.as_deref() {
                    writeln!(out, "- honeymoon_ends_at: `{end}`")?;
                }
            }
            Err(e) => writeln!(out, "- load error: {e}")?,
        }
    } else {
        writeln!(out, "- no profile.json — run `russell profile --init`")?;
    }

    // Journal summary.
    writeln!(out, "\n## Events\n")?;
    let journal_path = paths.journal();
    if !journal_path.exists() {
        writeln!(out, "- journal absent. Run `russell sentinel-once`.")?;
        print!("{out}");
        return Ok(());
    }

    let reader = JournalReader::new(&journal_path);
    let counts = reader
        .severity_counts(since, i64::MAX)
        .with_context(|| format!("reading {}", journal_path.display()))?;
    writeln!(out, "- info:  {}", counts.info)?;
    writeln!(out, "- warn:  {}", counts.warn)?;
    writeln!(out, "- alert: {}", counts.alert)?;
    writeln!(out, "- crit:  {}", counts.crit)?;

    // Most-recent 10 events.
    let rows = reader.recent(10)?;
    if !rows.is_empty() {
        writeln!(out, "\n## Most-recent events\n")?;
        writeln!(out, "| ts | severity | scope | module | action | summary |")?;
        writeln!(out, "|---|---|---|---|---|---|")?;
        for r in rows {
            writeln!(
                out,
                "| {} | {} | {} | {} | {} | {} |",
                r.ts,
                r.severity.as_str(),
                r.scope.as_str(),
                r.module.as_deref().unwrap_or("-"),
                r.action,
                r.summary.as_deref().unwrap_or("")
            )?;
        }
    }

    print!("{out}");
    Ok(())
}

/// Writes the daily log to `memory/daily/YYYY-MM-DD.md`.
/// ADR-0022: the Markdown file is a derived export from the journal.
/// If the file already exists, it is overwritten — the journal is the
/// sole canonical store.
fn write_daily_log(paths: &Paths) -> Result<()> {
    let now = russell_core::time::now_unix();
    let dt = ::time::OffsetDateTime::from_unix_timestamp(now)
        .map_err(|e| anyhow::anyhow!("invalid timestamp: {e}"))?;

    let (year, month, day) = (dt.year(), u8::from(dt.month()), dt.day());
    let date_str = format!("{year:04}-{month:02}-{day:02}");
    let filename = format!("{date_str}.md");
    let path = paths.memory_daily_dir().join(&filename);

    let journal_path = paths.journal();
    let reader = if journal_path.exists() {
        Some(JournalReader::new(&journal_path))
    } else {
        None
    };

    let mut out = String::new();

    // Header
    writeln!(out, "# Russell Daily Log — {date_str}\n")?;

    // Summary block
    writeln!(out, "## Summary")?;
    if let Some(ref r) = reader {
        let since = day_start_unix(now);
        let counts = r
            .severity_counts(since, i64::MAX)
            .with_context(|| format!("reading {}", journal_path.display()))?;
        let sessions = r
            .help_sessions_in_range(since, i64::MAX)
            .unwrap_or_default();
        let session_count = sessions.len();
        writeln!(out, "- Sessions: {session_count} help calls")?;
        writeln!(
            out,
            "- Alerts: {} | Warnings: {} | Info: {}",
            counts.alert, counts.warn, counts.info
        )?;
        // Self-health: check if sentinel ran recently.
        let health = match r.last_host_sample_ts().unwrap_or(None) {
            Some(ts) if now - ts <= 900 => "healthy",
            Some(_) => "degraded — sentinel may be stale",
            None => "degraded — no host samples",
        };
        writeln!(out, "- Self-health: {health}")?;
    } else {
        writeln!(out, "- No journal yet. Run `russell sentinel-once`.")?;
        writeln!(out, "- Self-health: unknown")?;
    }

    // Session Notes
    writeln!(out, "\n## Session Notes")?;
    if let Some(ref r) = reader {
        let since = day_start_unix(now);
        let sessions = r
            .help_sessions_in_range(since, i64::MAX)
            .unwrap_or_default();
        if sessions.is_empty() {
            writeln!(out, "- (no sessions today)")?;
        } else {
            for s in &sessions {
                let summary = match s.note.as_deref() {
                    Some(n) if !n.trim().is_empty() => format!("{n} [{}]", s.status.as_str()),
                    _ => format!("(no note) [{}]", s.status.as_str()),
                };
                writeln!(out, "- [{}] — {summary}", s.id)?;
            }
        }
    } else {
        writeln!(out, "- (no journal)")?;
    }

    // Retain section (empty — filled by operator or `russell reflect`)
    writeln!(out, "\n## Retain")?;
    writeln!(
        out,
        "<!-- 2–5 durable observations. Tagged: W=world fact, B=biographical, O=opinion -->"
    )?;
    writeln!(out, "<!-- Entries here survive journal compaction. -->")?;
    writeln!(
        out,
        "<!-- See docs/templates/daily-log.md for conventions. -->"
    )?;
    writeln!(out)?;
    writeln!(out, "<!-- Add entries below: -->")?;

    // Write the file (overwrites if exists — rebuildable guarantee).
    std::fs::write(&path, &out)
        .with_context(|| format!("writing daily log to {}", path.display()))?;

    // Print a brief confirmation to stdout.
    println!("Daily log written: {}", path.display());
    println!(
        "  Sessions: {}",
        reader.as_ref().map_or(0, |r| r
            .help_sessions_in_range(day_start_unix(now), i64::MAX)
            .unwrap_or_default()
            .len())
    );
    println!("  Edit `{}` to add ## Retain entries.", path.display());

    Ok(())
}

/// Unix timestamp for the start of today (midnight UTC).
fn day_start_unix(now_unix: i64) -> i64 {
    let days_since_epoch = now_unix / 86_400;
    days_since_epoch * 86_400
}


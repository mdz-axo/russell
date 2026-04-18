// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell digest` — Markdown summary of recent activity.
//!
//! Phase 0 renders to stdout only. `cybernetic-health-harness.md`
//! §14 also describes an HTML dashboard; that lands in Phase 1.

use std::fmt::Write as _;

use anyhow::{Context, Result};
use russell_core::paths::Paths;

pub fn run(paths: &Paths, since_hours: u32) -> Result<()> {
    let now = russell_core::time::now_unix();
    let since = now - i64::from(since_hours) * 3600;

    let mut out = String::new();
    writeln!(
        out,
        "# Russell digest — last {since_hours}h\n\n_Generated {}_\n",
        russell_core::time::now_rfc3339()
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

    let reader = russell_core::journal::JournalReader::new(&journal_path);
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
            let sev = match r.severity {
                russell_core::event::Severity::Info => "info",
                russell_core::event::Severity::Warn => "warn",
                russell_core::event::Severity::Alert => "alert",
                russell_core::event::Severity::Crit => "crit",
            };
            let scope = match r.scope {
                russell_core::event::Scope::Host => "host",
                russell_core::event::Scope::Self_ => "self",
            };
            writeln!(
                out,
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

    print!("{out}");
    Ok(())
}

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
    writ
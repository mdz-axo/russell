// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell list` — print recent journal events.

use anyhow::{Context, Result};
use russell_core::paths::Paths;

pub fn run(paths: &Paths, limit: usize) -> Result<()> {
    let journal_path = paths.journal();
    if !journal_path.exists() {
        println!("journal absent at {}", journal_path.display());
        println!("run `russell sentinel-once` to create it.");
        return Ok(());
    }
    let reader = russell_core::journal::JournalReader::new(&journal_path);
    let rows = reader
        .recent(limit)
        .with_context(|| format!("reading journal at {}", journal_path.display()))?;
    if rows.is_empty() {
        println!("(no events recorded yet)");
        return Ok(());
    }
    println!(
        "{:<26} {:<8} {:<5} {:<24} {:<14} summary",
        "ts", "sev", "scope", "module", "action"
    );
    for r in rows {
        println!(
            "{:<26} {:<8} {:<5} {:<24} {:<14} {}",
            r.ts,
            r.severity.as_str(),
            r.scope.as_str(),
            r.module.as_deref().unwrap_or("-"),
            r.action,
            r.summary.as_deref().unwrap_or("")
        );
    }
    Ok(())
}

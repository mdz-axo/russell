// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell jack` — Jack's cry-for-help channel.

use anyhow::{Context, Result};
use russell_core::journal::JournalWriter;
use russell_core::paths::Paths;
use russell_meta::run_help;

pub async fn run(paths: &Paths, note: Option<&str>) -> Result<()> {
    let writer = JournalWriter::open(&paths.journal())
        .with_context(|| format!("opening journal {}", paths.journal().display()))?;

    let outcome = run_help(paths, &writer, note, &[]).await?;

    println!("{}", outcome.response);
    println!("\n[evidence: {}]", outcome.evidence_dir.display());

    Ok(())
}

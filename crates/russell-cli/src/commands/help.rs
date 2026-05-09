// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell help` — Jack's cry-for-help channel.

use anyhow::{Context, Result};
use russell_core::journal::JournalWriter;
use russell_core::paths::Paths;

pub async fn run(paths: &Paths, note: Option<&str>) -> Result<()> {
    let writer = JournalWriter::open(&paths.journal())
        .with_context(|| format!("opening journal {}", paths.journal().display()))?;

    let outcome = russell_doctor::run_help(paths, &writer, note)
        .await
        .context("running Doctor help flow")?;

    // The CLI prints the response, then a one-line footer.
    println!("{}", outcome.response.trim_end());
    println!();
    println!(
        "  [jack via {} · session {} · bundle {}]",
        outcome.backend,
        outcome.session_id,
        outcome.evidence_dir.display()
    );

    // ADR-0020: skip_reason replaces fell_back
    if let Some(sr) = outcome.skip_reason {
        let msg = match sr {
            russell_doctor::help::SkipReason::OfflineFallback => {
                "  [offline fallback engaged — set OPENROUTER_API_KEY for the real Kimi]"
            }
            russell_doctor::help::SkipReason::ThresholdSkip => {
                "  [below escalation threshold — rule-based summary returned]"
            }
        };
        println!("{msg}");
    }

    Ok(())
}

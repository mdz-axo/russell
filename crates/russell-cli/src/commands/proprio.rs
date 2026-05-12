// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell proprio` — Russell self-observation.

use anyhow::{Context, Result};
use russell_core::journal::{JournalReader, JournalWriter};
use russell_core::paths::Paths;

pub fn run(paths: &Paths) -> Result<()> {
    let journal = JournalWriter::open(&paths.journal())
        .with_context(|| format!("opening journal {}", paths.journal().display()))?;
    let reader = JournalReader::new(&paths.journal());

    let result = russell_proprio::run_once(&journal, &reader).context("running proprioception")?;

    println!("Proprioception results:");
    if let Some(age) = result.age_s {
        println!("  sentinel_last_run_age_s:  {age}s ({:?})", result.severity);
    } else {
        println!("  sentinel_last_run_age_s:  (no host samples yet)");
    }
    if let Some(stall) = result.journal_stall_s {
        println!(
            "  journal_writer_stall_s:   {stall}s ({:?})",
            result.journal_stall_severity
        );
    }
    if let Some(p95) = result.llm_p95_latency_ms {
        println!(
            "  llm_p95_latency_ms:       {p95:.0}ms ({:?})",
            result.llm_p95_severity
        );
    }
    if let Some(drift) = result.timer_drift_s {
        println!(
            "  timer_drift_s:            {drift}s ({:?})",
            result.timer_drift_severity
        );
    }
    if let Some(rate) = result.help_error_rate_pct {
        println!(
            "  help_error_rate_pct:      {rate:.1}% ({:?})",
            result.help_error_rate_severity
        );
    }
    Ok(())
}

// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell proprio` — Russell self-observation.

use anyhow::{Context, Result};
use russell_core::journal::{JournalReader, JournalWriter};
use russell_core::paths::Paths;
use russell_proprio::KaskHealthInput;

pub async fn run(paths: &Paths) -> Result<()> {
    let journal = JournalWriter::open(&paths.journal())
        .with_context(|| format!("opening journal {}", paths.journal().display()))?;
    let reader = JournalReader::new(paths.journal());

    // Perform Kask MCP health probe asynchronously before the sync proprio cycle.
    let kask_health_raw = russell_mcp::health::probe_reachability().await;
    let kask_input = KaskHealthInput {
        reachable: kask_health_raw.reachable,
        latency_ms: kask_health_raw.latency_ms,
    };

    let result = russell_proprio::run_once_with_kask(&journal, &reader, kask_input)
        .context("running proprioception")?;

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

    // Kask MCP reachability (Phase 4C, ADR-0025 §5 — now journaled by proprio).
    match result.kask_mcp_reachable_ms {
        Some(ms) => {
            println!(
                "  kask_mcp_reachable_ms:    {ms}ms ({:?})",
                result.kask_mcp_reachable_severity
            );
        }
        None => {
            println!(
                "  kask_mcp_reachable_ms:    unreachable ({:?})",
                result.kask_mcp_reachable_severity
            );
        }
    }

    Ok(())
}

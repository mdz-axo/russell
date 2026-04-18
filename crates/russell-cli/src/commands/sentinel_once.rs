// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell sentinel-once` — fire the Sentinel once.
//!
//! Phase-0 helper. The timer-driven Sentinel lands in Phase 1.

use anyhow::{Context, Result};
use russell_core::event::{Event, Severity};
use russell_core::journal::JournalWriter;
use russell_core::paths::Paths;

pub fn run(paths: &Paths) -> Result<()> {
    let started = std::time::Instant::now();
    let journal = JournalWriter::open(&paths.journal())
        .with_context(|| format!("opening journal {}", paths.journal().display()))?;

    let n = russell_sentinel::run_once(&journal).context("running Sentinel")?;

    let mut ev = Event::new("observe", Severity::Info);
    ev.tier = Some("sentinel".into());
    ev.module = Some("sentinel/phase0".into());
    ev.summary = Some(format!("captured {n} samples"));
    ev.duration_ms = Some(started.elapsed().as_millis() as u64);
    ev.outputs
        .insert("sample_count".into(), serde_json::Value::from(n as u64));
    journal.append(&ev)?;

    println!(
        "sentinel: captured {n} samples in {} ms",
        started.elapsed().as_millis()
    );
    Ok(())
}

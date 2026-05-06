// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell sentinel-once` — fire the Sentinel once.
//!
//! Runs the proprioception self-vital (JR-5) FIRST — measuring
//! how stale the previous cycle's data is — then the host probe
//! set, then emits a cycle event summarising both.
//!
//! The ordering matters: proprio must read `MAX(ts)` from host
//! samples *before* the current cycle writes new ones, otherwise
//! the age is always ~0 s and the self-vital can never trigger.

use anyhow::{Context, Result};
use russell_core::event::{Event, Severity};
use russell_core::journal::JournalWriter;
use russell_core::paths::Paths;

pub fn run(paths: &Paths) -> Result<()> {
    let started = std::time::Instant::now();
    let journal = JournalWriter::open(&paths.journal())
        .with_context(|| format!("opening journal {}", paths.journal().display()))?;

    // 1. Proprioception: self-vital (JR-5).
    //    Must run BEFORE host probes so it measures the age of the
    //    *previous* cycle's samples, not the ones we're about to write.
    let reader = journal.reader();
    let proprio = russell_proprio::run_once(&journal, &reader).context("running proprioception")?;

    // 2. Host probes.
    let n = russell_sentinel::run_once(&journal).context("running Sentinel")?;

    // 3. Cycle event.
    let mut ev = Event::new("observe", Severity::Info);
    ev.tier = Some("sentinel".into());
    ev.module = Some("sentinel/cycle".into());
    ev.summary = Some(format!(
        "captured {n} host samples; self-vital age={}s severity={:?}",
        proprio
            .age_s
            .map(|a| a.to_string())
            .unwrap_or_else(|| "none".into()),
        proprio.severity,
    ));
    ev.duration_ms = Some(started.elapsed().as_millis() as u64);
    ev.outputs
        .insert("sample_count".into(), serde_json::Value::from(n as u64));
    ev.outputs.insert(
        "proprio_age_s".into(),
        match proprio.age_s {
            Some(a) => serde_json::Value::from(a),
            None => serde_json::Value::Null,
        },
    );
    ev.outputs.insert(
        "proprio_severity".into(),
        serde_json::Value::from(format!("{:?}", proprio.severity)),
    );
    journal.append(&ev)?;

    println!(
        "sentinel: captured {n} samples in {} ms; self-vital: age={}s ({:?})",
        started.elapsed().as_millis(),
        proprio
            .age_s
            .map(|a| a.to_string())
            .unwrap_or_else(|| "n/a".into()),
        proprio.severity,
    );
    Ok(())
}

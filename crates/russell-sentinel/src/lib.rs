// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell-sentinel` — Phase-0 scaffold.
//!
//! Ships a tiny, OS-neutral probe set sufficient to drive the
//! `status` / `digest` read-only CLI subcommands listed for
//! Phase 0 in `cybernetic-health-harness.md` §20. The full rule
//! engine, EWMA baselines, and probe catalogue arrive in Phase 1.

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms)]
#![warn(missing_docs)]

pub mod probes;

use russell_core::Result;
use russell_core::event::Scope;
use russell_core::journal::JournalWriter;

/// Run the Phase-0 probe set once and append the samples to the
/// journal. Returns the number of samples written.
///
/// # Errors
///
/// Returns [`russell_core::CoreError::Sqlite`] or related core
/// errors if a journal write fails.
pub fn run_once(writer: &JournalWriter) -> Result<usize> {
    let ts = russell_core::time::now_unix();
    let samples = probes::collect();
    for s in &samples {
        writer.append_sample(
            ts,
            Scope::Host,
            &s.name,
            s.value_num,
            s.value_text.as_deref(),
            s.unit,
        )?;
    }
    Ok(samples.len())
}

// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell-proprio` — proprioception: Russell watches Russell.
//!
//! Phase 2 entry point. Implements the MVP self-vital defined in
//! [`MVP_SPEC.md §3`](../../../docs/specifications/MVP_SPEC.md):
//!
//! | Self-vital              | Source                          | Rule                                      |
//! |-------------------------|---------------------------------|-------------------------------------------|
//! | `sentinel_last_run_age_s` | journal `MAX(ts)` on host samples | Warn if > 450 s; Alert if > 1 800 s     |
//!
//! This crate is deliberately tiny (JR-1). It provides one
//! function, [`run_once`], which:
//!
//! 1. Reads the most-recent host-scope sample timestamp from the
//!    journal.
//! 2. Computes the age in seconds.
//! 3. Writes a self-scope sample (`sentinel_last_run_age_s`).
//! 4. If the age exceeds the hard-coded thresholds, emits a
//!    self-scope event at the appropriate severity.
//!
//! See [ADR-0015](../../../docs/adr/0015-proprioception-self-health.md).

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms)]
#![warn(missing_docs)]

use russell_core::Result;
use russell_core::event::{Event, Scope, Severity};
use russell_core::journal::{JournalReader, JournalWriter};
use tracing::debug;

/// Probe name for the one MVP self-vital.
pub const PROBE_SENTINEL_AGE: &str = "sentinel_last_run_age_s";

/// Threshold (seconds) above which the self-vital emits `warn`.
/// 1.5× the 300 s cadence.
pub const WARN_THRESHOLD_S: i64 = 450;

/// Threshold (seconds) above which the self-vital emits `alert`.
/// 6× the 300 s cadence (30 minutes).
pub const ALERT_THRESHOLD_S: i64 = 1_800;

/// Result of a single proprioception cycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProprioResult {
    /// The computed age in seconds, or `None` if no host samples
    /// exist yet (first-ever cycle).
    pub age_s: Option<i64>,
    /// The severity band the age fell into.
    pub severity: Severity,
    /// Whether an event was emitted (only for `warn` or above).
    pub event_emitted: bool,
}

/// Run the proprioception cycle once.
///
/// Reads the most-recent host-scope sample timestamp, computes
/// the age, writes a self-scope sample, and (if thresholds are
/// breached) emits a self-scope event.
///
/// # Errors
///
/// Returns [`russell_core::CoreError`] on journal I/O failures.
pub fn run_once(writer: &JournalWriter, reader: &JournalReader) -> Result<ProprioResult> {
    let now = russell_core::time::now_unix();

    // Read the most-recent host-scope sample timestamp.
    let last_host_ts = last_host_sample_ts(reader)?;

    let (age_s, severity) = match last_host_ts {
        Some(ts) => {
            let age = now.saturating_sub(ts);
            let sev = classify_age(age);
            (Some(age), sev)
        }
        // No host samples yet — first cycle. Not alarming.
        None => (None, Severity::Info),
    };

    // Write the self-scope sample.
    writer.append_sample(
        now,
        Scope::Self_,
        PROBE_SENTINEL_AGE,
        age_s.map(|a| a as f64),
        None,
        Some("s"),
    )?;

    debug!(
        age_s = ?age_s,
        severity = ?severity,
        "proprio: sentinel_last_run_age_s"
    );

    // Emit an event only when severity is warn or above.
    let event_emitted = severity != Severity::Info;
    if event_emitted {
        let mut ev = Event::new("self_vital_breach", severity);
        ev.scope = Scope::Self_;
        ev.tier = Some("proprio".into());
        ev.module = Some("proprio/sentinel_age".into());
        ev.summary = Some(format!(
            "sentinel_last_run_age_s = {} (threshold: {} for {:?})",
            age_s.unwrap_or(-1),
            if severity == Severity::Alert || severity == Severity::Crit {
                ALERT_THRESHOLD_S
            } else {
                WARN_THRESHOLD_S
            },
            severity,
        ));
        ev.outputs
            .insert("age_s".into(), serde_json::Value::from(age_s.unwrap_or(-1)));
        writer.append(&ev)?;
    }

    Ok(ProprioResult {
        age_s,
        severity,
        event_emitted,
    })
}

/// Classify the age into a severity band per MVP_SPEC §3.
fn classify_age(age_s: i64) -> Severity {
    if age_s > ALERT_THRESHOLD_S {
        Severity::Alert
    } else if age_s > WARN_THRESHOLD_S {
        Severity::Warn
    } else {
        Severity::Info
    }
}

/// Query the most-recent host-scope sample timestamp.
fn last_host_sample_ts(reader: &JournalReader) -> Result<Option<i64>> {
    reader.last_host_sample_ts()
}

#[cfg(test)]
mod tests {
    use super::*;
    use russell_core::event::Scope;
    use russell_core::journal::JournalWriter;

    fn tmp_journal() -> (tempfile::TempDir, JournalWriter) {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("journal.db");
        let w = JournalWriter::open(&p).unwrap();
        (tmp, w)
    }

    #[test]
    fn first_cycle_no_host_samples_is_info() {
        let (_tmp, w) = tmp_journal();
        let r = w.reader();
        let result = run_once(&w, &r).unwrap();
        assert_eq!(result.severity, Severity::Info);
        assert_eq!(result.age_s, None);
        assert!(!result.event_emitted);
    }

    #[test]
    fn recent_host_sample_is_info() {
        let (_tmp, w) = tmp_journal();
        // Write a host sample "just now".
        let now = russell_core::time::now_unix();
        w.append_sample(now, Scope::Host, "loadavg_1m", Some(1.0), None, None)
            .unwrap();

        let r = w.reader();
        let result = run_once(&w, &r).unwrap();
        assert_eq!(result.severity, Severity::Info);
        assert!(result.age_s.unwrap() < WARN_THRESHOLD_S);
        assert!(!result.event_emitted);
    }

    #[test]
    fn stale_host_sample_is_warn() {
        let (_tmp, w) = tmp_journal();
        let now = russell_core::time::now_unix();
        // Write a host sample 500 s ago (> 450 s threshold).
        w.append_sample(now - 500, Scope::Host, "loadavg_1m", Some(1.0), None, None)
            .unwrap();

        let r = w.reader();
        let result = run_once(&w, &r).unwrap();
        assert_eq!(result.severity, Severity::Warn);
        assert!(result.event_emitted);
    }

    #[test]
    fn very_stale_host_sample_is_alert() {
        let (_tmp, w) = tmp_journal();
        let now = russell_core::time::now_unix();
        // Write a host sample 2000 s ago (> 1800 s threshold).
        w.append_sample(now - 2000, Scope::Host, "loadavg_1m", Some(1.0), None, None)
            .unwrap();

        let r = w.reader();
        let result = run_once(&w, &r).unwrap();
        assert_eq!(result.severity, Severity::Alert);
        assert!(result.event_emitted);
    }

    #[test]
    fn self_sample_is_written() {
        let (_tmp, w) = tmp_journal();
        let now = russell_core::time::now_unix();
        w.append_sample(
            now,
            Scope::Host,
            "mem_available_mib",
            Some(8000.0),
            None,
            Some("MiB"),
        )
        .unwrap();

        let r = w.reader();
        run_once(&w, &r).unwrap();

        // Verify self-scope sample exists.
        let conn = rusqlite::Connection::open_with_flags(
            w.path(),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM samples WHERE scope='self' AND probe=?1",
                rusqlite::params![PROBE_SENTINEL_AGE],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn classify_age_boundaries() {
        assert_eq!(classify_age(0), Severity::Info);
        assert_eq!(classify_age(449), Severity::Info);
        assert_eq!(classify_age(450), Severity::Info); // > not >=
        assert_eq!(classify_age(451), Severity::Warn);
        assert_eq!(classify_age(1800), Severity::Warn); // > not >=
        assert_eq!(classify_age(1801), Severity::Alert);
    }
}

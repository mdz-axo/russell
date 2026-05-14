// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell-proprio` — proprioception: Russell watches Russell.
//!
//! Phase 2 entry point. Implements the five self-vitals defined in
//! [ADR-0021](../../../docs/adr/0021-proprioception-phase2-reflex-arcs.md):
//!
//! | Self-vital                  | Source                              | Warn               | Alert              |
//! |-----------------------------|-------------------------------------|--------------------|--------------------|
//! | `sentinel_last_run_age_s`   | journal `MAX(ts)` on host samples   | > 450 s            | > 1 800 s          |
//! | `journal_writer_stall_s`    | `JournalWriter.last_write_unix_s`   | > 60 s             | > 300 s            |
//! | `llm_p95_latency_ms`        | `help_sessions` p95 in last 24h     | > 8 000 ms         | > 20 000 ms        |
//! | `timer_drift_s`             | `systemctl show` timer              | > 90 s             | > 300 s            |
//! | `help_error_rate_pct`       | `help_sessions` error% in last 24h  | > 20%              | > 50%              |
//!
//! This crate provides one main function, [`run_once`], which:
//!
//! 1. Reads the most-recent host-scope sample timestamp from the journal.
//! 2. Computes the sentinel age and 4 additional self-vitals.
//! 3. Writes a self-scope sample for each vital.
//! 4. If any threshold is breached, emits self-scope events.
//!
//! ## AutoimmuneGuard
//!
//! The [`AutoimmuneGuard`] struct is a process-wide mutex that prevents
//! re-entrant meta-Doctor runs. It is built here (Phase 2A) as a foundation
//! for future meta-Doctor use but is not yet wired into `run_once`.
//!
//! See [ADR-0015](../../../docs/adr/0015-proprioception-self-health.md) and
//! [ADR-0021](../../../docs/adr/0021-proprioception-phase2-reflex-arcs.md).

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms)]
#![warn(missing_docs)]

use std::process::Command;
use std::sync::{Mutex, MutexGuard};

use russell_core::Result;
use russell_core::event::{Event, Scope, Severity};
use russell_core::journal::{JournalReader, JournalWriter};
use tracing::{debug, warn};

// ---------------------------------------------------------------------------
// TimerSource — abstraction over systemd timer queries
// ---------------------------------------------------------------------------

/// Abstracts the query for the sentinel timer's last trigger time.
///
/// The production implementation shells out to `systemctl`; tests use a
/// fixed value so they don't depend on the host's systemd state.
///
/// Returns `Ok(Some(microseconds_since_epoch))` on success,
/// `Ok(None)` if the timer property isn't found, or `Err(...)` on failure.
pub trait TimerSource {
    /// Read `LastTriggerUSec` from the sentinel timer.
    ///
    /// # Errors
    ///
    /// Returns a human-readable error string on subprocess or parse failure.
    fn read_last_trigger_us(&self) -> std::result::Result<Option<u64>, String>;
}

/// The production [`TimerSource`] — queries systemd via `systemctl`.
///
/// This is a driven adapter for the [`TimerSource`] port. The
/// port is a pure trait; this adapter shells out to `systemctl
/// --user show russell-sentinel.timer --property=LastTriggerUSec`
/// for the production implementation.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemdTimerSource;

impl TimerSource for SystemdTimerSource {
    fn read_last_trigger_us(&self) -> std::result::Result<Option<u64>, String> {
        read_timer_last_trigger()
    }
}

// ---------------------------------------------------------------------------
// Probe constants
// ---------------------------------------------------------------------------

/// Probe name for the sentinel age self-vital (MVP, JR-5).
pub const PROBE_SENTINEL_AGE: &str = "sentinel_last_run_age_s";

/// Probe name for the journal writer stall self-vital.
pub const PROBE_JOURNAL_STALL: &str = "journal_writer_stall_s";

/// Probe name for the LLM p95 latency self-vital.
pub const PROBE_LLM_P95_LATENCY: &str = "llm_p95_latency_ms";

/// Probe name for the timer drift self-vital.
pub const PROBE_TIMER_DRIFT: &str = "timer_drift_s";

/// Probe name for the help error rate self-vital.
pub const PROBE_HELP_ERROR_RATE: &str = "help_error_rate_pct";

/// Probe name for the Kask MCP reachability self-vital (Phase 4C, ADR-0025 §5).
pub const PROBE_KASK_MCP_REACHABLE: &str = "kask_mcp_reachable_ms";

// ---------------------------------------------------------------------------
// Sentinel age thresholds
// ---------------------------------------------------------------------------

/// Threshold (seconds) above which the sentinel-age vital emits `warn`.
/// 1.5× the 300 s cadence.
pub const SENTINEL_WARN_THRESHOLD_S: i64 = 450;

/// Threshold (seconds) above which the sentinel-age vital emits `alert`.
/// 6× the 300 s cadence (30 minutes).
pub const SENTINEL_ALERT_THRESHOLD_S: i64 = 1_800;

// ---------------------------------------------------------------------------
// Journal writer stall thresholds
// ---------------------------------------------------------------------------

/// Threshold (seconds) above which the journal-stall vital emits `warn`.
pub const STALL_WARN_THRESHOLD_S: i64 = 60;

/// Threshold (seconds) above which the journal-stall vital emits `alert`.
pub const STALL_ALERT_THRESHOLD_S: i64 = 300;

// ---------------------------------------------------------------------------
// LLM p95 latency thresholds
// ---------------------------------------------------------------------------

/// Threshold (milliseconds) above which the LLM latency vital emits `warn`.
pub const LLM_P95_WARN_THRESHOLD_MS: f64 = 8_000.0;

/// Threshold (milliseconds) above which the LLM latency vital emits `alert`.
pub const LLM_P95_ALERT_THRESHOLD_MS: f64 = 20_000.0;

// ---------------------------------------------------------------------------
// Timer drift thresholds
// ---------------------------------------------------------------------------

/// Threshold (seconds) above which the timer-drift vital emits `warn`.
/// 1.5× the 60 s timer cadence.
pub const DRIFT_WARN_THRESHOLD_S: i64 = 90;

/// Threshold (seconds) above which the timer-drift vital emits `alert`.
pub const DRIFT_ALERT_THRESHOLD_S: i64 = 300;

// ---------------------------------------------------------------------------
// Help error rate thresholds
// ---------------------------------------------------------------------------

/// Threshold (percentage) above which the help error rate vital emits `warn`.
pub const ERROR_RATE_WARN_THRESHOLD_PCT: f64 = 20.0;

/// Threshold (percentage) above which the help error rate vital emits `alert`.
pub const ERROR_RATE_ALERT_THRESHOLD_PCT: f64 = 50.0;

// ---------------------------------------------------------------------------
// Kask MCP reachability thresholds
// ---------------------------------------------------------------------------

/// Threshold (milliseconds) above which the kask MCP latency vital emits `warn`.
/// 2× the 2s health probe timeout.
pub const KASK_LATENCY_WARN_THRESHOLD_MS: u64 = 2_000;

// ---------------------------------------------------------------------------
// AutoimmuneGuard
// ---------------------------------------------------------------------------

/// Process-wide guard preventing re-entrant meta-Doctor runs.
///
/// When held, any attempt to re-enter meta-Doctor should be refused.
/// Built here (Phase 2A) as a foundation; not yet wired into `run_once`.
///
/// Uses [`std::sync::Mutex`] because the current proprioception cycle is
/// synchronous. Can be upgraded to `tokio::sync::Mutex` if needed for async
/// meta-Doctor calls.
#[derive(Debug)]
pub struct AutoimmuneGuard(Mutex<()>);

impl AutoimmuneGuard {
    /// Create a new, unlocked guard.
    #[must_use]
    pub fn new() -> Self {
        Self(Mutex::new(()))
    }

    /// Enter the guard, blocking until it is acquired.
    ///
    /// Returns a standard [`MutexGuard`] that releases the lock on drop.
    ///
    /// # Panics
    ///
    /// Panics if the mutex is poisoned (i.e., a previous holder panicked
    /// while holding the guard).
    pub fn enter(&self) -> MutexGuard<'_, ()> {
        self.0.lock().expect("AutoimmuneGuard mutex poisoned")
    }

    /// Try to enter the guard without blocking.
    ///
    /// Returns `Some(guard)` if acquired, `None` if the guard is already
    /// held by another caller.
    #[must_use]
    pub fn try_enter(&self) -> Option<MutexGuard<'_, ()>> {
        self.0.try_lock().ok()
    }
}

impl Default for AutoimmuneGuard {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ProprioResult
// ---------------------------------------------------------------------------

/// Result of a single proprioception cycle.
#[derive(Debug, Clone, PartialEq)]
pub struct ProprioResult {
    // -- Sentinel age (MVP, JR-5) --
    /// The computed sentinel age in seconds, or `None` if no host samples
    /// exist yet (first-ever cycle).
    pub age_s: Option<i64>,
    /// The severity band the sentinel fell into.
    pub severity: Severity,
    /// Whether any self-vital emitted an event.
    pub event_emitted: bool,

    // -- Journal writer stall (Phase 2A) --
    /// Time since last journal write, in seconds.
    pub journal_stall_s: Option<i64>,
    /// Severity of the journal stall vital.
    pub journal_stall_severity: Severity,

    // -- LLM p95 latency (Phase 2A) --
    /// p95 of LLM latency in last 24h, in milliseconds. `None` if fewer
    /// than 4 data points exist.
    pub llm_p95_latency_ms: Option<f64>,
    /// Severity of the LLM latency vital.
    pub llm_p95_severity: Severity,

    // -- Timer drift (Phase 2A) --
    /// Seconds since the systemd timer last triggered. `None` if systemctl
    /// is unavailable or the timer isn't found.
    pub timer_drift_s: Option<i64>,
    /// Severity of the timer drift vital.
    pub timer_drift_severity: Severity,

    // -- Help error rate (Phase 2A) --
    /// Percentage of help sessions in error/fallback/threshold_skip state,
    /// over the last 24h. `None` if no sessions exist.
    pub help_error_rate_pct: Option<f64>,
    /// Severity of the help error rate vital.
    pub help_error_rate_severity: Severity,

    // -- Kask MCP reachability (Phase 4C, ADR-0025 §5) --
    /// Kask MCP latency in milliseconds, or `None` if the probe was not
    /// run (no kask config) or the endpoint was unreachable.
    pub kask_mcp_reachable_ms: Option<u64>,
    /// Severity of the kask MCP reachability vital.
    pub kask_mcp_reachable_severity: Severity,
}

/// Input from the async Kask MCP health probe, passed into the proprio cycle
/// by the CLI layer (which performs the async HTTP check).
#[derive(Debug, Clone, Copy)]
pub struct KaskHealthInput {
    /// Whether the endpoint responded.
    pub reachable: bool,
    /// Round-trip latency in milliseconds.
    pub latency_ms: Option<u64>,
}

// ---------------------------------------------------------------------------
// run_once
// ---------------------------------------------------------------------------

/// Run the proprioception cycle once.
///
/// Convenience wrapper around [`run_once_inner`] with the real
/// [`SystemdTimerSource`] and no Kask health probe.
///
/// # Errors
///
/// Returns [`russell_core::CoreError`] on journal I/O failures.
pub fn run_once(writer: &JournalWriter, reader: &JournalReader) -> Result<ProprioResult> {
    run_once_inner(writer, reader, &SystemdTimerSource, None)
}

/// Run the proprioception cycle once with a caller-provided [`TimerSource`].
///
/// Reads the journal, computes all five self-vitals, writes self-scope
/// samples, and emits events for any breached thresholds.
///
/// Tests should use [`FixedTimerSource`] to avoid depending on the host's
/// systemd state.
///
/// # Errors
///
/// Returns [`russell_core::CoreError`] on journal I/O failures.
pub fn run_once_with(
    writer: &JournalWriter,
    reader: &JournalReader,
    timer: &dyn TimerSource,
) -> Result<ProprioResult> {
    run_once_inner(writer, reader, timer, None)
}

/// Run the proprioception cycle once with Kask MCP health data.
///
/// In addition to the five standard self-vitals, journals the
/// `kask_mcp_reachable_ms` probe (Phase 4C, ADR-0025 §5).
///
/// The caller (CLI) performs the async HTTP health check, then
/// passes the result here so the synchronous proprio cycle can
/// journal it without depending on async runtime.
///
/// # Errors
///
/// Returns [`russell_core::CoreError`] on journal I/O failures.
pub fn run_once_with_kask(
    writer: &JournalWriter,
    reader: &JournalReader,
    kask_health: KaskHealthInput,
) -> Result<ProprioResult> {
    run_once_inner(writer, reader, &SystemdTimerSource, Some(kask_health))
}

    // 1. Sentinel age (existing MVP vital).
    let last_host_ts = reader.last_host_sample_ts()?;
    let (age_s, sentinel_severity) = match last_host_ts {
        Some(ts) => {
            let age = now.saturating_sub(ts);
            let sev =
                classify_threshold(age, SENTINEL_WARN_THRESHOLD_S, SENTINEL_ALERT_THRESHOLD_S);
            (Some(age), sev)
        }
        None => (None, Severity::Info),
    };
    writer.append_sample(
        now,
        Scope::Self_,
        PROBE_SENTINEL_AGE,
        age_s.map(|a| a as f64),
        None,
        Some("s"),
    )?;
    debug!(age_s = ?age_s, severity = ?sentinel_severity, "proprio: {PROBE_SENTINEL_AGE}");

    // 2. Journal writer stall.
    let (journal_stall_s, stall_severity) = gather_journal_stall(writer, now)?;

    // 3. LLM p95 latency.
    let (llm_p95_latency_ms, llm_p95_severity) = gather_llm_p95_latency(writer, reader, now)?;

    // 4. Timer drift.
    let (timer_drift_s, drift_severity) = gather_timer_drift(writer, now, timer);

    // 5. Help error rate.
    let (help_error_rate_pct, error_rate_severity) = gather_help_error_rate(writer, reader, now)?;

    // Emit events for any vital that breached threshold.
    // Descriptors: (severity, module, probe_name, value_f64, warn_threshold, alert_threshold, json_key)
    let vitals: &[(Severity, &str, &str, f64, f64, f64, &str)] = &[
        (
            sentinel_severity,
            "proprio/sentinel_age",
            PROBE_SENTINEL_AGE,
            age_s.unwrap_or(-1) as f64,
            SENTINEL_WARN_THRESHOLD_S as f64,
            SENTINEL_ALERT_THRESHOLD_S as f64,
            "age_s",
        ),
        (
            stall_severity,
            "proprio/journal_stall",
            PROBE_JOURNAL_STALL,
            journal_stall_s.unwrap_or(-1) as f64,
            STALL_WARN_THRESHOLD_S as f64,
            STALL_ALERT_THRESHOLD_S as f64,
            "stall_s",
        ),
        (
            llm_p95_severity,
            "proprio/llm_latency",
            PROBE_LLM_P95_LATENCY,
            llm_p95_latency_ms.unwrap_or(-1.0),
            LLM_P95_WARN_THRESHOLD_MS,
            LLM_P95_ALERT_THRESHOLD_MS,
            "p95_ms",
        ),
        (
            drift_severity,
            "proprio/timer_drift",
            PROBE_TIMER_DRIFT,
            timer_drift_s.unwrap_or(-1) as f64,
            DRIFT_WARN_THRESHOLD_S as f64,
            DRIFT_ALERT_THRESHOLD_S as f64,
            "drift_s",
        ),
        (
            error_rate_severity,
            "proprio/help_error_rate",
            PROBE_HELP_ERROR_RATE,
            help_error_rate_pct
                .map(|v| (v * 10.0).round() / 10.0)
                .unwrap_or(-1.0),
            ERROR_RATE_WARN_THRESHOLD_PCT,
            ERROR_RATE_ALERT_THRESHOLD_PCT,
            "pct",
        ),
    ];

    let mut event_emitted = false;
    for &(sev, module, probe, value, warn_t, alert_t, json_key) in vitals {
        if sev != Severity::Info {
            let threshold = if matches!(sev, Severity::Alert | Severity::Crit) {
                alert_t
            } else {
                warn_t
            };
            emit_event(
                writer,
                sev,
                module,
                &format!("{probe} = {value} (threshold: {threshold} for {sev:?})"),
                &[(json_key, serde_json::Value::from(value))],
            )?;
            event_emitted = true;
        }
    }

    Ok(ProprioResult {
        age_s,
        severity: sentinel_severity,
        event_emitted,
        journal_stall_s,
        journal_stall_severity: stall_severity,
        llm_p95_latency_ms,
        llm_p95_severity,
        timer_drift_s,
        timer_drift_severity: drift_severity,
        help_error_rate_pct,
        help_error_rate_severity: error_rate_severity,
    })
}

// ---------------------------------------------------------------------------
// Vital gatherers
// ---------------------------------------------------------------------------

/// Classify a numeric value against warn/alert thresholds.
/// Returns the highest severity breached. `>` (not `>=`) per ADR convention.
fn classify_threshold(value: i64, warn: i64, alert: i64) -> Severity {
    if value > alert {
        Severity::Alert
    } else if value > warn {
        Severity::Warn
    } else {
        Severity::Info
    }
}

/// Classify a floating-point value against warn/alert thresholds.
fn classify_threshold_f64(value: f64, warn: f64, alert: f64) -> Severity {
    if value > alert {
        Severity::Alert
    } else if value > warn {
        Severity::Warn
    } else {
        Severity::Info
    }
}

/// Gather the journal writer stall vital.
fn gather_journal_stall(writer: &JournalWriter, now: i64) -> Result<(Option<i64>, Severity)> {
    let last_write = writer.last_write_unix_s();
    let stall_s = now.saturating_sub(last_write);

    writer.append_sample(
        now,
        Scope::Self_,
        PROBE_JOURNAL_STALL,
        Some(stall_s as f64),
        None,
        Some("s"),
    )?;

    let sev = classify_threshold(stall_s, STALL_WARN_THRESHOLD_S, STALL_ALERT_THRESHOLD_S);
    debug!(stall_s, severity = ?sev, "proprio: {PROBE_JOURNAL_STALL}");
    Ok((Some(stall_s), sev))
}

/// Gather the LLM p95 latency vital.
fn gather_llm_p95_latency(
    writer: &JournalWriter,
    reader: &JournalReader,
    now: i64,
) -> Result<(Option<f64>, Severity)> {
    let p95 = reader.llm_latency_p95_ms()?;

    writer.append_sample(
        now,
        Scope::Self_,
        PROBE_LLM_P95_LATENCY,
        p95,
        None,
        Some("ms"),
    )?;

    let sev = match p95 {
        Some(v) => classify_threshold_f64(v, LLM_P95_WARN_THRESHOLD_MS, LLM_P95_ALERT_THRESHOLD_MS),
        None => Severity::Info,
    };
    debug!(p95_ms = ?p95, severity = ?sev, "proprio: {PROBE_LLM_P95_LATENCY}");
    Ok((p95, sev))
}

/// Gather the timer drift vital.
///
/// Uses the provided [`TimerSource`] to query the sentinel timer's last
/// trigger time. Gracefully returns `None` if the query fails or the timer
/// doesn't exist.
fn gather_timer_drift(
    writer: &JournalWriter,
    now: i64,
    timer: &dyn TimerSource,
) -> (Option<i64>, Severity) {
    let drift = match timer.read_last_trigger_us() {
        Ok(Some(trigger_us)) => {
            let trigger_s = (trigger_us / 1_000_000) as i64;
            Some(now.saturating_sub(trigger_s))
        }
        Ok(None) => {
            debug!("proprio: timer source returned None (timer not found)");
            None
        }
        Err(e) => {
            warn!(error = %e, "proprio: failed to read timer, skipping timer_drift_s");
            None
        }
    };

    let sev = match drift {
        Some(d) => classify_threshold(d, DRIFT_WARN_THRESHOLD_S, DRIFT_ALERT_THRESHOLD_S),
        None => Severity::Info,
    };

    // Write sample even when None — records the fact that we tried.
    if let Err(e) = writer.append_sample(
        now,
        Scope::Self_,
        PROBE_TIMER_DRIFT,
        drift.map(|d| d as f64),
        None,
        Some("s"),
    ) {
        warn!(error = %e, "proprio: failed to write timer_drift_s sample");
    }

    debug!(drift_s = ?drift, severity = ?sev, "proprio: {PROBE_TIMER_DRIFT}");
    (drift, sev)
}

/// Read the `LastTriggerUSec` from the Russell sentinel systemd timer.
fn read_timer_last_trigger() -> std::result::Result<Option<u64>, String> {
    // Try LastTriggerUSec first (microseconds since epoch on newer systemd).
    // Fall back to parsing the human-readable timestamp if that's what we get.
    let output = Command::new("systemctl")
        .args([
            "--user",
            "show",
            "russell-sentinel.timer",
            "--property=LastTriggerUSec",
        ])
        .output()
        .map_err(|e| format!("systemctl exec failed: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "systemctl exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.trim();

    // Formats we handle:
    // - "LastTriggerUSec=1746800000000000" (microseconds since epoch, preferred)
    // - "LastTriggerUSec=Sat 2026-05-09 21:55:25 PDT" (human-readable, fallback)
    if let Some(value_str) = line.strip_prefix("LastTriggerUSec=") {
        let value_str = value_str.trim();

        // Try numeric microseconds first.
        if let Ok(us) = value_str.parse::<u64>() {
            return Ok(Some(us));
        }

        // Try parsing as a human-readable timestamp via the `date` command.
        // This is a fallback; it won't produce microsecond precision but
        // it's close enough for a drift check (> 90 s threshold).
        if let Ok(epoch_s) = parse_human_timestamp(value_str) {
            return Ok(Some(epoch_s * 1_000_000));
        }

        return Err(format!(
            "unrecognised LastTriggerUSec format: '{value_str}'"
        ));
    }

    // Property not found — timer may not exist.
    Ok(None)
}

/// Parse a human-readable timestamp like "Sat 2026-05-09 21:55:25 PDT"
/// into Unix seconds. Uses `date -d` as a subprocess (simplest correct
/// parser for arbitrary locale formats).
fn parse_human_timestamp(s: &str) -> std::result::Result<u64, String> {
    let output = Command::new("date")
        .args(["-d", s, "+%s"])
        .output()
        .map_err(|e| format!("date exec failed: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "date exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .trim()
        .parse::<u64>()
        .map_err(|e| format!("invalid epoch seconds from date: {e}"))
}

/// Gather the help error rate vital.
fn gather_help_error_rate(
    writer: &JournalWriter,
    reader: &JournalReader,
    now: i64,
) -> Result<(Option<f64>, Severity)> {
    let rate = reader.help_error_rate_pct()?;

    writer.append_sample(
        now,
        Scope::Self_,
        PROBE_HELP_ERROR_RATE,
        rate,
        None,
        Some("%"),
    )?;

    let sev = match rate {
        Some(v) => classify_threshold_f64(
            v,
            ERROR_RATE_WARN_THRESHOLD_PCT,
            ERROR_RATE_ALERT_THRESHOLD_PCT,
        ),
        None => Severity::Info,
    };
    debug!(rate_pct = ?rate, severity = ?sev, "proprio: {PROBE_HELP_ERROR_RATE}");
    Ok((rate, sev))
}

/// Emit a self-scope event for a threshold breach.
fn emit_event(
    writer: &JournalWriter,
    severity: Severity,
    module: &str,
    summary: &str,
    outputs: &[(&str, serde_json::Value)],
) -> Result<()> {
    let mut ev = Event::new("self_vital_breach", severity);
    ev.scope = Scope::Self_;
    ev.tier = Some("proprio".into());
    ev.module = Some(module.into());
    ev.summary = Some(summary.into());
    for (k, v) in outputs {
        ev.outputs.insert((*k).into(), v.clone());
    }
    writer.append(&ev)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use russell_core::event::Scope;
    use russell_core::journal::{HelpSessionStatus, JournalWriter};

    // -----------------------------------------------------------------------
    // Test TimerSource
    // -----------------------------------------------------------------------

    /// Test [`TimerSource`] that returns a fixed microsecond-since-epoch value.
    ///
    /// Pass `None` to simulate a missing timer (drift = `None`); pass
    /// `Some(us)` to simulate a timer that last triggered at that instant.
    #[derive(Debug, Clone, Copy)]
    struct FixedTimerSource {
        last_trigger_us: Option<u64>,
    }

    impl FixedTimerSource {
        fn new(last_trigger_us: Option<u64>) -> Self {
            Self { last_trigger_us }
        }
    }

    impl TimerSource for FixedTimerSource {
        fn read_last_trigger_us(&self) -> std::result::Result<Option<u64>, String> {
            Ok(self.last_trigger_us)
        }
    }

    /// A [`FixedTimerSource`] that reports no timer found — drift is always
    /// `None`, severity is always `Info`, no event is emitted.
    fn no_timer() -> FixedTimerSource {
        FixedTimerSource::new(None)
    }

    /// A [`FixedTimerSource`] that reports the timer triggered at the given
    /// Unix epoch second.
    fn timer_at(epoch_s: i64) -> FixedTimerSource {
        FixedTimerSource::new(Some(epoch_s as u64 * 1_000_000))
    }

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn tmp_journal() -> (tempfile::TempDir, JournalWriter) {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("journal.db");
        let w = JournalWriter::open(&p).unwrap();
        (tmp, w)
    }

    // -- Sentinel age --

    #[test]
    fn first_cycle_no_host_samples_is_info() {
        let (_tmp, w) = tmp_journal();
        let r = w.reader();
        let result = run_once_with(&w, &r, &no_timer()).unwrap();
        assert_eq!(result.severity, Severity::Info);
        assert_eq!(result.age_s, None);
        assert!(!result.event_emitted);
    }

    #[test]
    fn recent_host_sample_is_info() {
        let (_tmp, w) = tmp_journal();
        let now = russell_core::time::now_unix();
        w.append_sample(now, Scope::Host, "loadavg_1m", Some(1.0), None, None)
            .unwrap();

        let r = w.reader();
        let result = run_once_with(&w, &r, &no_timer()).unwrap();
        assert_eq!(result.severity, Severity::Info);
        assert!(result.age_s.unwrap() < SENTINEL_WARN_THRESHOLD_S);
        assert!(!result.event_emitted);
    }

    #[test]
    fn stale_host_sample_is_warn() {
        let (_tmp, w) = tmp_journal();
        let now = russell_core::time::now_unix();
        w.append_sample(now - 500, Scope::Host, "loadavg_1m", Some(1.0), None, None)
            .unwrap();

        let r = w.reader();
        let result = run_once_with(&w, &r, &no_timer()).unwrap();
        assert_eq!(result.severity, Severity::Warn);
        assert!(result.event_emitted);
    }

    #[test]
    fn very_stale_host_sample_is_alert() {
        let (_tmp, w) = tmp_journal();
        let now = russell_core::time::now_unix();
        w.append_sample(now - 2000, Scope::Host, "loadavg_1m", Some(1.0), None, None)
            .unwrap();

        let r = w.reader();
        let result = run_once_with(&w, &r, &no_timer()).unwrap();
        assert_eq!(result.severity, Severity::Alert);
        assert!(result.event_emitted);
    }

    #[test]
    fn self_samples_are_written() {
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
        run_once_with(&w, &r, &no_timer()).unwrap();

        let conn = rusqlite::Connection::open_with_flags(
            w.path(),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .unwrap();

        // All five self-scope probes should be present.
        for probe in [
            PROBE_SENTINEL_AGE,
            PROBE_JOURNAL_STALL,
            PROBE_LLM_P95_LATENCY,
            PROBE_TIMER_DRIFT,
            PROBE_HELP_ERROR_RATE,
        ] {
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM samples WHERE scope='self' AND probe=?1",
                    rusqlite::params![probe],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            assert_eq!(count, 1, "missing probe: {probe}");
        }
    }

    // -- classify_threshold boundaries --

    #[test]
    fn classify_threshold_boundaries() {
        assert_eq!(classify_threshold(0, 450, 1800), Severity::Info);
        assert_eq!(classify_threshold(450, 450, 1800), Severity::Info); // > not >=
        assert_eq!(classify_threshold(451, 450, 1800), Severity::Warn);
        assert_eq!(classify_threshold(1800, 450, 1800), Severity::Warn); // > not >=
        assert_eq!(classify_threshold(1801, 450, 1800), Severity::Alert);
    }

    #[test]
    fn classify_threshold_f64_boundaries() {
        assert_eq!(classify_threshold_f64(0.0, 20.0, 50.0), Severity::Info);
        assert_eq!(classify_threshold_f64(20.0, 20.0, 50.0), Severity::Info);
        assert_eq!(classify_threshold_f64(20.1, 20.0, 50.0), Severity::Warn);
        assert_eq!(classify_threshold_f64(50.0, 20.0, 50.0), Severity::Warn);
        assert_eq!(classify_threshold_f64(50.1, 20.0, 50.0), Severity::Alert);
    }

    // -- Journal writer stall --

    #[test]
    fn journal_stall_is_low_after_write() {
        let (_tmp, w) = tmp_journal();
        // write something so last_write_unix_s is fresh
        w.append_sample(0, Scope::Host, "test", Some(0.0), None, None)
            .unwrap();
        let r = w.reader();
        let result = run_once_with(&w, &r, &no_timer()).unwrap();
        // Just wrote, so stall should be small
        assert!(result.journal_stall_s.unwrap() < STALL_WARN_THRESHOLD_S);
        assert_eq!(result.journal_stall_severity, Severity::Info);
    }

    // -- LLM p95 latency with no data --

    #[test]
    fn llm_p95_is_none_with_no_sessions() {
        let (_tmp, w) = tmp_journal();
        let r = w.reader();
        let result = run_once_with(&w, &r, &no_timer()).unwrap();
        assert_eq!(result.llm_p95_latency_ms, None);
        assert_eq!(result.llm_p95_severity, Severity::Info);
    }

    // -- Help error rate with no data --

    #[test]
    fn help_error_rate_is_none_with_no_sessions() {
        let (_tmp, w) = tmp_journal();
        let r = w.reader();
        let result = run_once_with(&w, &r, &no_timer()).unwrap();
        assert_eq!(result.help_error_rate_pct, None);
        assert_eq!(result.help_error_rate_severity, Severity::Info);
    }

    // -- LLM p95 latency with sufficient data --

    #[test]
    fn llm_p95_computes_correctly() {
        let (_tmp, w) = tmp_journal();
        let now = russell_core::time::now_unix();
        // Insert 10 sessions with latencies 100, 200, ..., 1000
        for i in 0..10 {
            w.append_help_session_row(
                &format!("id_{i}"),
                now,
                "2026-01-01T00:00:00Z",
                "okapi",
                Some("llama3"),
                None,
                100,
                200,
                Some((i + 1) * 100),
                HelpSessionStatus::Ok,
                None,
                "ev",
            )
            .unwrap();
        }
        let r = w.reader();
        let result = run_once_with(&w, &r, &no_timer()).unwrap();
        // p95 of [100,200,...,1000] with 10 values: idx = floor(0.95*10) = 9, value = 1000
        assert_eq!(result.llm_p95_latency_ms, Some(1000.0));
        assert_eq!(result.llm_p95_severity, Severity::Info);
    }

    #[test]
    fn llm_p95_triggers_warn() {
        let (_tmp, w) = tmp_journal();
        let now = russell_core::time::now_unix();
        // Insert 4 sessions with high latencies to trigger warn (>8000 ms)
        for i in 0..4 {
            w.append_help_session_row(
                &format!("id_{i}"),
                now,
                "2026-01-01T00:00:00Z",
                "okapi",
                Some("llama3"),
                None,
                100,
                200,
                Some(9_000 + i * 100),
                HelpSessionStatus::Ok,
                None,
                "ev",
            )
            .unwrap();
        }
        let r = w.reader();
        let result = run_once_with(&w, &r, &no_timer()).unwrap();
        // p95 of 4 values sorted: [9000,9100,9200,9300], idx=floor(0.95*4)=3, value=9300 > 8000
        assert!(result.llm_p95_latency_ms.unwrap() > LLM_P95_WARN_THRESHOLD_MS);
        assert_eq!(result.llm_p95_severity, Severity::Warn);
    }

    // -- Help error rate with data --

    #[test]
    fn help_error_rate_computes_correctly() {
        let (_tmp, w) = tmp_journal();
        let now = russell_core::time::now_unix();
        // 4 ok, 1 error, 1 fallback, 1 threshold_skip = 7 total, 3 bad = ~42.86%
        let statuses: &[HelpSessionStatus] = &[
            HelpSessionStatus::Ok,
            HelpSessionStatus::Ok,
            HelpSessionStatus::Ok,
            HelpSessionStatus::Ok,
            HelpSessionStatus::Error,
            HelpSessionStatus::Fallback,
            HelpSessionStatus::ThresholdSkip,
        ];
        for (i, status) in statuses.iter().enumerate() {
            w.append_help_session_row(
                &format!("id_{i}"),
                now,
                "2026-01-01T00:00:00Z",
                "okapi",
                Some("llama3"),
                None,
                100,
                200,
                Some(500),
                *status,
                None,
                "ev",
            )
            .unwrap();
        }
        let r = w.reader();
        let result = run_once_with(&w, &r, &no_timer()).unwrap();
        // 3/7 * 100 ≈ 42.86% — triggers warn (>20%) but not alert (<=50%)
        let rate = result.help_error_rate_pct.unwrap();
        assert!(rate > ERROR_RATE_WARN_THRESHOLD_PCT);
        assert!(rate <= ERROR_RATE_ALERT_THRESHOLD_PCT);
        assert_eq!(result.help_error_rate_severity, Severity::Warn);
    }

    #[test]
    fn help_error_rate_triggers_alert() {
        let (_tmp, w) = tmp_journal();
        let now = russell_core::time::now_unix();
        // 2 ok, 3 error = 5 total, 3 bad = 60% — triggers alert
        let statuses: &[HelpSessionStatus] = &[
            HelpSessionStatus::Ok,
            HelpSessionStatus::Ok,
            HelpSessionStatus::Error,
            HelpSessionStatus::Error,
            HelpSessionStatus::Error,
        ];
        for (i, status) in statuses.iter().enumerate() {
            w.append_help_session_row(
                &format!("id_{i}"),
                now,
                "2026-01-01T00:00:00Z",
                "okapi",
                Some("llama3"),
                None,
                100,
                200,
                Some(500),
                *status,
                None,
                "ev",
            )
            .unwrap();
        }
        let r = w.reader();
        let result = run_once_with(&w, &r, &no_timer()).unwrap();
        assert_eq!(result.help_error_rate_severity, Severity::Alert);
    }

    // -- Timer drift --

    #[test]
    fn timer_drift_is_none_when_timer_missing() {
        let (_tmp, w) = tmp_journal();
        let r = w.reader();
        let result = run_once_with(&w, &r, &no_timer()).unwrap();
        assert_eq!(result.timer_drift_s, None);
        assert_eq!(result.timer_drift_severity, Severity::Info);
    }

    #[test]
    fn timer_drift_is_low_when_recent() {
        let (_tmp, w) = tmp_journal();
        let now = russell_core::time::now_unix();
        // Timer triggered 10 seconds ago — well within the 90 s warn threshold.
        let r = w.reader();
        let result = run_once_with(&w, &r, &timer_at(now - 10)).unwrap();
        assert!(result.timer_drift_s.unwrap() < DRIFT_WARN_THRESHOLD_S);
        assert_eq!(result.timer_drift_severity, Severity::Info);
    }

    #[test]
    fn timer_drift_triggers_warn() {
        let (_tmp, w) = tmp_journal();
        let now = russell_core::time::now_unix();
        // Timer triggered 100 seconds ago — above the 90 s warn threshold.
        let r = w.reader();
        let result = run_once_with(&w, &r, &timer_at(now - 100)).unwrap();
        assert!(result.timer_drift_s.unwrap() > DRIFT_WARN_THRESHOLD_S);
        assert_eq!(result.timer_drift_severity, Severity::Warn);
        assert!(result.event_emitted);
    }

    #[test]
    fn timer_drift_triggers_alert() {
        let (_tmp, w) = tmp_journal();
        let now = russell_core::time::now_unix();
        // Timer triggered 400 seconds ago — above the 300 s alert threshold.
        let r = w.reader();
        let result = run_once_with(&w, &r, &timer_at(now - 400)).unwrap();
        assert!(result.timer_drift_s.unwrap() > DRIFT_ALERT_THRESHOLD_S);
        assert_eq!(result.timer_drift_severity, Severity::Alert);
        assert!(result.event_emitted);
    }

    // -- AutoimmuneGuard --

    #[test]
    fn autoimmune_guard_acquire_and_release() {
        let guard = AutoimmuneGuard::new();
        {
            let _g = guard.enter();
            // Should be held now
            assert!(guard.try_enter().is_none());
        }
        // After drop, should be acquirable again
        assert!(guard.try_enter().is_some());
    }

    #[test]
    fn autoimmune_guard_try_enter_returns_none_when_held() {
        let guard = AutoimmuneGuard::new();
        let _g = guard.enter();
        assert!(guard.try_enter().is_none());
    }

    #[test]
    fn autoimmune_guard_default_works() {
        let _guard = AutoimmuneGuard::default();
        // just verify construction
    }
}

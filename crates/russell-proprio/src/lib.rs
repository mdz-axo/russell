// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell-proprio` — Russell proprioception (self-observation).
//!
//! Jack watches Jack. This crate implements the 9-point proprioception
//! check that Russell performs on himself every cycle:
//!
//! 1. `sentinel_last_run_age_s` — how long since the last sentinel cycle?
//! 2. `journal_writer_stall_s` — is the journal writer stalled?
//! 3. `llm_p95_latency_ms` — is LLM inference responding quickly?
//! 4. `timer_drift_s` — is the systemd timer drifting?
//! 5. `help_error_rate_pct` — are help sessions failing too often?
//! 6. `remote_discovery_latency_s` — is remote skill discovery reachable?
//! 7. `journal_chain_intact` — is the journal hash chain intact?
//! 8. `evidence_integrity_ok` — are evidence bundles uncorrupted?
//!
//! Each vital is checked against warn/alert thresholds and optionally
//! journaled as a self-scope sample. Any threshold breach emits a
//! self-scope event.

pub mod reflex;

use russell_core::error::Result;
use russell_core::event::{Event, Scope, Severity};
use russell_core::journal::{JournalWriter, SelfTelemetryPort};
use std::process::Command;
use tracing::{debug, warn};

// ---------------------------------------------------------------------------
// Constants — vital names and thresholds
// ---------------------------------------------------------------------------

/// Vital probe name: sentinel age.
pub const PROBE_SENTINEL_AGE: &str = "sentinel_last_run_age_s";
/// Vital probe name: journal writer stall.
pub const PROBE_JOURNAL_STALL: &str = "journal_writer_stall_s";
/// Vital probe name: LLM p95 latency.
pub const PROBE_LLM_P95_LATENCY: &str = "llm_p95_latency_ms";
/// Vital probe name: timer drift.
pub const PROBE_TIMER_DRIFT: &str = "timer_drift_s";
/// Vital probe name: help error rate.
pub const PROBE_HELP_ERROR_RATE: &str = "help_error_rate_pct";
/// Vital probe name: remote discovery latency.
pub const PROBE_REMOTE_DISCOVERY_LATENCY: &str = "remote_discovery_latency_s";

// ---------------------------------------------------------------------------
// Thresholds (JR-1: austere by default)
// ---------------------------------------------------------------------------

/// Sentinel age warn threshold (seconds). >5 min is unusual; >30 min is alarming.
pub const SENTINEL_WARN_THRESHOLD_S: i64 = 300;
/// Sentinel age alert threshold (seconds).
pub const SENTINEL_ALERT_THRESHOLD_S: i64 = 1800;

/// Journal stall warn threshold (seconds).
pub const STALL_WARN_THRESHOLD_S: i64 = 60;
/// Journal stall alert threshold (seconds).
pub const STALL_ALERT_THRESHOLD_S: i64 = 300;

/// LLM p95 latency warn threshold (ms).
pub const LLM_P95_WARN_THRESHOLD_MS: f64 = 5000.0;
/// LLM p95 latency alert threshold (ms).
pub const LLM_P95_ALERT_THRESHOLD_MS: f64 = 20_000.0;

/// Timer drift warn threshold (seconds).
pub const DRIFT_WARN_THRESHOLD_S: i64 = 90;
/// Timer drift alert threshold (seconds).
pub const DRIFT_ALERT_THRESHOLD_S: i64 = 300;

/// Help error rate warn threshold (percentage).
pub const ERROR_RATE_WARN_THRESHOLD_PCT: f64 = 25.0;
/// Help error rate alert threshold (percentage).
pub const ERROR_RATE_ALERT_THRESHOLD_PCT: f64 = 50.0;

/// Remote discovery latency warn threshold (seconds).
pub const REMOTE_DISCOVERY_WARN_THRESHOLD_S: i64 = 86400;
/// Remote discovery latency alert threshold (seconds).
pub const REMOTE_DISCOVERY_ALERT_THRESHOLD_S: i64 = 259200;

// ---------------------------------------------------------------------------
// Autoimmune guard
// ---------------------------------------------------------------------------

/// Prevents two proprio cycles from running simultaneously.
pub struct AutoimmuneGuard {
    guard: std::sync::atomic::AtomicBool,
}

impl AutoimmuneGuard {
    /// Create a new autoimmune guard.
    #[must_use]
    pub fn new() -> Self {
        Self {
            guard: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// Try to enter the critical section. Returns `true` if this is the
    /// only caller; `false` if another cycle is already running.
    pub fn enter(&self) -> bool {
        self.guard
            .compare_exchange(
                false,
                true,
                std::sync::atomic::Ordering::Acquire,
                std::sync::atomic::Ordering::Relaxed,
            )
            .is_ok()
    }

    /// Try to enter the critical section. Returns `true` on success.
    pub fn try_enter(&self) -> bool {
        self.enter()
    }
}

impl Default for AutoimmuneGuard {
    fn default() -> Self {
        Self::new()
    }
}

static AUTOIMMUNE: AutoimmuneGuard = AutoimmuneGuard {
    guard: std::sync::atomic::AtomicBool::new(false),
};

// ---------------------------------------------------------------------------
// ProprioResult
// ---------------------------------------------------------------------------

/// Result of a proprioception cycle.
///
/// Each field corresponds to one of the 7 numeric self-vitals (plus 2
/// boolean integrity checks). `None` means the vital could not be
/// measured (e.g., no data yet, service unavailable).
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

    // -- Remote discovery latency (Gap 5) --
    /// Time since last successful remote skill registry fetch, in seconds.
    /// `None` if remote discovery is not configured or has never run.
    pub remote_discovery_latency_s: Option<i64>,
    /// Severity of the remote discovery latency vital.
    pub remote_discovery_latency_severity: Severity,

    // -- Journal chain integrity (T6) --
    /// Whether the journal hash chain is intact. `true` if verification
    /// passed (or no chained events exist yet). `false` if a break was
    /// detected. `None` if verification could not run.
    pub journal_chain_intact: Option<bool>,

    // -- Evidence bundle integrity (Task 13) --
    /// Whether the most recent evidence bundles pass hash verification.
    /// `true` if all checked bundles are intact. `false` if any bundle
    /// has a hash mismatch. `None` if no bundles exist to check.
    pub evidence_integrity_ok: Option<bool>,
}

// ---------------------------------------------------------------------------
// run_once
// ---------------------------------------------------------------------------

/// Run the proprioception cycle once.
pub fn run_once(writer: &JournalWriter, reader: &dyn SelfTelemetryPort) -> Result<ProprioResult> {
    let _guard = AUTOIMMUNE.enter();
    run_once_inner(writer, reader, &SystemdTimerSource)
}

/// Run the proprioception cycle once with a caller-provided [`TimerSource`].
pub fn run_once_with(
    writer: &JournalWriter,
    reader: &dyn SelfTelemetryPort,
    timer: &dyn TimerSource,
) -> Result<ProprioResult> {
    let _guard = AUTOIMMUNE.enter();
    run_once_inner(writer, reader, timer)
}

/// Core proprioception logic. Called by [`run_once`] and [`run_once_with`].
fn run_once_inner(
    writer: &JournalWriter,
    reader: &dyn SelfTelemetryPort,
    timer: &dyn TimerSource,
) -> Result<ProprioResult> {
    let now = russell_core::time::now_unix();

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

    // 6. Remote discovery latency (Gap 5).
    let (remote_latency_s, remote_latency_severity) = gather_remote_discovery_latency(writer, now);

    // 7. Journal chain integrity (T6). Quick spot-check: verify only
    //    the last 10 events (full verification is via `russell verify-journal`).
    let journal_chain_intact = check_journal_chain_integrity(reader);

    // Emit events for any vital that breached threshold.
    // Descriptors: (severity, module, probe_name, value_f64, warn_threshold, alert_threshold, json_key)
    let vitals: &[(Severity, &str, &str, f64, f64, f64, &str)] = &[
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
        (
            remote_latency_severity,
            "proprio/remote_discovery",
            PROBE_REMOTE_DISCOVERY_LATENCY,
            remote_latency_s.unwrap_or(-1) as f64,
            REMOTE_DISCOVERY_WARN_THRESHOLD_S as f64,
            REMOTE_DISCOVERY_ALERT_THRESHOLD_S as f64,
            "latency_s",
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
        remote_discovery_latency_s: remote_latency_s,
        remote_discovery_latency_severity: remote_latency_severity,
        journal_chain_intact,
        evidence_integrity_ok: None,
    })
}

// ---------------------------------------------------------------------------
// Vital gatherers
// ---------------------------------------------------------------------------

/// Generic vital metadata for threshold classification.
struct VitalThresholds {
    warn: f64,
    alert: f64,
}

/// Classify a numeric value against warn/alert thresholds.
/// Returns the highest severity band breached. `>` (not `>=`) per ADR convention.
/// Returns the highest severity band breached. `>` (not `>=`) per ADR convention.
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

/// Gather a generic vital that returns an i64 value.
fn gather_i64_vital(
    writer: &JournalWriter,
    now: i64,
    probe_name: &str,
    value: i64,
    thresholds: VitalThresholds,
    unit: &str,
) -> Result<(Option<i64>, Severity)> {
    writer.append_sample(
        now,
        Scope::Self_,
        probe_name,
        Some(value as f64),
        None,
        Some(unit),
    )?;

    let sev = classify_threshold(value, thresholds.warn as i64, thresholds.alert as i64);
    debug!(value, severity = ?sev, "proprio: {probe_name}");
    Ok((Some(value), sev))
}

/// Gather a generic vital that returns an f64 value.
fn gather_f64_vital(
    writer: &JournalWriter,
    now: i64,
    probe_name: &str,
    value: Option<f64>,
    thresholds: VitalThresholds,
    unit: &str,
) -> Result<(Option<f64>, Severity)> {
    writer.append_sample(now, Scope::Self_, probe_name, value, None, Some(unit))?;

    let sev = match value {
        Some(v) => classify_threshold_f64(v, thresholds.warn, thresholds.alert),
        None => Severity::Info,
    };
    debug!(value = ?value, severity = ?sev, "proprio: {probe_name}");
    Ok((value, sev))
}

/// Gather the journal writer stall vital.
fn gather_journal_stall(writer: &JournalWriter, now: i64) -> Result<(Option<i64>, Severity)> {
    let last_write = writer.last_write_unix_s();
    let stall_s = now.saturating_sub(last_write);

    let thresholds = VitalThresholds {
        warn: STALL_WARN_THRESHOLD_S as f64,
        alert: STALL_ALERT_THRESHOLD_S as f64,
    };
    gather_i64_vital(writer, now, PROBE_JOURNAL_STALL, stall_s, thresholds, "s")
}

/// Gather the LLM p95 latency vital.
fn gather_llm_p95_latency(
    writer: &JournalWriter,
    reader: &dyn SelfTelemetryPort,
    now: i64,
) -> Result<(Option<f64>, Severity)> {
    let p95 = reader.llm_latency_p95_ms()?;

    let thresholds = VitalThresholds {
        warn: LLM_P95_WARN_THRESHOLD_MS,
        alert: LLM_P95_ALERT_THRESHOLD_MS,
    };
    gather_f64_vital(writer, now, PROBE_LLM_P95_LATENCY, p95, thresholds, "ms")
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
    reader: &dyn SelfTelemetryPort,
    now: i64,
) -> Result<(Option<f64>, Severity)> {
    let rate = reader.help_error_rate_pct()?;

    let thresholds = VitalThresholds {
        warn: ERROR_RATE_WARN_THRESHOLD_PCT,
        alert: ERROR_RATE_ALERT_THRESHOLD_PCT,
    };
    gather_f64_vital(writer, now, PROBE_HELP_ERROR_RATE, rate, thresholds, "%")
}

/// Gap 5: Gather the remote discovery latency vital.
fn gather_remote_discovery_latency(writer: &JournalWriter, now: i64) -> (Option<i64>, Severity) {
    // Read the last remote discovery fetch event from the journal.
    // For now, this checks if any `remote.skill.fetch` event exists.
    // When remote discovery is wired, this will read from the registry
    // cache's last_fetch_at timestamp.
    let latency = match writer.reader().last_remote_fetch_ts() {
        Ok(Some(last_ts)) => Some(now.saturating_sub(last_ts)),
        // None means no remote discovery has been configured/run.
        Ok(None) => None,
        Err(e) => {
            warn!(error = %e, "proprio: failed to read remote discovery timestamp");
            None
        }
    };

    let sev = match latency {
        Some(d) => classify_threshold(
            d,
            REMOTE_DISCOVERY_WARN_THRESHOLD_S,
            REMOTE_DISCOVERY_ALERT_THRESHOLD_S,
        ),
        None => Severity::Info,
    };

    // Write sample even when None — records the probe ran.
    if let Err(e) = writer.append_sample(
        now,
        Scope::Self_,
        PROBE_REMOTE_DISCOVERY_LATENCY,
        latency.map(|d| d as f64),
        None,
        Some("s"),
    ) {
        warn!(error = %e, "proprio: failed to write {PROBE_REMOTE_DISCOVERY_LATENCY} sample");
    }

    debug!(latency_s = ?latency, severity = ?sev, "proprio: {PROBE_REMOTE_DISCOVERY_LATENCY}");
    (latency, sev)
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
// Journal chain integrity check (T6)
// ---------------------------------------------------------------------------

/// Quick spot-check of the journal hash chain. Verifies the last 10
/// events. Returns:
/// - `Some(true)` if intact or no chained events exist
/// - `Some(false)` if a chain break was detected
/// - `None` if the check could not run (DB error)
fn check_journal_chain_integrity(reader: &dyn SelfTelemetryPort) -> Option<bool> {
    let result = reader.check_chain_integrity();
    if result == Some(false) {
        warn!("journal hash chain BROKEN — tamper evidence detected");
    }
    result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autoimmune_guard_prevents_concurrent() {
        let guard = AutoimmuneGuard::new();
        assert!(guard.enter());
        assert!(!guard.enter()); // Second entry blocked
    }
}

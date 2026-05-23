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
//! re-entrant metacognitive-layer (russell-meta) runs. It is built here (Phase 2A) as a foundation
//! for future self-triage use but is not yet wired into `run_once`.
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
// Reflex Arcs
// ---------------------------------------------------------------------------

pub mod reflex;
pub use reflex::{ReflexAction, ReflexArc, RiskLevel};

// ---------------------------------------------------------------------------
// TimerSource — abstraction over systemd timer queries
// ---------------------------------------------------------------------------

/// Abstracts the query for the sentinel timer's last trigger time.
pub trait TimerSource {
    /// Read `LastTriggerUSec` from the sentinel timer.
    ///
    /// # Errors
    ///
    /// Returns a human-readable error string on subprocess or parse failure.
    fn read_last_trigger_us(&self) -> std::result::Result<Option<u64>, String>;
}

/// The production [`TimerSource`] — queries systemd via `systemctl`.
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

/// Probe name for the HKask MCP reachability self-vital (Phase 4C, ADR-0025 §5).
pub const PROBE_HKASK_MCP_REACHABLE: &str = "hkask_mcp_reachable_ms";

/// Gap 5: Probe name for remote skill discovery latency self-vital.
pub const PROBE_REMOTE_DISCOVERY_LATENCY: &str = "remote_discovery_latency_s";

// ---------------------------------------------------------------------------
// Sentinel age thresholds
// ---------------------------------------------------------------------------

/// Threshold (seconds) above which the sentinel-age vital emits `warn`.
/// 1.5× the 300 s cadence.
/// 1.5× the 300 s cadence.
pub const SENTINEL_WARN_THRESHOLD_S: i64 = 450;

/// Threshold (seconds) above which the sentinel-age vital emits `alert`.
/// 6× the 300 s cadence (30 minutes).
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

/// Threshold (milliseconds) above which the hkask MCP latency vital emits `warn`.
/// 2× the 2s health probe timeout.
/// 2× the 2s health probe timeout.
pub const HKASK_LATENCY_WARN_THRESHOLD_MS: u64 = 2_000;

// ---------------------------------------------------------------------------
// Remote discovery thresholds (Gap 5)
// ---------------------------------------------------------------------------

/// Threshold (seconds) above which the remote discovery latency vital
/// emits `warn`. ~1 hour — stale index.
/// emits `warn`. ~1 hour — stale index.
pub const REMOTE_DISCOVERY_WARN_THRESHOLD_S: i64 = 3_600;

/// Threshold (seconds) above which the remote discovery latency vital
/// emits `alert`. ~24 hours — registry unreachable.
/// emits `alert`. ~24 hours — registry unreachable.
pub const REMOTE_DISCOVERY_ALERT_THRESHOLD_S: i64 = 86_400;

// ---------------------------------------------------------------------------
// AutoimmuneGuard
// ---------------------------------------------------------------------------

/// Process-wide guard preventing re-entrant metacognitive-layer runs.
#[derive(Debug)]
pub struct AutoimmuneGuard(Mutex<()>);

impl AutoimmuneGuard {
    /// Create a new, unlocked guard.
    #[must_use]
    pub fn new() -> Self {
        Self(Mutex::new(()))
    }

    /// Enter the guard, blocking until it is acquired.
    pub fn enter(&self) -> MutexGuard<'_, ()> {
        self.0.lock().expect("AutoimmuneGuard mutex poisoned")
    }

    /// Try to enter the guard without blocking.
    ///
    /// Returns `Some(guard)` if acquired, `None` if the guard is already
    /// held by another caller.
    ///
    /// Returns `Some(guard)` if acquired, `None` if the guard is already
    /// held by another caller.
    /// Returns `Some(guard)` if acquired, `None` if the guard is already
    /// held by another caller.
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

/// The process-wide autoimmune guard — prevents re-entrant metacognitive-layer
/// runs (a Nurse run whose subject is Russell himself). Held for the
/// duration of any proprioception cycle.
/// runs (a Nurse run whose subject is Russell himself). Held for the
/// duration of any proprioception cycle.
/// duration of any proprioception cycle.
static AUTOIMMUNE: std::sync::LazyLock<AutoimmuneGuard> =
    std::sync::LazyLock::new(AutoimmuneGuard::new);

// ---------------------------------------------------------------------------
// ProprioResult
// ---------------------------------------------------------------------------

/// Result of a single proprioception cycle.
#[derive(Debug, Clone, PartialEq)]
pub struct ProprioResult {
    // -- Sentinel age (MVP, JR-5) --
    /// The computed sentinel age in seconds, or `None` if no host samples
    /// exist yet (first-ever cycle).
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
    /// than 4 data points exist.
    pub llm_p95_latency_ms: Option<f64>,
    /// Severity of the LLM latency vital.
    pub llm_p95_severity: Severity,

    // -- Timer drift (Phase 2A) --
    /// Seconds since the systemd timer last triggered. `None` if systemctl
    /// is unavailable or the timer isn't found.
    /// is unavailable or the timer isn't found.
    pub timer_drift_s: Option<i64>,
    /// Severity of the timer drift vital.
    pub timer_drift_severity: Severity,

    // -- Help error rate (Phase 2A) --
    /// Percentage of help sessions in error/fallback/threshold_skip state,
    /// over the last 24h. `None` if no sessions exist.
    /// over the last 24h. `None` if no sessions exist.
    pub help_error_rate_pct: Option<f64>,
    /// Severity of the help error rate vital.
    pub help_error_rate_severity: Severity,

    // -- HKask MCP reachability (Phase 4C, ADR-0025 §5) --
    /// HKask MCP latency in milliseconds, or `None` if the probe was not
    /// run (no hkask config) or the endpoint was unreachable.
    /// run (no hkask config) or the endpoint was unreachable.
    pub hkask_mcp_reachable_ms: Option<u64>,
    /// Severity of the hkask MCP reachability vital.
    pub hkask_mcp_reachable_severity: Severity,

    // -- Remote discovery latency (Gap 5) --
    /// Time since last successful remote skill registry fetch, in seconds.
    /// `None` if remote discovery is not configured or has never run.
    /// `None` if remote discovery is not configured or has never run.
    pub remote_discovery_latency_s: Option<i64>,
    /// Severity of the remote discovery latency vital.
    pub remote_discovery_latency_severity: Severity,

    // -- Journal chain integrity (T6) --
    /// Whether the journal hash chain is intact. `true` if verification
    /// passed (or no chained events exist yet). `false` if a break was
    /// detected. `None` if verification could not run.
    /// passed (or no chained events exist yet). `false` if a break was
    /// detected. `None` if verification could not run.
    /// detected. `None` if verification could not run.
    pub journal_chain_intact: Option<bool>,
}

/// Input from the async HKask MCP health probe, passed into the proprio cycle
/// by the CLI layer (which performs the async HTTP check).
/// by the CLI layer (which performs the async HTTP check).
#[derive(Debug, Clone, Copy)]
pub struct HkaskHealthInput {
    /// Whether the endpoint responded.
    pub reachable: bool,
    /// Round-trip latency in milliseconds.
    pub latency_ms: Option<u64>,
}

// ---------------------------------------------------------------------------
// run_once
// ---------------------------------------------------------------------------

/// Run the proprioception cycle once.
pub fn run_once(writer: &JournalWriter, reader: &JournalReader) -> Result<ProprioResult> {
    let _guard = AUTOIMMUNE.enter();
    run_once_inner(writer, reader, &SystemdTimerSource, None)
}

/// Run the proprioception cycle once with a caller-provided [`TimerSource`].
pub fn run_once_with(
    writer: &JournalWriter,
    reader: &JournalReader,
    timer: &dyn TimerSource,
) -> Result<ProprioResult> {
    let _guard = AUTOIMMUNE.enter();
    run_once_inner(writer, reader, timer, None)
}

/// Run the proprioception cycle once with HKask MCP health data.
pub fn run_once_with_hkask(
    writer: &JournalWriter,
    reader: &JournalReader,
    hkask_health: HkaskHealthInput,
) -> Result<ProprioResult> {
    let _guard = AUTOIMMUNE.enter();
    run_once_inner(writer, reader, &SystemdTimerSource, Some(hkask_health))
}

/// Core proprioception logic. Called by [`run_once`], [`run_once_with`],
/// and [`run_once_with_hkask`].
///
/// When `hkask_health` is `Some`, also gathers and journals the
/// `hkask_mcp_reachable_ms` self-vital.
/// and [`run_once_with_hkask`].
///
/// When `hkask_health` is `Some`, also gathers and journals the
/// `hkask_mcp_reachable_ms` self-vital.
///
/// When `hkask_health` is `Some`, also gathers and journals the
/// `hkask_mcp_reachable_ms` self-vital.
/// When `hkask_health` is `Some`, also gathers and journals the
/// `hkask_mcp_reachable_ms` self-vital.
/// `hkask_mcp_reachable_ms` self-vital.
fn run_once_inner(
    writer: &JournalWriter,
    reader: &JournalReader,
    timer: &dyn TimerSource,
    hkask_health: Option<HkaskHealthInput>,
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

    // 6. HKask MCP reachability (Phase 4C, ADR-0025 §5).
    let (hkask_mcp_ms, hkask_mcp_severity) = gather_hkask_mcp_reachable(writer, now, hkask_health);

    // 7. Remote discovery latency (Gap 5).
    let (remote_latency_s, remote_latency_severity) = gather_remote_discovery_latency(writer, now);

    // 8. Journal chain integrity (T6). Quick spot-check: verify only
    //    the last 10 events (full verification is via `russell verify-journal`).
    let journal_chain_intact = check_journal_chain_integrity(reader);

    // Emit events for any vital that breached threshold.
    // Descriptors: (severity, module, probe_name, value_f64, warn_threshold, alert_threshold, json_key)
    let vitals: &[(Severity, &str, &str, f64, f64, f64, &str)] = &[
        (
            hkask_mcp_severity,
            "proprio/hkask_mcp",
            PROBE_HKASK_MCP_REACHABLE,
            hkask_mcp_ms.map(|v| v as f64).unwrap_or(-1.0),
            HKASK_LATENCY_WARN_THRESHOLD_MS as f64,
            HKASK_LATENCY_WARN_THRESHOLD_MS as f64,
            "latency_ms",
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
        hkask_mcp_reachable_ms: hkask_mcp_ms,
        hkask_mcp_reachable_severity: hkask_mcp_severity,
        remote_discovery_latency_s: remote_latency_s,
        remote_discovery_latency_severity: remote_latency_severity,
        journal_chain_intact,
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
    reader: &JournalReader,
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
///
/// Uses the provided [`TimerSource`] to query the sentinel timer's last
/// trigger time. Gracefully returns `None` if the query fails or the timer
/// doesn't exist.
/// Uses the provided [`TimerSource`] to query the sentinel timer's last
/// trigger time. Gracefully returns `None` if the query fails or the timer
/// doesn't exist.
/// trigger time. Gracefully returns `None` if the query fails or the timer
/// doesn't exist.
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
/// into Unix seconds. Uses `date -d` as a subprocess (simplest correct
/// parser for arbitrary locale formats).
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

    let thresholds = VitalThresholds {
        warn: ERROR_RATE_WARN_THRESHOLD_PCT,
        alert: ERROR_RATE_ALERT_THRESHOLD_PCT,
    };
    gather_f64_vital(writer, now, PROBE_HELP_ERROR_RATE, rate, thresholds, "%")
}

/// Gather the HKask MCP reachability vital (Phase 4C, ADR-0025 §5).
fn gather_hkask_mcp_reachable(
    writer: &JournalWriter,
    now: i64,
    hkask_health: Option<HkaskHealthInput>,
) -> (Option<u64>, Severity) {
    let (hkask_ms, sev) = match hkask_health {
        Some(h) => {
            match (h.reachable, h.latency_ms) {
                (true, Some(ms)) => {
                    let sev = if ms > HKASK_LATENCY_WARN_THRESHOLD_MS {
                        Severity::Warn
                    } else {
                        Severity::Info
                    };
                    (Some(ms), sev)
                }
                (false, _) => {
                    // Unreachable — treat as warn.
                    (None, Severity::Warn)
                }
                (true, None) => {
                    // Reachable but no latency measurement (unusual).
                    (None, Severity::Warn)
                }
            }
        }
        None => {
            // No hkask health probe run — don't journal.
            return (None, Severity::Info);
        }
    };

    // Write sample: value is latency_ms when known, -1 when unreachable.
    let sample_value = hkask_ms.map(|v| v as f64).unwrap_or(-1.0);
    if let Err(e) = writer.append_sample(
        now,
        Scope::Self_,
        PROBE_HKASK_MCP_REACHABLE,
        Some(sample_value),
        None,
        Some("ms"),
    ) {
        warn!(error = %e, "proprio: failed to write {PROBE_HKASK_MCP_REACHABLE} sample");
    }

    debug!(latency_ms = ?hkask_ms, reachable = hkask_health.map(|h| h.reachable), severity = ?sev, "proprio: {PROBE_HKASK_MCP_REACHABLE}");
    (hkask_ms, sev)
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
fn check_journal_chain_integrity(reader: &JournalReader) -> Option<bool> {
    let conn = reader.open_ro_conn().ok()?;
    let mut stmt = conn
        .prepare(
            "SELECT prev_hash, payload, hash FROM events \
             WHERE hash IS NOT NULL \
             ORDER BY ts_unix DESC, id DESC \
             LIMIT 10",
        )
        .ok()?;

    let links: Vec<(String, String, String)> = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .ok()?
        .filter_map(|r| r.ok())
        .collect();

    if links.is_empty() {
        return Some(true); // No chained events yet — not a failure.
    }

    // Reverse to chronological order for verification.
    let links: Vec<_> = links.into_iter().rev().collect();

    match russell_core::hash_chain::verify_chain(&links) {
        russell_core::hash_chain::ChainVerdict::Intact { .. } => Some(true),
        russell_core::hash_chain::ChainVerdict::Empty => Some(true),
        russell_core::hash_chain::ChainVerdict::Broken { .. } => {
            warn!("journal hash chain BROKEN — tamper evidence detected");
            Some(false)
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

// SPDX-License-Identifier: MIT OR Apache-2.0
//! Systemd health probe compositions.
//!
//! Probes the systemd service manager for degradation signals.
//! All probes use controlled subprocess calls to `systemctl`
//! (deterministic, not LLM-generated — JR-3 compliant).

use super::connectors;

/// Probe: is the system in a degraded state?
///
/// Calls `systemctl is-system-running`. Returns `1.0` for
/// degraded/maintenance/unknown states, `0.0` for running/starting,
/// `None` if systemd is unreachable.
pub fn systemd_degraded() -> Option<f64> {
    let output = connectors::run_command_stdout_always(&["systemctl", "is-system-running"])?;
    let state = output.trim();
    Some(match state {
        "degraded" | "maintenance" | "unknown" => 1.0,
        _ => 0.0,
    })
}

/// Probe: count of failed user units.
///
/// Calls `systemctl --user list-units --failed --no-legend`.
/// Returns the count of failed units, or `None` if systemd is
/// unreachable.
pub fn systemd_user_failed_count() -> Option<f64> {
    let output = connectors::run_command_stdout_always(&[
        "systemctl",
        "--user",
        "list-units",
        "--failed",
        "--no-legend",
    ])?;
    let count = output.trim().lines().count();
    Some(count as f64)
}

/// Probe: count of failed system units.
pub fn systemd_system_failed_count() -> Option<f64> {
    let output = connectors::run_command_stdout_always(&[
        "systemctl",
        "list-units",
        "--failed",
        "--no-legend",
    ])?;
    let count = output.trim().lines().count();
    Some(count as f64)
}

pub struct SystemdDegraded;
pub struct SystemdUserFailedCount;
pub struct SystemdSystemFailedCount;

impl_probe!(
    SystemdDegraded,
    "systemd_degraded",
    "bool",
    systemd_degraded
);
impl_probe!(
    SystemdUserFailedCount,
    "systemd_user_failed_count",
    "count",
    systemd_user_failed_count
);
impl_probe!(
    SystemdSystemFailedCount,
    "systemd_system_failed_count",
    "count",
    systemd_system_failed_count
);


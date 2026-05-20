// SPDX-License-Identifier: MIT OR Apache-2.0
//! Tool layer — pure transformation functions.
//!
//! Every function in this module is pure: no I/O, no side effects,
//! deterministic output for a given input. These are independently
//! unit-testable with canned strings.
//!
//! See `docs/specifications/audit-crate.md` Layer 1 for the
//! tool/connector separation discipline.

/// Parse a key from `/proc/meminfo`-formatted text, returning the
/// value in KiB.
///
/// Format: `KeyName:     12345 kB`
pub fn parse_meminfo_kib(content: &str, key: &str) -> Option<u64> {
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix(key) {
            let rest = rest.trim_start_matches(':').trim();
            let first = rest.split_whitespace().next()?;
            return first.parse::<u64>().ok();
        }
    }
    None
}

/// Convert KiB to MiB.
#[inline]
pub fn kib_to_mib(kib: u64) -> f64 {
    kib as f64 / 1024.0
}

/// Parse the 1-minute load average from `/proc/loadavg`-formatted text.
///
/// Format: `0.52 0.34 0.28 1/234 5678`
pub fn parse_loadavg_1m(content: &str) -> Option<f64> {
    content.split_whitespace().next()?.parse::<f64>().ok()
}

/// Parsed fields from `/proc/<pid>/stat`.
#[derive(Debug, Clone, PartialEq)]
pub struct ProcessStat {
    /// Process name (truncated to 15 chars by the kernel).
    pub comm: String,
    /// Process state: R (running), S (sleeping), D (uninterruptible),
    /// Z (zombie), T (stopped), etc.
    pub state: char,
    /// User-mode CPU time in ticks.
    pub utime_ticks: u64,
    /// Kernel-mode CPU time in ticks.
    pub stime_ticks: u64,
    /// Resident set size in pages.
    pub rss_pages: u64,
}

/// Parse `/proc/<pid>/stat` content.
///
/// Format (simplified):
/// ```text
/// 1234 (comm) S 0 0 ... utime stime ... rss ...
/// ```
///
/// Extracts comm (process name), state, utime, stime, and rss.
/// Fields are whitespace-delimited after the closing paren.
pub fn parse_proc_stat(content: &str) -> Option<ProcessStat> {
    let comm_end = content.rfind(')')?;
    let open_paren = content[..comm_end].rfind('(')?;
    let comm = content[open_paren + 1..comm_end].to_string();

    let rest = &content[comm_end + 2..];
    let mut fields = rest.split_whitespace();

    let state = fields.next()?.chars().next()?;

    // Skip 10 fields to reach utime (field 14, 0-indexed 13):
    // ppid, pgrp, session, tty_nr, tpgid, flags, minflt, cminflt, majflt, cmajflt
    for _ in 0..10 {
        fields.next()?;
    }

    let utime_ticks: u64 = fields.next()?.parse().ok()?;
    let stime_ticks: u64 = fields.next()?.parse().ok()?;

    // Skip 8 fields to reach rss (field 24, 0-indexed 23):
    // cutime, cstime, priority, nice, num_threads, itrealvalue, starttime, vsize
    for _ in 0..8 {
        fields.next()?;
    }

    let rss_pages: u64 = fields.next()?.parse().ok()?;

    Some(ProcessStat {
        comm,
        state,
        utime_ticks,
        stime_ticks,
        rss_pages,
    })
}

/// Total CPU ticks (utime + stime) for a process.
#[inline]
pub fn cpu_ticks(s: &ProcessStat) -> u64 {
    s.utime_ticks + s.stime_ticks
}

/// Parse a GPU utilisation percentage from the content of
/// `/sys/class/drm/card*/device/gpu_busy_percent`.
///
/// The file contains a single integer (e.g. `42` meaning 42%).
pub fn parse_gpu_util_pct(content: &str) -> Option<f64> {
    content.trim().parse::<u64>().ok().map(|v| v as f64)
}

/// Convert a millidegree-Celsius value (from `hwmon` `temp1_input`)
/// to degrees Celsius.
///
/// The file contains a raw integer in m°C (e.g. `49000` → 49.0 °C).
pub fn parse_millidegrees_to_c(content: &str) -> Option<f64> {
    let mdeg: i64 = content.trim().parse().ok()?;
    Some(mdeg as f64 / 1000.0)
}

/// Parse the "some" pressure average from `/proc/pressure/io`.
///
/// Format:
/// ```text
/// some avg10=0.00 avg60=0.00 avg300=0.14 total=38929564
/// full avg10=0.00 avg60=0.00 avg300=0.13 total=22896661
/// ```
///
/// Returns the `some avg10` as a percentage (0–100).
pub fn parse_io_pressure_some(content: &str) -> Option<f64> {
    content
        .lines()
        .find(|l| l.starts_with("some "))
        .and_then(parse_pressure_avg10)
}

/// Parse the "full" pressure average from `/proc/pressure/io`.
///
/// Returns the `full avg10` as a percentage (0–100).
pub fn parse_io_pressure_full(content: &str) -> Option<f64> {
    content
        .lines()
        .find(|l| l.starts_with("full "))
        .and_then(parse_pressure_avg10)
}

fn parse_pressure_avg10(line: &str) -> Option<f64> {
    line.split_whitespace()
        .find(|part| part.starts_with("avg10="))
        .and_then(|part| part.strip_prefix("avg10="))
        .and_then(|v| v.parse::<f64>().ok())
}

/// Parse the "some" pressure average from `/proc/pressure/memory`.
pub fn parse_memory_pressure_some(content: &str) -> Option<f64> {
    content
        .lines()
        .find(|l| l.starts_with("some "))
        .and_then(parse_pressure_avg10)
}

/// Parse the "full" pressure average from `/proc/pressure/memory`.
pub fn parse_memory_pressure_full(content: &str) -> Option<f64> {
    content
        .lines()
        .find(|l| l.starts_with("full "))
        .and_then(parse_pressure_avg10)
}

/// Parse a specific key from `/proc/net/sockstat` or `/proc/net/sockstat6`.
/// Format: `TCP: inuse 39 orphan 0 tw 0 alloc 107 mem 686`
///
/// Returns the first numeric value after the label (e.g. `inuse`).
pub fn parse_sockstat(content: &str, key: &str) -> Option<f64> {
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix(key) {
            let rest = rest.trim_start_matches(':').trim();
            let mut parts = rest.split_whitespace();
            parts.next()?; // skip the label (e.g. "inuse")
            return parts.next()?.parse::<u64>().ok().map(|v| v as f64);
        }
    }
    None
}

/// Parse `df -B1 --output=size,used /` output.
///
/// Returns `(total_bytes, used_bytes)` or `None` on parse failure.
///
/// Expected format:
/// ```text
///      1B-blocks        Used
///   4000787030016 966485950464
/// ```
pub fn parse_df_output(content: &str) -> Option<(u64, u64)> {
    let mut lines = content.trim().lines();
    lines.next()?; // skip header
    let data = lines.next()?;
    let mut parts = data.split_whitespace();
    let total: u64 = parts.next()?.parse().ok()?;
    let used: u64 = parts.next()?.parse().ok()?;
    Some((total, used))
}

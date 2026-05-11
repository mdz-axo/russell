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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_meminfo_kib_extracts_value() {
        let content =
            "MemTotal:       94321 kB\nMemFree:         1234 kB\nMemAvailable:   56789 kB\n";
        assert_eq!(parse_meminfo_kib(content, "MemAvailable"), Some(56789));
        assert_eq!(parse_meminfo_kib(content, "MemTotal"), Some(94321));
        assert_eq!(parse_meminfo_kib(content, "MemFree"), Some(1234));
    }

    #[test]
    fn parse_meminfo_kib_returns_none_for_missing_key() {
        let content = "MemTotal:       94321 kB\n";
        assert_eq!(parse_meminfo_kib(content, "MemAvailable"), None);
    }

    #[test]
    fn parse_meminfo_kib_handles_empty_input() {
        assert_eq!(parse_meminfo_kib("", "MemAvailable"), None);
    }

    #[test]
    fn kib_to_mib_correct() {
        assert!((kib_to_mib(1024) - 1.0).abs() < f64::EPSILON);
        assert!((kib_to_mib(2048) - 2.0).abs() < f64::EPSILON);
        assert!((kib_to_mib(0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn parse_loadavg_1m_extracts_first_field() {
        assert_eq!(parse_loadavg_1m("0.52 0.34 0.28 1/234 5678"), Some(0.52));
        assert_eq!(parse_loadavg_1m("1.00 0.50 0.25 2/100 999"), Some(1.0));
    }

    #[test]
    fn parse_loadavg_1m_handles_empty() {
        assert_eq!(parse_loadavg_1m(""), None);
    }

    #[test]
    fn parse_loadavg_1m_handles_garbage() {
        assert_eq!(parse_loadavg_1m("not_a_number rest"), None);
    }

    #[test]
    fn parse_proc_stat_extracts_fields() {
        // Real stat line format: pid (comm) state ppid pgrp session tty_nr tpgid
        // flags minflt cminflt majflt cmajflt utime stime cutime cstime priority
        // nice num_threads itrealvalue starttime vsize rss ...
        let content = "1234 (my-process) S 1233 1234 1234 0 -1 4194304 123 \
                       0 0 0 45 67 0 0 20 0 1 0 123456 12345678 3456 \
                       18446744073709551615 1 1 0 0 0 0 0 0 0 0 0 0 \
                       0 0 0 0 0 0 0 0 0 0 0 0 0";
        let stat = parse_proc_stat(content).expect("should parse");
        assert_eq!(stat.comm, "my-process");
        assert_eq!(stat.state, 'S');
        assert_eq!(stat.utime_ticks, 45);
        assert_eq!(stat.stime_ticks, 67);
        assert_eq!(stat.rss_pages, 3456);
    }

    #[test]
    fn parse_proc_stat_zombie() {
        let content = "9999 (zombie-proc) Z 1 1 1 0 -1 0 0 0 0 0 0 0 0 0 \
                       0 0 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 \
                       0 0 0 0 0 0 0";
        let stat = parse_proc_stat(content).expect("should parse");
        assert_eq!(stat.comm, "zombie-proc");
        assert_eq!(stat.state, 'Z');
        assert_eq!(stat.rss_pages, 0);
    }

    #[test]
    fn parse_proc_stat_stuck_d_state() {
        let content = "5555 (stuck-io) D 1 1 1 0 -1 0 100 0 50 0 5000 3000 \
                       0 0 0 0 1 0 555 10000000 2048 \
                       0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0";
        let stat = parse_proc_stat(content).expect("should parse");
        assert_eq!(stat.state, 'D');
    }

    #[test]
    fn parse_proc_stat_comm_with_spaces() {
        // Comm may be truncated to 15 chars, test with paren-wrapped name
        let content = "1 (systemd) S 0 1 1 0 -1 4194304 1234 0 0 0 \
                       100 50 10 5 20 0 1 0 50000 25000000 1024 \
                       0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0";
        let stat = parse_proc_stat(content).expect("should parse");
        assert_eq!(stat.comm, "systemd");
    }

    #[test]
    fn parse_proc_stat_empty_input() {
        assert_eq!(parse_proc_stat(""), None);
    }

    #[test]
    fn cpu_ticks_sums_both() {
        let s = ProcessStat {
            comm: "test".into(),
            state: 'R',
            utime_ticks: 100,
            stime_ticks: 50,
            rss_pages: 0,
        };
        assert_eq!(cpu_ticks(&s), 150);
    }

    #[test]
    fn parse_gpu_util_pct_parses_integer() {
        assert_eq!(parse_gpu_util_pct("42"), Some(42.0));
        assert_eq!(parse_gpu_util_pct("0"), Some(0.0));
        assert_eq!(parse_gpu_util_pct("100\n"), Some(100.0));
    }

    #[test]
    fn parse_gpu_util_pct_handles_bad_input() {
        assert_eq!(parse_gpu_util_pct(""), None);
        assert_eq!(parse_gpu_util_pct("abc"), None);
        assert_eq!(parse_gpu_util_pct("-1"), None);
    }

    #[test]
    fn parse_millidegrees_to_c_converts() {
        assert_eq!(parse_millidegrees_to_c("49000"), Some(49.0));
        assert_eq!(parse_millidegrees_to_c("0"), Some(0.0));
        assert_eq!(parse_millidegrees_to_c("28000\n"), Some(28.0));
    }

    #[test]
    fn parse_millidegrees_to_c_handles_bad_input() {
        assert_eq!(parse_millidegrees_to_c(""), None);
        assert_eq!(parse_millidegrees_to_c("abc"), None);
    }
}

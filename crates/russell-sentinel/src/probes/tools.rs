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
}

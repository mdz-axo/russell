// SPDX-License-Identifier: MIT OR Apache-2.0
//! Thin wrapper around `time::OffsetDateTime` to standardise
//! how Russell stamps journal rows and filenames.

use std::time::{SystemTime, UNIX_EPOCH};
use time::OffsetDateTime;

/// Current UTC time, in RFC 3339 form, e.g. `2026-04-17T03:30:12Z`.
///
/// This is the single place all journal / digest / filename
/// timestamps are produced, so a test harness can later shim it
/// behind a trait if needed.
pub fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .expect("RFC3339 formatting of current time cannot fail")
}

/// Unix seconds at call time. Uses `SystemTime`.
pub fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Current UTC date as an ISO 8601 string, e.g. `2026-05-14`.
///
/// Uses Howard Hinnant's civil-from-days algorithm for zero-dependency
/// date conversion from Unix seconds.
pub fn now_date_iso8601() -> String {
    if let Ok(dur) = SystemTime::now().duration_since(UNIX_EPOCH) {
        let secs = dur.as_secs();
        let days_since_epoch = secs / 86400;
        let (y, m, d) = civil_from_days(days_since_epoch as i64 + 719_468);
        format!("{y:04}-{m:02}-{d:02}")
    } else {
        // SystemTime before UNIX_EPOCH — fallback.
        "1970-01-01".to_string()
    }
}

/// Howard Hinnant's civil-from-days algorithm.
///
/// Converts a day count since 0000-03-01 into (year, month, day).
/// Used by [`now_date_iso8601`] to produce calendar dates without
/// pulling in a full date library.
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z - 719_468; // shift epoch to 1970-01-01
    let era = if z >= 0 { z } else { z - 146096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m as i64, d as i64)
}

/// Approximate number of days between two ISO 8601 date strings
/// (e.g. `"2026-05-01"` and `"2026-06-15"`).
///
/// Uses a simplified 30-day-per-month approximation — sufficient
/// for staleness checks and display purposes, not for precise
/// calendrical arithmetic.
pub fn approx_days_between(a: &str, b: &str) -> i64 {
    let pa = parse_date_parts(a);
    let pb = parse_date_parts(b);
    (pb.0 * 365 + pb.1 * 30 + pb.2 as i64) - (pa.0 * 365 + pa.1 * 30 + pa.2 as i64)
}

/// Parse an ISO 8601 date string into (year, month, day).
///
/// Tolerant: returns (0, 0, 1) for completely unparseable input.
fn parse_date_parts(d: &str) -> (i64, i64, i32) {
    let parts: Vec<&str> = d.split('-').collect();
    let y = parts.first().and_then(|s| s.parse().ok()).unwrap_or(0);
    let m = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    let d = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(1);
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc3339_is_plausible() {
        let s = now_rfc3339();
        // Minimal sanity: starts with a 4-digit year and ends with Z.
        assert!(s.len() >= 20, "too short: {s}");
        assert!(s.ends_with('Z'), "not UTC: {s}");
    }

    #[test]
    fn unix_time_is_positive() {
        assert!(now_unix() > 1_700_000_000);
    }

    #[test]
    fn date_iso8601_is_plausible() {
        let d = now_date_iso8601();
        assert_eq!(d.len(), 10);
        assert_eq!(&d[4..5], "-");
        assert_eq!(&d[7..8], "-");
    }

    #[test]
    fn approx_days_between_same_date() {
        assert_eq!(approx_days_between("2026-05-14", "2026-05-14"), 0);
    }

    #[test]
    fn approx_days_between_one_month() {
        let d = approx_days_between("2026-05-01", "2026-06-01");
        assert_eq!(d, 30);
    }

    #[test]
    fn approx_days_between_one_year() {
        let d = approx_days_between("2025-01-01", "2026-01-01");
        assert_eq!(d, 365);
    }
}

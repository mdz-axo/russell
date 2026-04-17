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
}

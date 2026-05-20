// SPDX-License-Identifier: MIT OR Apache-2.0
//! Thin wrapper around `time::OffsetDateTime` to standardise
//! how Russell stamps journal rows and filenames.
//!
//! ## Clock trait (T2 — capability injection)
//!
//! The [`Clock`] trait abstracts time acquisition. Production code
//! uses [`SystemClock`]; tests use [`FixedClock`] for deterministic
//! assertions. This eliminates ambient time authority — callers
//! must explicitly pass a clock capability.
//!
//! The free functions [`now_rfc3339`], [`now_unix`], etc. remain
//! for backward compatibility but are implemented in terms of
//! `SystemClock`. New code should prefer the trait.

use std::sync::atomic::{AtomicI64, Ordering};
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

// ─── Clock trait (T2) ────────────────────────────────────────

/// Abstraction over time acquisition.
pub trait Clock: Send + Sync {
    /// Current Unix timestamp (seconds since epoch).
    fn now_unix(&self) -> i64;

    /// Current time as RFC 3339 string (e.g. `2026-05-19T21:15:00Z`).
    fn now_rfc3339(&self) -> String;

    /// Current date as ISO 8601 string (e.g. `2026-05-19`).
    fn now_date_iso8601(&self) -> String;
}

/// Production clock — delegates to the system clock.
///
/// This is the default implementation used by all production code.
/// Zero-sized type; cloning and passing around is free.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_unix(&self) -> i64 {
        now_unix()
    }

    fn now_rfc3339(&self) -> String {
        now_rfc3339()
    }

    fn now_date_iso8601(&self) -> String {
        now_date_iso8601()
    }
}

/// Deterministic clock for testing.
///
/// Returns a fixed timestamp that can be advanced manually.
/// Useful for asserting temporal invariants without flaky
/// real-time dependencies.
#[derive(Debug)]
pub struct FixedClock {
    unix: AtomicI64,
}

impl FixedClock {
    /// Create a fixed clock at the given Unix timestamp.
    #[must_use]
    pub fn new(unix: i64) -> Self {
        Self {
            unix: AtomicI64::new(unix),
        }
    }

    /// Advance the clock by `secs` seconds.
    pub fn advance(&self, secs: i64) {
        self.unix.fetch_add(secs, Ordering::Relaxed);
    }

    /// Set the clock to a specific Unix timestamp.
    pub fn set(&self, unix: i64) {
        self.unix.store(unix, Ordering::Relaxed);
    }
}

impl Clock for FixedClock {
    fn now_unix(&self) -> i64 {
        self.unix.load(Ordering::Relaxed)
    }

    fn now_rfc3339(&self) -> String {
        let ts = self.now_unix();
        OffsetDateTime::from_unix_timestamp(ts)
            .unwrap_or(OffsetDateTime::UNIX_EPOCH)
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_else(|_| "1970-01-01T00:00:00Z".into())
    }

    fn now_date_iso8601(&self) -> String {
        let ts = self.now_unix();
        let dt = OffsetDateTime::from_unix_timestamp(ts).unwrap_or(OffsetDateTime::UNIX_EPOCH);
        format!("{:04}-{:02}-{:02}", dt.year(), dt.month() as u8, dt.day())
    }
}


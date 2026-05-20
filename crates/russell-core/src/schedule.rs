// SPDX-License-Identifier: MIT OR Apache-2.0
//! Proactive model management — time-based scheduling.
//!
//! Reads a TOML schedule file (`~/.config/harness/schedules.toml`)
//! that maps model names to local-time windows.

use serde::Deserialize;

/// Schema tag for versioned schedule files.
pub const SCHEDULE_SCHEMA: &str = "russell.schedule.v1";

/// A single schedule entry mapping a model to a time window.
#[derive(Debug, Clone, Deserialize)]
pub struct Schedule {
    /// LLM model identifier (e.g. `"deepseek-v4-pro"`).
    pub model: String,
    /// Start time in `HH:MM` 24-hour format.
    pub start: String,
    /// End time in `HH:MM` 24-hour format.
    pub end: String,
    /// Days of the week this schedule is active (e.g. `["Mon", "Fri"]`).
    /// Empty means every day.
    #[serde(default)]
    pub days: Vec<String>,
    /// Optional adapter overrides active during this window.
    #[serde(default)]
    pub adapters: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct SchedulesFile {
    #[serde(default)]
    schema: Option<String>,
    #[serde(default)]
    schedule: Vec<Schedule>,
}

/// A collection of time-based model schedules.
///
/// Loaded from `~/.config/harness/schedules.toml`. If the file is
/// missing or malformed, the set is empty (no schedule override).
#[derive(Debug, Clone, Default)]
pub struct ScheduleSet {
    entries: Vec<Schedule>,
}

/// Returns the current UTC hour, minute, and weekday.
///
/// Note: this is UTC, not local time. Schedule windows are evaluated
/// against UTC. If local-time scheduling is needed, a timezone-aware
/// implementation (requiring a new dependency) must replace this.
fn now_utc() -> (u8, u8, time::Weekday) {
    let now = std::time::SystemTime::now();
    let since_epoch = now.duration_since(std::time::UNIX_EPOCH).unwrap();
    let total_secs = since_epoch.as_secs();
    let secs_since_midnight = total_secs % 86400;
    let hour = ((secs_since_midnight / 3600) as u8).min(23);
    let minute = (((secs_since_midnight % 3600) / 60) as u8).min(59);

    let days_since_epoch = total_secs / 86400;
    // Unix epoch (1970-01-01) was a Thursday. Adjust so (days % 7) maps correctly.
    let days = days_since_epoch + 4; // shift epoch-Thursday to Monday-indexed
    let wday = match days % 7 {
        0 => time::Weekday::Monday,
        1 => time::Weekday::Tuesday,
        2 => time::Weekday::Wednesday,
        3 => time::Weekday::Thursday,
        4 => time::Weekday::Friday,
        5 => time::Weekday::Saturday,
        _ => time::Weekday::Sunday,
    };

    (hour, minute, wday)
}

fn parse_time(s: &str) -> Option<(u8, u8)> {
    let mut parts = s.splitn(2, ':');
    let h: u8 = parts.next()?.parse().ok()?;
    let m: u8 = parts.next()?.parse().ok()?;
    if h > 23 || m > 59 {
        return None;
    }
    Some((h, m))
}

fn day_matches(wday: time::Weekday, days: &[String]) -> bool {
    for d in days {
        let m = match d.as_str() {
            "Mon" => wday == time::Weekday::Monday,
            "Tue" => wday == time::Weekday::Tuesday,
            "Wed" => wday == time::Weekday::Wednesday,
            "Thu" => wday == time::Weekday::Thursday,
            "Fri" => wday == time::Weekday::Friday,
            "Sat" => wday == time::Weekday::Saturday,
            "Sun" => wday == time::Weekday::Sunday,
            _ => continue,
        };
        if m {
            return true;
        }
    }
    false
}

impl ScheduleSet {
    /// Create an empty schedule set (no time-based overrides).
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Load schedules from a TOML file at `path`.
    ///
    /// Returns an empty set if the file is missing, malformed, or
    /// carries an unrecognized schema tag.
    pub fn load(path: &std::path::Path) -> Self {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return Self::new(),
        };
        let file: SchedulesFile = match toml::from_str(&content) {
            Ok(f) => f,
            Err(e) => {
                tracing::warn!(error=%e, "malformed schedule");
                return Self::new();
            }
        };
        if let Some(ref s) = file.schema
            && s != SCHEDULE_SCHEMA
        {
            return Self::new();
        }
        Self {
            entries: file.schedule,
        }
    }

    /// Return the schedule entry active at the current UTC time, if any.
    ///
    /// Entries with malformed time strings are skipped (logged at warn
    /// level) rather than aborting the entire lookup.
    pub fn active_now(&self) -> Option<&Schedule> {
        let (now_h, now_m, now_wday) = now_utc();

        for entry in &self.entries {
            let Some((sh, sm)) = parse_time(&entry.start) else {
                tracing::warn!(start = %entry.start, model = %entry.model, "skipping schedule: bad start time");
                continue;
            };
            let Some((eh, em)) = parse_time(&entry.end) else {
                tracing::warn!(end = %entry.end, model = %entry.model, "skipping schedule: bad end time");
                continue;
            };
            let start = (sh, sm);
            let end = (eh, em);
            let now = (now_h, now_m);

            let in_window = if start <= end {
                now >= start && now < end
            } else {
                now >= start || now < end
            };
            if !in_window {
                continue;
            }
            if !entry.days.is_empty() && !day_matches(now_wday, &entry.days) {
                continue;
            }
            return Some(entry);
        }
        None
    }

    /// Number of schedule entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if there are no schedule entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

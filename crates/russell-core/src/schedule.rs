// SPDX-License-Identifier: MIT OR Apache-2.0
//! Proactive model management — time-based scheduling.
//!
//! Reads a TOML schedule file (`~/.config/harness/schedules.toml`)
//! that maps model names to time windows. Each schedule entry defines
//! when a model should be loaded, which days of the week it applies,
//! and optional adapters to load alongside it.
//!
//! Times are parsed as local time (HH:MM, 24-hour) using the system
//! timezone. The `days` field defaults to all days if omitted.

use serde::Deserialize;

/// Schema tag for versioned schedule files.
pub const SCHEDULE_SCHEMA: &str = "russell.schedule.v1";

/// One schedule entry: a model + time window + optional adapters.
#[derive(Debug, Clone, Deserialize)]
pub struct Schedule {
    pub model: String,
    pub start: String,
    pub end: String,
    #[serde(default)]
    pub days: Vec<String>,
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

#[derive(Debug, Clone, Default)]
pub struct ScheduleSet {
    entries: Vec<Schedule>,
}

fn now_local() -> time::OffsetDateTime {
    let unix_ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let offset = unsafe {
        let mut tm: libc::tm = std::mem::zeroed();
        let ts = unix_ts as libc::time_t;
        if libc::localtime_r(&ts, &mut tm).is_null() {
            return time::OffsetDateTime::now_utc();
        }
        tm.tm_gmtoff as i32
    };

    time::UtcOffset::from_whole_seconds(offset)
        .ok()
        .and_then(|o| {
            time::OffsetDateTime::from_unix_timestamp(unix_ts)
                .ok()?
                .checked_to_offset(o)
        })
        .unwrap_or_else(time::OffsetDateTime::now_utc)
}

fn parse_time(s: &str) -> Option<(u8, u8)> {
    let mut parts = s.splitn(2, ':');
    let hour: u8 = parts.next()?.parse().ok()?;
    let minute: u8 = parts.next()?.parse().ok()?;
    if hour > 23 || minute > 59 {
        return None;
    }
    Some((hour, minute))
}

fn day_matches(wday: time::Weekday, days: &[String]) -> bool {
    for d in days {
        let matches = match d.as_str() {
            "Mon" => wday == time::Weekday::Monday,
            "Tue" => wday == time::Weekday::Tuesday,
            "Wed" => wday == time::Weekday::Wednesday,
            "Thu" => wday == time::Weekday::Thursday,
            "Fri" => wday == time::Weekday::Friday,
            "Sat" => wday == time::Weekday::Saturday,
            "Sun" => wday == time::Weekday::Sunday,
            _ => continue,
        };
        if matches {
            return true;
        }
    }
    false
}

impl ScheduleSet {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    pub fn load(path: &std::path::Path) -> Self {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                if e.kind() != std::io::ErrorKind::NotFound {
                    tracing::warn!(path = %path.display(), error = %e, "cannot read schedule file");
                }
                return Self::new();
            }
        };

        let file: SchedulesFile = match toml::from_str(&content) {
            Ok(f) => f,
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "malformed schedule file");
                return Self::new();
            }
        };

        if let Some(ref schema) = file.schema
            && schema != SCHEDULE_SCHEMA
        {
            tracing::warn!(
                path = %path.display(),
                expected = SCHEDULE_SCHEMA,
                found = %schema,
                "unknown schedule schema",
            );
            return Self::new();
        }

        Self {
            entries: file.schedule,
        }
    }

    pub fn active_now(&self) -> Option<&Schedule> {
        let now = now_local();
        let now_time = (now.hour(), now.minute());
        let now_wday = now.weekday();

        for entry in &self.entries {
            let start = parse_time(&entry.start)?;
            let end = parse_time(&entry.end)?;

            let in_window = if start <= end {
                now_time >= start && now_time < end
            } else {
                now_time >= start || now_time < end
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

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_times() {
        assert_eq!(parse_time("08:00"), Some((8, 0)));
        assert_eq!(parse_time("23:59"), Some((23, 59)));
    }

    #[test]
    fn parse_invalid_times() {
        assert_eq!(parse_time("24:00"), None);
        assert_eq!(parse_time("08:60"), None);
        assert_eq!(parse_time(""), None);
    }

    #[test]
    fn day_matches_works() {
        assert!(day_matches(time::Weekday::Monday, &["Mon".into()]));
        assert!(!day_matches(time::Weekday::Tuesday, &["Mon".into()]));
        assert!(day_matches(
            time::Weekday::Friday,
            &["Mon".into(), "Fri".into()]
        ));
    }
}
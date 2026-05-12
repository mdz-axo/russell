// SPDX-License-Identifier: MIT OR Apache-2.0
//! Proactive model management — time-based scheduling.
//!
//! Reads a TOML schedule file (`~/.config/harness/schedules.toml`)
//! that maps model names to local-time windows.

use serde::Deserialize;

/// Schema tag for versioned schedule files.
pub const SCHEDULE_SCHEMA: &str = "russell.schedule.v1";

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

fn now_local() -> (u8, u8, time::Weekday) {
    let now = std::time::SystemTime::now();
    let since_epoch = now.duration_since(std::time::UNIX_EPOCH).unwrap();
    let total_secs = since_epoch.as_secs();
    let secs_since_midnight = total_secs % 86400;
    let hour = ((secs_since_midnight / 3600) as u8).min(23);
    let minute = (((secs_since_midnight % 3600) / 60) as u8).min(59);

    let days_since_epoch = total_secs / 86400;
    let wday = match days_since_epoch % 7 {
        3 => time::Weekday::Thursday,
        4 => time::Weekday::Friday,
        5 => time::Weekday::Saturday,
        6 => time::Weekday::Sunday,
        0 => time::Weekday::Monday,
        1 => time::Weekday::Tuesday,
        _ => time::Weekday::Wednesday,
    };

    (hour, minute, wday)
}

fn parse_time(s: &str) -> Option<(u8, u8)> {
    let mut parts = s.splitn(2, ':');
    let h: u8 = parts.next()?.parse().ok()?;
    let m: u8 = parts.next()?.parse().ok()?;
    if h > 23 || m > 59 { return None; }
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
        if m { return true; }
    }
    false
}

impl ScheduleSet {
    pub fn new() -> Self { Self { entries: Vec::new() } }

    pub fn load(path: &std::path::Path) -> Self {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return Self::new(),
        };
        let file: SchedulesFile = match toml::from_str(&content) {
            Ok(f) => f,
            Err(e) => { tracing::warn!(error=%e, "malformed schedule"); return Self::new(); }
        };
        if let Some(ref s) = file.schema && s != SCHEDULE_SCHEMA {
            return Self::new();
        }
        Self { entries: file.schedule }
    }

    pub fn active_now(&self) -> Option<&Schedule> {
        let (now_h, now_m, now_wday) = now_local();

        for entry in &self.entries {
            let (sh, sm) = parse_time(&entry.start)?;
            let (eh, em) = parse_time(&entry.end)?;
            let start = (sh, sm);
            let end = (eh, em);
            let now = (now_h, now_m);

            let in_window = if start <= end {
                now >= start && now < end
            } else {
                now >= start || now < end
            };
            if !in_window { continue; }
            if !entry.days.is_empty() && !day_matches(now_wday, &entry.days) { continue; }
            return Some(entry);
        }
        None
    }

    pub fn len(&self) -> usize { self.entries.len() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_works() {
        assert_eq!(parse_time("08:00"), Some((8,0)));
        assert_eq!(parse_time("23:59"), Some((23,59)));
        assert_eq!(parse_time("24:00"), None);
        assert_eq!(parse_time(""), None);
    }

    #[test]
    fn day_checks() {
        assert!(day_matches(time::Weekday::Monday, &["Mon".into()]));
        assert!(!day_matches(time::Weekday::Tuesday, &["Mon".into()]));
        assert!(day_matches(time::Weekday::Friday, &["Mon".into(),"Fri".into()]));
    }

    #[test]
    fn empty_set_no_active() {
        assert!(ScheduleSet::new().active_now().is_none());
    }
}
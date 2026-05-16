// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell-sentinel` — host probe collection and rule evaluation.
//!
//! **TOGAF Phase:** Phase G (Implementation Governance) — the Sentinel
//! observes the host on a 5-minute cadence and writes samples into the
//! SQLite journal, fulfilling the "observe" tier of JR-2.
//!
//! Collects host probes and evaluates them against the rule engine.
//! Thresholds are configurable via `rules.d/*.toml`.
//!
//! The sentinel is the "Observe" phase from the Observe > Recommend >
//! Act posture (JR-2, [`PRINCIPLES_CATALOG.md`](../../docs/architecture/PRINCIPLES_CATALOG.md)).
//! It reads the host; it does not mutate.

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms)]
#![warn(missing_docs)]

pub mod probes;

use probes::Sample;
use russell_core::Result;
use russell_core::RuleSet;
use russell_core::event::{Event, Scope, Severity};
use russell_core::journal::{JournalReader, JournalWriter};

/// Run the probe set once and append samples to the journal.
/// No rule evaluation — samples only.
pub fn run_once(writer: &JournalWriter) -> Result<usize> {
    let samples = probes::collect();
    journal_samples(writer, &samples)?;
    Ok(samples.len())
}

/// Run the probe set with rule evaluation.
///
/// Evaluates each numeric sample against the [`RuleSet`] and emits
/// threshold-breach events for any severity above `Info`.
///
/// When `reader` is `Some`, also evaluates rate-of-change thresholds
/// by comparing against the previous sample in the journal.
///
/// Returns (sample count, threshold breach events). The caller is
/// responsible for journaling the events — this function only
/// writes samples, not events, preserving the ability to annotate
/// events with cycle metadata before persistence.
pub fn run_once_with_rules(
    writer: &JournalWriter,
    rules: &RuleSet,
    reader: Option<&JournalReader>,
) -> Result<(usize, Vec<Event>)> {
    let samples = probes::collect();
    journal_samples(writer, &samples)?;
    let events = if let Some(r) = reader {
        evaluate_samples_with_rates(rules, &samples, r)
    } else {
        evaluate_samples_basic(rules, &samples)
    };
    Ok((samples.len(), events))
}

/// Write samples to the journal in one batch.
///
/// All samples share the same timestamp (the start of this cycle).
fn journal_samples(writer: &JournalWriter, samples: &[Sample]) -> Result<()> {
    let ts = russell_core::time::now_unix();
    for s in samples {
        writer.append_sample(
            ts,
            Scope::Host,
            &s.name,
            s.value_num,
            s.value_text.as_deref(),
            s.unit,
        )?;
    }
    Ok(())
}

/// Evaluate all numeric samples against the rule set (absolute thresholds only).
///
/// This is a pure function — no I/O, no journal writes. Returns only
/// the breach events; the caller is responsible for journaling them.
///
/// Samples without a numeric value (text probes) are silently skipped.
pub fn evaluate_samples_basic(rules: &RuleSet, samples: &[Sample]) -> Vec<Event> {
    let mut events = Vec::new();
    for s in samples {
        if let Some(v) = s.value_num {
            let sev = rules.evaluate(&s.name, v);
            if sev != Severity::Info {
                events.push(build_breach_event(&s.name, v, s.unit, sev));
            }
        }
    }
    events
}

/// Evaluate all numeric samples against the rule set AND rate-of-change
/// thresholds. Rate is computed from the previous sample in the journal.
///
/// Returns breach events for both absolute and rate thresholds.
/// Rate events use `action = "rate_breach"` to distinguish from
/// absolute threshold breaches.
pub fn evaluate_samples_with_rates(
    rules: &RuleSet,
    samples: &[Sample],
    reader: &JournalReader,
) -> Vec<Event> {
    let now = russell_core::time::now_unix();

    let mut events = Vec::new();
    for s in samples {
        let Some(v) = s.value_num else {
            continue;
        };

        // Absolute threshold check.
        let sev_abs = rules.evaluate(&s.name, v);
        if sev_abs != Severity::Info {
            events.push(build_breach_event(&s.name, v, s.unit, sev_abs));
        }

        // Rate-of-change check: compare against previous sample.
        if let Some((prev_val, prev_ts)) = reader.previous_sample(&s.name, now) {
            let dt = ((now - prev_ts).max(1)) as f64;
            let rate = (v - prev_val).abs() / dt;
            let sev_rate = rules.evaluate_rate(&s.name, rate);
            if sev_rate != Severity::Info {
                let mut ev = Event::new("rate_breach", sev_rate);
                ev.tier = Some("sentinel".into());
                ev.module = Some(format!("sentinel/rate/{}", s.name));
                ev.summary = Some(format!(
                    "{} rate {:.4}/s breached threshold ({sev_rate:?})",
                    s.name, rate
                ));
                ev.outputs.insert("probe".into(), s.name.clone().into());
                ev.outputs.insert("value".into(), v.into());
                ev.outputs.insert("rate".into(), rate.into());
                if let Some(u) = s.unit {
                    ev.outputs.insert("unit".into(), u.into());
                }
                events.push(ev);
            }
        }
    }
    events
}

/// Build a single threshold-breach event.
fn build_breach_event(
    probe_name: &str,
    value: f64,
    unit: Option<&'static str>,
    severity: Severity,
) -> Event {
    let mut ev = Event::new("threshold_breach", severity);
    ev.tier = Some("sentinel".into());
    ev.module = Some(format!("sentinel/threshold/{probe_name}"));
    ev.summary = Some(format!(
        "{probe_name} = {value:.2} breached threshold ({severity:?})"
    ));
    ev.outputs.insert("probe".into(), probe_name.into());
    ev.outputs.insert("value".into(), value.into());
    if let Some(u) = unit {
        ev.outputs.insert("unit".into(), u.into());
    }
    ev
}

/// Evaluate externally-written scenario metrics against the rule set.
///
/// Reads all samples from the journal written in the last `window_seconds`,
/// evaluates each numeric sample against the [`RuleSet`], and returns
/// threshold-breach events. This catches samples written by the
/// scenario-tester skill (e.g. `okapi_latency_p95_ms`) that the sentinel
/// doesn't collect through its own probe registry.
///
/// Duplicate breaches (same probe name + value) are de-duplicated by
/// merging with the existing breach events from the sentinel's own
/// probe collection.
pub fn evaluate_scenario_samples(
    reader: &JournalReader,
    rules: &RuleSet,
    window_seconds: i64,
    existing_events: &[Event],
) -> Vec<Event> {
    let now = russell_core::time::now_unix();
    let since = now - window_seconds;

    let samples = match reader.host_samples_summary(since, now) {
        Ok(s) => s,
        Err(_) => {
            tracing::debug!("cannot read scenario samples for rule evaluation");
            return Vec::new();
        }
    };

    let existing_probes: std::collections::BTreeSet<&str> = existing_events
        .iter()
        .filter_map(|ev| ev.outputs.get("probe").and_then(|v| v.as_str()))
        .collect();

    let mut events = Vec::new();
    for s in &samples {
        if existing_probes.contains(s.probe.as_str()) {
            continue;
        }
        let Some(value) = s.last else {
            continue;
        };
        let sev = rules.evaluate(&s.probe, value);
        if sev != Severity::Info {
            tracing::info!(
                probe = %s.probe, value, ?sev,
                "scenario metric breach detected"
            );
            events.push(build_breach_event(&s.probe, value, None, sev));
        }
    }
    events
}

#[cfg(test)]
mod tests {
    use super::*;
    use russell_core::RuleSet;

    #[test]
    fn evaluate_samples_returns_empty_for_info_values() {
        let rs = RuleSet::with_defaults();
        let samples = vec![Sample {
            name: "mem_available_mib".into(),
            value_num: Some(8192.0), // above warn threshold → Info
            value_text: None,
            unit: Some("MiB"),
        }];
        let events = evaluate_samples_basic(&rs, &samples);
        assert!(events.is_empty());
    }

    #[test]
    fn evaluate_samples_returns_breach_for_warn_values() {
        let rs = RuleSet::with_defaults();
        let samples = vec![Sample {
            name: "mem_available_mib".into(),
            value_num: Some(3000.0), // below warn threshold (4096) → Warn
            value_text: None,
            unit: Some("MiB"),
        }];
        let events = evaluate_samples_basic(&rs, &samples);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].severity, Severity::Warn);
        assert_eq!(events[0].action, "threshold_breach");
        assert!(events[0].summary.as_deref().unwrap().contains("3000"));
    }

    #[test]
    fn evaluate_samples_skips_text_probes() {
        let rs = RuleSet::with_defaults();
        let samples = vec![Sample {
            name: "proc_top_cpu_name".into(),
            value_num: None, // text probe — no numeric value
            value_text: Some("systemd".into()),
            unit: None,
        }];
        let events = evaluate_samples_basic(&rs, &samples);
        assert!(events.is_empty());
    }

    #[test]
    fn evaluate_samples_returns_multiple_breaches() {
        let rs = RuleSet::with_defaults();
        let samples = vec![
            Sample {
                name: "mem_available_mib".into(),
                value_num: Some(500.0), // below crit (1024) → Crit
                value_text: None,
                unit: Some("MiB"),
            },
            Sample {
                name: "swap_used_mib".into(),
                value_num: Some(7500.0), // above warn (7168) → Warn
                value_text: None,
                unit: Some("MiB"),
            },
        ];
        let events = evaluate_samples_basic(&rs, &samples);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].severity, Severity::Crit);
        assert_eq!(events[1].severity, Severity::Warn);
    }

    #[test]
    fn build_breach_event_has_expected_fields() {
        let ev = build_breach_event("loadavg_1m", 25.0, None, Severity::Alert);
        assert_eq!(ev.action, "threshold_breach");
        assert_eq!(ev.severity, Severity::Alert);
        assert_eq!(ev.tier.as_deref(), Some("sentinel"));
        assert!(ev.module.as_deref().unwrap().contains("loadavg_1m"));
        assert_eq!(
            ev.outputs.get("probe").and_then(|v| v.as_str()),
            Some("loadavg_1m")
        );
        assert!(!ev.outputs.contains_key("unit"));
    }
}

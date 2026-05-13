// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell-sentinel` — host probe collection and rule evaluation.
//!
//! Collects host probes and evaluates them against the rule engine.
//! Thresholds are configurable via `rules.d/*.toml` (ADR-0012).

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms)]
#![warn(missing_docs)]

pub mod probes;

use russell_core::Result;
use russell_core::RuleSet;
use russell_core::event::{Event, Scope, Severity};
use russell_core::journal::JournalWriter;
use probes::Sample;

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
/// Returns (sample count, threshold breach events).
pub fn run_once_with_rules(writer: &JournalWriter, rules: &RuleSet) -> Result<(usize, Vec<Event>)> {
    let samples = probes::collect();
    journal_samples(writer, &samples)?;
    let events = evaluate_samples(rules, &samples);
    for ev in &events {
        writer.append(ev)?;
    }
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

/// Evaluate all numeric samples against the rule set.
///
/// This is a pure function — no I/O, no journal writes. Returns only
/// the breach events; the caller is responsible for journaling them.
///
/// Samples without a numeric value (text probes) are silently skipped.
pub fn evaluate_samples(rules: &RuleSet, samples: &[Sample]) -> Vec<Event> {
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
        let events = evaluate_samples(&rs, &samples);
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
        let events = evaluate_samples(&rs, &samples);
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
        let events = evaluate_samples(&rs, &samples);
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
        let events = evaluate_samples(&rs, &samples);
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
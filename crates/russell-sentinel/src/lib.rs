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

/// Macro to generate ProbeDescriptor impls, reducing boilerplate.
///
/// Usage: `impl_probe!(StructName, "probe_name", "unit", function_name);`
///
/// For unitless probes: `impl_probe!(StructName, "probe_name", none, function_name);`
#[macro_export]
macro_rules! impl_probe {
    ($struct_name:ident, $name:literal, "unit", $func:ident) => {
        impl $crate::probes::descriptor::ProbeMetadata for $struct_name {
            fn name(&self) -> &'static str {
                $name
            }
            fn unit(&self) -> Option<&'static str> {
                Some("unit")
            }
        }
        impl $crate::probes::descriptor::ProbeCollector for $struct_name {
            fn collect(&self) -> Option<f64> {
                $func()
            }
        }
    };
    ($struct_name:ident, $name:literal, $unit:literal, $func:ident) => {
        impl $crate::probes::descriptor::ProbeMetadata for $struct_name {
            fn name(&self) -> &'static str {
                $name
            }
            fn unit(&self) -> Option<&'static str> {
                Some($unit)
            }
        }
        impl $crate::probes::descriptor::ProbeCollector for $struct_name {
            fn collect(&self) -> Option<f64> {
                $func()
            }
        }
    };
    ($struct_name:ident, $name:literal, none, $func:ident) => {
        impl $crate::probes::descriptor::ProbeMetadata for $struct_name {
            fn name(&self) -> &'static str {
                $name
            }
            fn unit(&self) -> Option<&'static str> {
                None
            }
        }
        impl $crate::probes::descriptor::ProbeCollector for $struct_name {
            fn collect(&self) -> Option<f64> {
                $func()
            }
        }
    };
}

pub mod probes;

use probes::Sample;
use russell_core::Result;
use russell_core::RuleSet;
use russell_core::event::{Event, Scope, Severity};
use russell_core::journal::port::JournalWritePort;
use russell_core::journal::{JournalReader, JournalWriter};
use russell_core::time::Clock;

/// Run the probe set once and append samples to the journal.
/// No rule evaluation — samples only.
///
/// Uses the default probe registry (singleton). For dependency
/// injection, use [`run_once_with_registry`].
pub fn run_once(writer: &JournalWriter) -> Result<usize> {
    let samples = probes::collect();
    journal_samples(writer, &samples)?;
    Ok(samples.len())
}

/// Run the probe set once using an explicitly-provided registry.
///
/// This is the **capability-injected** version of [`run_once`],
/// enabling tests and custom configurations to supply their own
/// probe set without relying on the global singleton.
pub fn run_once_with_registry(
    writer: &JournalWriter,
    registry: &probes::ProbeRegistry,
) -> Result<usize> {
    let samples = probes::collect_with(registry);
    journal_samples(writer, &samples)?;
    Ok(samples.len())
}

/// Run the probe set with rule evaluation.
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

/// Run the probe set with rule evaluation using an injected registry.
///
/// Combines [`run_once_with_registry`] and rule evaluation for
/// callers who need both dependency injection and threshold checking.
pub fn run_once_with_rules_and_registry(
    writer: &JournalWriter,
    rules: &RuleSet,
    reader: Option<&JournalReader>,
    registry: &probes::ProbeRegistry,
) -> Result<(usize, Vec<Event>)> {
    let samples = probes::collect_with(registry);
    journal_samples(writer, &samples)?;
    let events = if let Some(r) = reader {
        evaluate_samples_with_rates(rules, &samples, r)
    } else {
        evaluate_samples_basic(rules, &samples)
    };
    Ok((samples.len(), events))
}

/// Fully-injectable sentinel cycle: custom probe registry, custom
/// journal port, custom clock. This is the "pure hexagonal" entry
/// point for testing — all dependencies are explicit capabilities.
///
/// Returns (sample_count, threshold_events).
pub fn run_once_injectable(
    writer: &dyn JournalWritePort,
    clock: &dyn Clock,
    rules: &RuleSet,
    registry: &probes::ProbeRegistry,
) -> Result<(usize, Vec<Event>)> {
    let samples = probes::collect_with(registry);
    journal_samples_via_port(writer, clock, &samples)?;
    let events = evaluate_samples_basic(rules, &samples);
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

/// Port-based sample writing (T1/T2 — injectable journal + clock).
fn journal_samples_via_port(
    writer: &dyn JournalWritePort,
    clock: &dyn Clock,
    samples: &[Sample],
) -> Result<()> {
    let ts = clock.now_unix();
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

    // --- Integration test: fully-injectable sentinel cycle (T1/T2/T8/T13) ---

    #[test]
    fn injectable_sentinel_cycle_with_test_doubles() {
        use russell_core::event::Scope;
        use russell_core::time::FixedClock;
        use std::sync::Mutex;

        // In-memory journal (test double).
        struct TestJournal {
            samples: Mutex<Vec<(i64, String, Option<f64>)>>,
        }
        impl russell_core::journal::port::JournalWritePort for TestJournal {
            fn append(&self, _event: &Event) -> russell_core::Result<()> {
                Ok(())
            }
            fn append_sample(
                &self,
                ts: i64,
                _scope: Scope,
                probe: &str,
                value_num: Option<f64>,
                _value_text: Option<&str>,
                _unit: Option<&str>,
            ) -> russell_core::Result<()> {
                self.samples
                    .lock()
                    .unwrap()
                    .push((ts, probe.to_string(), value_num));
                Ok(())
            }
        }

        let journal = TestJournal {
            samples: Mutex::new(Vec::new()),
        };
        let clock = FixedClock::new(1_700_000_000);
        let rules = RuleSet::with_defaults();
        let registry = probes::ProbeRegistry::with_defaults();

        let (count, _events) = run_once_injectable(&journal, &clock, &rules, &registry).unwrap();

        // All samples should have the fixed timestamp.
        let samples = journal.samples.lock().unwrap();
        assert!(count > 0);
        assert!(!samples.is_empty());
        for (ts, _, _) in samples.iter() {
            assert_eq!(
                *ts, 1_700_000_000,
                "all samples should use the injected clock"
            );
        }
    }
}

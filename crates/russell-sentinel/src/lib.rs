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

/// Run the probe set once and append samples to the journal.
/// No rule evaluation — samples only.
pub fn run_once(writer: &JournalWriter) -> Result<usize> {
    let ts = russell_core::time::now_unix();
    let samples = probes::collect();
    for s in &samples {
        writer.append_sample(
            ts,
            Scope::Host,
            &s.name,
            s.value_num,
            s.value_text.as_deref(),
            s.unit,
        )?;
    }
    Ok(samples.len())
}

/// Run the probe set with rule evaluation.
///
/// Evaluates each numeric sample against the [`RuleSet`] and emits
/// threshold-breach events for any severity above `Info`.
///
/// Returns (sample count, threshold breach events).
pub fn run_once_with_rules(writer: &JournalWriter, rules: &RuleSet) -> Result<(usize, Vec<Event>)> {
    let ts = russell_core::time::now_unix();
    let samples = probes::collect();
    for s in &samples {
        writer.append_sample(
            ts,
            Scope::Host,
            &s.name,
            s.value_num,
            s.value_text.as_deref(),
            s.unit,
        )?;
    }

    let mut events = Vec::new();
    for s in &samples {
        if let Some(v) = s.value_num {
            let sev = rules.evaluate(&s.name, v);
            if sev != Severity::Info {
                let mut ev = Event::new("threshold_breach", sev);
                ev.tier = Some("sentinel".into());
                ev.module = Some(format!("sentinel/threshold/{}", s.name));
                ev.summary = Some(format!("{} = {v:.2} breached threshold ({sev:?})", s.name,));
                ev.outputs.insert("probe".into(), s.name.clone().into());
                ev.outputs.insert("value".into(), v.into());
                if let Some(u) = s.unit {
                    ev.outputs.insert("unit".into(), u.into());
                }
                events.push(ev);
            }
        }
    }
    Ok((samples.len(), events))
}

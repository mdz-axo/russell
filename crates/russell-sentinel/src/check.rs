// SPDX-License-Identifier: MIT OR Apache-2.0
//! Threshold checks on host probe samples.
//!
//! Phase 2 rule engine (hard-coded defaults). Compares the
//! latest probe values against static warn/alert/crit thresholds
//! and emits [`russell_core::event::Event`] rows when breached.
//!
//! Future: operator overrides via `rules.d/*.toml` (lift ADR-0012).
//! Future: EWMA baselines replace static thresholds (Phase 2+).

use russell_core::Profile;
use russell_core::event::{Event, Severity};

use crate::probes::Sample;

/// Thresholds for the three MVP host probes.
const MEM_AVAILABLE_WARN_MIB: f64 = 4096.0;
const MEM_AVAILABLE_ALERT_MIB: f64 = 2048.0;
const MEM_AVAILABLE_CRIT_MIB: f64 = 1024.0;

const SWAP_USED_WARN_MIB: f64 = 4096.0;
const SWAP_USED_ALERT_MIB: f64 = 8192.0;
const SWAP_USED_CRIT_MIB: f64 = 16384.0;

/// Check each sample against its probe-specific threshold.
/// Returns zero or more events, one per breach.
///
/// `profile` supplies the CPU core count for loadavg thresholds.
/// If absent, loadavg checks are skipped.
#[must_use]
pub fn check_thresholds(samples: &[Sample], profile: Option<&Profile>) -> Vec<Event> {
    let mut events = Vec::new();
    let ncores = profile.and_then(|p| {
        if p.host.cpu.cores > 0 {
            Some(p.host.cpu.cores as f64)
        } else {
            None
        }
    });

    for s in samples {
        match s.name.as_str() {
            "mem_available_mib" => {
                if let Some(v) = s.value_num {
                    let sev = if v < MEM_AVAILABLE_CRIT_MIB {
                        Severity::Crit
                    } else if v < MEM_AVAILABLE_ALERT_MIB {
                        Severity::Alert
                    } else if v < MEM_AVAILABLE_WARN_MIB {
                        Severity::Warn
                    } else {
                        continue;
                    };
                    events.push(threshold_event(
                        "mem_available_mib",
                        sev,
                        v,
                        "MiB",
                        format!(
                            "available memory {v:.0} MiB below {} threshold ({MEM_AVAILABLE_WARN_MIB:.0} MiB)",
                            severity_label(sev)
                        ),
                    ));
                }
            }
            "swap_used_mib" => {
                if let Some(v) = s.value_num {
                    let sev = if v > SWAP_USED_CRIT_MIB {
                        Severity::Crit
                    } else if v > SWAP_USED_ALERT_MIB {
                        Severity::Alert
                    } else if v > SWAP_USED_WARN_MIB {
                        Severity::Warn
                    } else {
                        continue;
                    };
                    events.push(threshold_event(
                        "swap_used_mib",
                        sev,
                        v,
                        "MiB",
                        format!(
                            "swap usage {v:.0} MiB above {} threshold ({SWAP_USED_WARN_MIB:.0} MiB)",
                            severity_label(sev)
                        ),
                    ));
                }
            }
            "loadavg_1m" => {
                if let (Some(v), Some(cores)) = (s.value_num, ncores) {
                    let sev = if v > cores * 4.0 {
                        Severity::Crit
                    } else if v > cores * 2.0 {
                        Severity::Alert
                    } else if v > cores {
                        Severity::Warn
                    } else {
                        continue;
                    };
                    events.push(threshold_event(
                        "loadavg_1m",
                        sev,
                        v,
                        "",
                        format!(
                            "loadavg {v:.2} above {} threshold ({cores:.0} × {} cores)",
                            severity_label(sev),
                            match sev {
                                Severity::Crit => "4",
                                Severity::Alert => "2",
                                _ => "1",
                            }
                        ),
                    ));
                }
            }
            _ => {}
        }
    }

    events
}

fn threshold_event(
    probe: &str,
    severity: Severity,
    value: f64,
    unit: &str,
    summary: String,
) -> Event {
    let mut ev = Event::new("threshold_breach", severity);
    ev.tier = Some("sentinel".into());
    ev.module = Some(format!("sentinel/threshold/{probe}"));
    ev.summary = Some(summary);
    ev.outputs.insert("probe".into(), probe.into());
    ev.outputs.insert("value".into(), value.into());
    if !unit.is_empty() {
        ev.outputs.insert("unit".into(), unit.into());
    }
    ev
}

fn severity_label(sev: Severity) -> &'static str {
    match sev {
        Severity::Info => "info",
        Severity::Warn => "warn",
        Severity::Alert => "alert",
        Severity::Crit => "crit",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mem_ok_emits_nothing() {
        let s = vec![Sample {
            name: "mem_available_mib".into(),
            value_num: Some(8000.0),
            value_text: None,
            unit: Some("MiB"),
        }];
        let events = check_thresholds(&s, None);
        assert!(events.is_empty());
    }

    #[test]
    fn mem_warn_below_4096() {
        let s = vec![Sample {
            name: "mem_available_mib".into(),
            value_num: Some(3500.0),
            value_text: None,
            unit: Some("MiB"),
        }];
        let events = check_thresholds(&s, None);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].severity, Severity::Warn);
        assert!(events[0].summary.as_deref().unwrap().contains("3500"));
    }

    #[test]
    fn mem_crit_below_1024() {
        let s = vec![Sample {
            name: "mem_available_mib".into(),
            value_num: Some(512.0),
            value_text: None,
            unit: Some("MiB"),
        }];
        let events = check_thresholds(&s, None);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].severity, Severity::Crit);
    }

    #[test]
    fn swap_alert_above_8192() {
        let s = vec![Sample {
            name: "swap_used_mib".into(),
            value_num: Some(9000.0),
            value_text: None,
            unit: Some("MiB"),
        }];
        let events = check_thresholds(&s, None);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].severity, Severity::Alert);
    }

    #[test]
    fn swap_ok_emits_nothing() {
        let s = vec![Sample {
            name: "swap_used_mib".into(),
            value_num: Some(2000.0),
            value_text: None,
            unit: Some("MiB"),
        }];
        let events = check_thresholds(&s, None);
        assert!(events.is_empty());
    }

    #[test]
    fn loadavg_requires_profile() {
        let s = vec![Sample {
            name: "loadavg_1m".into(),
            value_num: Some(20.0),
            value_text: None,
            unit: None,
        }];
        // Without profile → no check.
        let events = check_thresholds(&s, None);
        assert!(events.is_empty());
    }

    #[test]
    fn loadavg_warn_above_ncores() {
        let s = vec![Sample {
            name: "loadavg_1m".into(),
            value_num: Some(9.0),
            value_text: None,
            unit: None,
        }];
        let mut p = russell_core::Profile::stub();
        p.host.cpu.cores = 8;
        let events = check_thresholds(&s, Some(&p));
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].severity, Severity::Warn);
    }

    #[test]
    fn loadavg_crit_above_4x_ncores() {
        let s = vec![Sample {
            name: "loadavg_1m".into(),
            value_num: Some(65.0),
            value_text: None,
            unit: None,
        }];
        let mut p = russell_core::Profile::stub();
        p.host.cpu.cores = 16;
        let events = check_thresholds(&s, Some(&p));
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].severity, Severity::Crit);
    }

    #[test]
    fn multiple_breaches_in_one_cycle() {
        let samples = vec![
            Sample {
                name: "mem_available_mib".into(),
                value_num: Some(1500.0),
                value_text: None,
                unit: Some("MiB"),
            },
            Sample {
                name: "swap_used_mib".into(),
                value_num: Some(10000.0),
                value_text: None,
                unit: Some("MiB"),
            },
        ];
        let events = check_thresholds(&samples, None);
        assert_eq!(events.len(), 2);
    }
}

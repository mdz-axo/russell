// SPDX-License-Identifier: MIT OR Apache-2.0
//! A deliberately tiny probe set for Phase 0.
//!
//! Each probe is infallible and returns `None` when the source is
//! unavailable. No panic, no I/O-propagated error — Sentinel's job
//! is to record what it can see and keep going.

use std::fs;

/// One sample emitted by a probe.
#[derive(Debug, Clone)]
pub struct Sample {
    /// Probe name, e.g. `"mem_available_mib"`.
    pub name: String,
    /// Numeric value, if any.
    pub value_num: Option<f64>,
    /// Textual value, if any.
    pub value_text: Option<String>,
    /// Unit string, e.g. `"MiB"`.
    pub unit: Option<&'static str>,
}

/// Collect one sample per probe. Returns only probes that
/// produced a value on this invocation.
pub fn collect() -> Vec<Sample> {
    let mut out = Vec::new();
    if let Some(v) = mem_available_mib() {
        out.push(Sample {
            name: "mem_available_mib".into(),
            value_num: Some(v),
            value_text: None,
            unit: Some("MiB"),
        });
    }
    if let Some(v) = swap_used_mib() {
        out.push(Sample {
            name: "swap_used_mib".into(),
            value_num: Some(v),
            value_text: None,
            unit: Some("MiB"),
        });
    }
    if let Some(v) = load_avg_1m() {
        out.push(Sample {
            name: "loadavg_1m".into(),
            value_num: Some(v),
            value_text: None,
            unit: None,
        });
    }
    out
}

fn read_meminfo_kib(key: &str) -> Option<u64> {
    let text = fs::read_to_string("/proc/meminfo").ok()?;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix(key) {
            let rest = rest.trim_start_matches(':').trim();
            let first = rest.split_whitespace().next()?;
            return first.parse::<u64>().ok();
        }
    }
    None
}

fn mem_available_mib() -> Option<f64> {
    read_meminfo_kib("MemAvailable").map(|kib| kib as f64 / 1024.0)
}

fn swap_used_mib() -> Option<f64> {
    let total = read_meminfo_kib("SwapTotal")?;
    let free = read_meminfo_kib("SwapFree")?;
    let used_kib = total.saturating_sub(free);
    Some(used_kib as f64 / 1024.0)
}

fn load_avg_1m() -> Option<f64> {
    let text = fs::read_to_string("/proc/loadavg").ok()?;
    text.split_whitespace().next()?.parse::<f64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_returns_some_probes_on_linux() {
        // On Linux we expect /proc/meminfo and /proc/loadavg to exist.
        let s = collect();
        if std::path::Path::new("/proc/meminfo").exists() {
            assert!(!s.is_empty(), "expected at least one probe on Linux");
        }
    }
}

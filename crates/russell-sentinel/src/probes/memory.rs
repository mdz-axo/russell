// SPDX-License-Identifier: MIT OR Apache-2.0
//! Memory probe compositions.
//!
//! Each probe composes a connector (I/O) with a tool (transform).
//! The composition is thin glue — no logic beyond sequencing.

use super::connectors;
use super::tools;

/// Probe: available memory in MiB.
///
/// Connector: read `/proc/meminfo`
/// Tool: parse `MemAvailable` key, convert KiB → MiB
pub fn mem_available_mib() -> Option<f64> {
    let raw = connectors::read_file_to_string("/proc/meminfo")?;
    tools::parse_meminfo_kib(&raw, "MemAvailable").map(tools::kib_to_mib)
}

/// Probe: swap used in MiB.
///
/// Connector: read `/proc/meminfo`
/// Tool: parse `SwapTotal` and `SwapFree`, compute difference, convert KiB → MiB
pub fn swap_used_mib() -> Option<f64> {
    let raw = connectors::read_file_to_string("/proc/meminfo")?;
    let total = tools::parse_meminfo_kib(&raw, "SwapTotal")?;
    let free = tools::parse_meminfo_kib(&raw, "SwapFree")?;
    let used_kib = total.saturating_sub(free);
    Some(tools::kib_to_mib(used_kib))
}

/// Probe: 1-minute load average.
///
/// Connector: read `/proc/loadavg`
/// Tool: parse first whitespace-delimited token as f64
pub fn load_avg_1m() -> Option<f64> {
    let raw = connectors::read_file_to_string("/proc/loadavg")?;
    tools::parse_loadavg_1m(&raw)
}

/// Return both memory pressure probes in a single read.
pub(crate) fn mem_pressure_samples() -> Vec<super::Sample> {
    let mut out = Vec::new();
    if let Some(content) = connectors::read_file_to_string("/proc/pressure/memory") {
        if let Some(v) = tools::parse_memory_pressure_some(&content) {
            out.push(super::Sample {
                name: "mem_pressure_some_pct".into(),
                value_num: Some(v),
                value_text: None,
                unit: Some("%"),
            });
        }
        if let Some(v) = tools::parse_memory_pressure_full(&content) {
            out.push(super::Sample {
                name: "mem_pressure_full_pct".into(),
                value_num: Some(v),
                value_text: None,
                unit: Some("%"),
            });
        }
    }
    out
}

// -- ProbeDescriptor impls --

use super::descriptor::ProbeDescriptor;

/// Probe descriptor for `mem_available_mib`.
pub struct MemAvailableMib;
impl ProbeDescriptor for MemAvailableMib {
    fn name(&self) -> &'static str { "mem_available_mib" }
    fn unit(&self) -> Option<&'static str> { Some("MiB") }
    fn collect(&self) -> Option<f64> { mem_available_mib() }
}

/// Probe descriptor for `swap_used_mib`.
pub struct SwapUsedMib;
impl ProbeDescriptor for SwapUsedMib {
    fn name(&self) -> &'static str { "swap_used_mib" }
    fn unit(&self) -> Option<&'static str> { Some("MiB") }
    fn collect(&self) -> Option<f64> { swap_used_mib() }
}

/// Probe descriptor for `loadavg_1m`.
pub struct LoadAvg1m;
impl ProbeDescriptor for LoadAvg1m {
    fn name(&self) -> &'static str { "loadavg_1m" }
    fn unit(&self) -> Option<&'static str> { None }
    fn collect(&self) -> Option<f64> { load_avg_1m() }
}

/// Probe descriptor for memory pressure "some".
pub struct MemPressureSome;
impl ProbeDescriptor for MemPressureSome {
    fn name(&self) -> &'static str { "mem_pressure_some_pct" }
    fn unit(&self) -> Option<&'static str> { Some("%") }
    fn collect(&self) -> Option<f64> {
        let content = connectors::read_file_to_string("/proc/pressure/memory")?;
        tools::parse_memory_pressure_some(&content)
    }
}

/// Probe descriptor for memory pressure "full".
pub struct MemPressureFull;
impl ProbeDescriptor for MemPressureFull {
    fn name(&self) -> &'static str { "mem_pressure_full_pct" }
    fn unit(&self) -> Option<&'static str> { Some("%") }
    fn collect(&self) -> Option<f64> {
        let content = connectors::read_file_to_string("/proc/pressure/memory")?;
        tools::parse_memory_pressure_full(&content)
    }
}

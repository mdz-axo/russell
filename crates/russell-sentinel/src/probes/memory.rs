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

/// Memory available probe marker.
pub struct MemAvailableMib;
/// Swap used probe marker.
pub struct SwapUsedMib;
/// Load average 1m probe marker.
pub struct LoadAvg1m;
/// Memory pressure "some" probe marker.
pub struct MemPressureSome;
/// Memory pressure "full" probe marker.
pub struct MemPressureFull;

impl_probe!(
    MemAvailableMib,
    "mem_available_mib",
    "MiB",
    mem_available_mib
);
impl_probe!(SwapUsedMib, "swap_used_mib", "MiB", swap_used_mib);
impl_probe!(LoadAvg1m, "loadavg_1m", none, load_avg_1m);
impl_probe!(
    MemPressureSome,
    "mem_pressure_some_pct",
    "%",
    mem_pressure_some
);
impl_probe!(
    MemPressureFull,
    "mem_pressure_full_pct",
    "%",
    mem_pressure_full
);

fn mem_pressure_some() -> Option<f64> {
    let content = connectors::read_file_to_string("/proc/pressure/memory")?;
    tools::parse_memory_pressure_some(&content)
}

fn mem_pressure_full() -> Option<f64> {
    let content = connectors::read_file_to_string("/proc/pressure/memory")?;
    tools::parse_memory_pressure_full(&content)
}

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

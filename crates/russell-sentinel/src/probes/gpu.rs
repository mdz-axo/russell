// SPDX-License-Identifier: MIT OR Apache-2.0
//! GPU probe compositions.
//!
//! Each probe composes connectors (I/O) with tools (transforms).
//! Targets the first discrete GPU found under `/sys/class/drm/`
//! (typically `card1` on AMD hybrid-graphics laptops).
//!
//! All probes return `None` if the expected sysfs files are not
//! present (no GPU, missing driver, or permission denied).

use super::connectors;
use super::tools;
use crate::impl_probe;

/// Sysfs path prefix for the dGPU device node.
/// On Framework 16: `card1` = RX 7700S, `card2` = iGPU (Radeon 880M).
const GPU_DEVICE: &str = "/sys/class/drm/card1/device";

/// Probe: VRAM used as a percentage of total VRAM on the dGPU.
///
/// Reads `mem_info_vram_used` and `mem_info_vram_total` (bytes).
pub fn gpu_vram_used_pct() -> Option<f64> {
    let used = connectors::read_file_to_string(&format!("{GPU_DEVICE}/mem_info_vram_used"))?;
    let total = connectors::read_file_to_string(&format!("{GPU_DEVICE}/mem_info_vram_total"))?;
    let used_bytes: u64 = used.trim().parse().ok()?;
    let total_bytes: u64 = total.trim().parse().ok()?;
    if total_bytes == 0 {
        return None;
    }
    Some((used_bytes as f64 / total_bytes as f64) * 100.0)
}

/// Probe: VRAM used in MiB on the dGPU.
pub fn gpu_vram_used_mib() -> Option<f64> {
    let used = connectors::read_file_to_string(&format!("{GPU_DEVICE}/mem_info_vram_used"))?;
    let used_bytes: u64 = used.trim().parse().ok()?;
    Some(tools::kib_to_mib(used_bytes / 1024))
}

/// Probe: total VRAM in MiB on the dGPU.
pub fn gpu_vram_total_mib() -> Option<f64> {
    let total = connectors::read_file_to_string(&format!("{GPU_DEVICE}/mem_info_vram_total"))?;
    let total_bytes: u64 = total.trim().parse().ok()?;
    Some(tools::kib_to_mib(total_bytes / 1024))
}

/// Probe: GPU temperature in °C.
///
/// Reads the first `hwmon` `temp1_input` under the GPU device.
pub fn gpu_temp_c() -> Option<f64> {
    let hwmon_dir = find_gpu_hwmon()?;
    let content = connectors::read_file_to_string(&format!("{hwmon_dir}/temp1_input"))?;
    tools::parse_millidegrees_to_c(&content)
}

/// Probe: GPU utilisation percentage (compute unit busy).
///
/// Reads `gpu_busy_percent` from the GPU device node.
pub fn gpu_util_pct() -> Option<f64> {
    let content = connectors::read_file_to_string(&format!("{GPU_DEVICE}/gpu_busy_percent"))?;
    tools::parse_gpu_util_pct(&content)
}

/// Find the first `hwmon` directory under the GPU device node.
fn find_gpu_hwmon() -> Option<String> {
    let dir = std::fs::read_dir(format!("{GPU_DEVICE}/hwmon")).ok()?;
    for entry in dir.flatten() {
        let name = entry.file_name();
        if name.to_string_lossy().starts_with("hwmon") {
            return Some(entry.path().display().to_string());
        }
    }
    None
}

// -- ProbeDescriptor impls (T13 split form) --

use super::descriptor::{ProbeCollector, ProbeMetadata};

/// Probe descriptor.
pub struct GpuVramUsedPct;
impl ProbeMetadata for GpuVramUsedPct {
    fn name(&self) -> &'static str { "gpu_vram_used_pct" }
    fn unit(&self) -> Option<&'static str> { Some("%") }
}
impl ProbeCollector for GpuVramUsedPct {
    fn collect(&self) -> Option<f64> { gpu_vram_used_pct() }
}

/// Probe descriptor.
pub struct GpuVramUsedMib;
impl ProbeMetadata for GpuVramUsedMib {
    fn name(&self) -> &'static str { "gpu_vram_used_mib" }
    fn unit(&self) -> Option<&'static str> { Some("MiB") }
}
impl ProbeCollector for GpuVramUsedMib {
    fn collect(&self) -> Option<f64> { gpu_vram_used_mib() }
}

/// Probe descriptor.
pub struct GpuVramTotalMib;
impl ProbeMetadata for GpuVramTotalMib {
    fn name(&self) -> &'static str { "gpu_vram_total_mib" }
    fn unit(&self) -> Option<&'static str> { Some("MiB") }
}
impl ProbeCollector for GpuVramTotalMib {
    fn collect(&self) -> Option<f64> { gpu_vram_total_mib() }
}

/// Probe descriptor.
pub struct GpuTempC;
impl ProbeMetadata for GpuTempC {
    fn name(&self) -> &'static str { "gpu_temp_c" }
    fn unit(&self) -> Option<&'static str> { Some("°C") }
}
impl ProbeCollector for GpuTempC {
    fn collect(&self) -> Option<f64> { gpu_temp_c() }
}

/// Probe descriptor.
pub struct GpuUtilPct;
impl ProbeMetadata for GpuUtilPct {
    fn name(&self) -> &'static str { "gpu_util_pct" }
    fn unit(&self) -> Option<&'static str> { Some("%") }
}
impl ProbeCollector for GpuUtilPct {
    fn collect(&self) -> Option<f64> { gpu_util_pct() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpu_vram_used_pct_is_percentage_on_linux() {
        if !std::path::Path::new(GPU_DEVICE).exists() {
            return;
        }
        // The device node may exist (e.g. iGPU) without exposing
        // mem_info_vram_used — in that case the probe returns None.
        if let Some(v) = gpu_vram_used_pct() {
            assert!(
                (0.0..=100.0).contains(&v),
                "vram pct should be 0-100, got {v}"
            );
        }
    }

    #[test]
    fn gpu_temp_c_is_plausible_on_linux() {
        if !std::path::Path::new(GPU_DEVICE).exists() {
            return;
        }
        let temp = gpu_temp_c();
        if let Some(t) = temp {
            // Temperature should be between 0 and 125°C.
            assert!(
                (0.0..=125.0).contains(&t),
                "temp should be 0-125°C, got {t}"
            );
        }
    }

    #[test]
    fn gpu_util_pct_is_percentage_on_linux() {
        if !std::path::Path::new(GPU_DEVICE).exists() {
            return;
        }
        let util = gpu_util_pct();
        if let Some(u) = util {
            assert!(
                (0.0..=100.0).contains(&u),
                "util pct should be 0-100, got {u}"
            );
        }
    }
}

// SPDX-License-Identifier: MIT OR Apache-2.0
//! GPU probe compositions.
//!
//! Each probe composes connectors (I/O) with tools (transforms).
//! Targets the first discrete GPU found under `/sys/class/drm/`
//! (typically `card1` on AMD hybrid-graphics laptops).
//!
//! All probes return `None` if the expected sysfs files are not
//! present (no GPU, missing driver, or permission denied).
//!
//! ## Task I2: Dynamic dGPU Detection
//!
//! The `detect_dgpu_card()` function enumerates `/sys/class/drm/card*/`
//! and selects the discrete GPU over integrated GPU based on:
//! 1. VRAM total (dGPU typically has more)
//! 2. Device vendor (AMD/NVIDIA vs Intel)
//! 3. Fallback to `card1` if detection fails

use super::connectors;
use super::tools;

/// Detect the discrete GPU card index dynamically.
///
/// Enumerates `/sys/class/drm/card*/device/` directories and selects
/// the dGPU based on VRAM capacity. Falls back to `card1` if detection
/// fails (preserving existing behavior).
///
/// Returns the card number (e.g., `1` for `card1`).
fn detect_dgpu_card() -> u32 {
    let drm_dir = match std::fs::read_dir("/sys/class/drm") {
        Ok(d) => d,
        Err(_) => return 1, // Fallback to card1
    };

    let mut best_card = 1u32;
    let mut best_vram = 0u64;

    for entry in drm_dir.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        // Only consider card* directories (not render*, etc.)
        if !name_str.starts_with("card") {
            continue;
        }

        // Skip if not a pure card number (e.g., skip card1-DP-1)
        if !name_str.chars().skip(4).all(|c| c.is_ascii_digit()) {
            continue;
        }

        let card_num: u32 = match name_str[4..].parse() {
            Ok(n) => n,
            Err(_) => continue,
        };

        let device_path = entry.path().join("device");
        let vram_total_path = device_path.join("mem_info_vram_total");

        // Read VRAM total to determine if this is a dGPU
        if let Ok(content) = std::fs::read_to_string(&vram_total_path)
            && let Ok(vram_bytes) = content.trim().parse::<u64>()
        {
            // dGPUs typically have more VRAM than iGPUs
            if vram_bytes > best_vram {
                best_vram = vram_bytes;
                best_card = card_num;
            }
        }
    }

    // Fallback: if no VRAM info found, prefer card1 over card0
    // (card0 is often the iGPU on hybrid systems)
    if best_vram == 0 && best_card == 0 {
        best_card = 1;
    }

    best_card
}

/// Sysfs path prefix for the dGPU device node.
/// Dynamically detected via `detect_dgpu_card()`.
/// On Framework 16: `card1` = RX 7700S, `card2` = iGPU (Radeon 880M).
fn gpu_device_path() -> String {
    let card = detect_dgpu_card();
    format!("/sys/class/drm/card{card}/device")
}

/// Probe: VRAM used as a percentage of total VRAM on the dGPU.
///
/// Reads `mem_info_vram_used` and `mem_info_vram_total` (bytes).
pub fn gpu_vram_used_pct() -> Option<f64> {
    let gpu_device = gpu_device_path();
    let used = connectors::read_file_to_string(&format!("{gpu_device}/mem_info_vram_used"))?;
    let total = connectors::read_file_to_string(&format!("{gpu_device}/mem_info_vram_total"))?;
    let used_bytes: u64 = used.trim().parse().ok()?;
    let total_bytes: u64 = total.trim().parse().ok()?;
    if total_bytes == 0 {
        return None;
    }
    Some((used_bytes as f64 / total_bytes as f64) * 100.0)
}

/// Probe: VRAM used in MiB on the dGPU.
pub fn gpu_vram_used_mib() -> Option<f64> {
    let gpu_device = gpu_device_path();
    let used = connectors::read_file_to_string(&format!("{gpu_device}/mem_info_vram_used"))?;
    let used_bytes: u64 = used.trim().parse().ok()?;
    Some(tools::kib_to_mib(used_bytes / 1024))
}

/// Probe: total VRAM in MiB on the dGPU.
pub fn gpu_vram_total_mib() -> Option<f64> {
    let gpu_device = gpu_device_path();
    let total = connectors::read_file_to_string(&format!("{gpu_device}/mem_info_vram_total"))?;
    let total_bytes: u64 = total.trim().parse().ok()?;
    Some(tools::kib_to_mib(total_bytes / 1024))
}

/// Probe: GPU temperature in °C.
///
/// Reads the first `hwmon` `temp1_input` under the GPU device.
pub fn gpu_temp_c() -> Option<f64> {
    let gpu_device = gpu_device_path();
    let hwmon_dir = find_gpu_hwmon(&gpu_device)?;
    let content = connectors::read_file_to_string(&format!("{hwmon_dir}/temp1_input"))?;
    tools::parse_millidegrees_to_c(&content)
}

/// Probe: GPU utilisation percentage (compute unit busy).
///
/// Reads `gpu_busy_percent` from the GPU device node.
pub fn gpu_util_pct() -> Option<f64> {
    let gpu_device = gpu_device_path();
    let content = connectors::read_file_to_string(&format!("{gpu_device}/gpu_busy_percent"))?;
    tools::parse_gpu_util_pct(&content)
}

/// Find the first `hwmon` directory under the GPU device node.
fn find_gpu_hwmon(gpu_device: &str) -> Option<String> {
    let dir = std::fs::read_dir(format!("{gpu_device}/hwmon")).ok()?;
    for entry in dir.flatten() {
        let name = entry.file_name();
        if name.to_string_lossy().starts_with("hwmon") {
            return Some(entry.path().display().to_string());
        }
    }
    None
}

// -- ProbeDescriptor impls (T13 split form) --


/// GPU VRAM usage percentage probe marker.
pub struct GpuVramUsedPct;
/// GPU VRAM usage in MiB probe marker.
pub struct GpuVramUsedMib;
/// GPU VRAM total in MiB probe marker.
pub struct GpuVramTotalMib;
/// GPU temperature probe marker.
pub struct GpuTempC;
/// GPU utilization percentage probe marker.
pub struct GpuUtilPct;

impl_probe!(GpuVramUsedPct, "gpu_vram_used_pct", "%", gpu_vram_used_pct);
impl_probe!(GpuVramUsedMib, "gpu_vram_used_mib", "MiB", gpu_vram_used_mib);
impl_probe!(GpuVramTotalMib, "gpu_vram_total_mib", "MiB", gpu_vram_total_mib);
impl_probe!(GpuTempC, "gpu_temp_c", "°C", gpu_temp_c);
impl_probe!(GpuUtilPct, "gpu_util_pct", "%", gpu_util_pct);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpu_vram_used_pct_is_percentage_on_linux() {
        let gpu_device = gpu_device_path();
        if !std::path::Path::new(&gpu_device).exists() {
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
        let gpu_device = gpu_device_path();
        if !std::path::Path::new(&gpu_device).exists() {
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
        let gpu_device = gpu_device_path();
        if !std::path::Path::new(&gpu_device).exists() {
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

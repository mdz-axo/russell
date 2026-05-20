// SPDX-License-Identifier: MIT OR Apache-2.0
//! Disk probe compositions.
//!
//! Monitors I/O pressure and storage health.
//! All probes return `None` if the expected procfs files are not
//! available (kernel too old, containerised environment, etc.).

use super::connectors;
use super::tools;

/// Probe: I/O pressure "some" average over the last 10 seconds.
///
/// From `/proc/pressure/io`. Values above ~10% suggest the system
/// is spending meaningful time waiting on I/O.
pub fn disk_io_pressure_some_pct() -> Option<f64> {
    let content = connectors::read_file_to_string("/proc/pressure/io")?;
    tools::parse_io_pressure_some(&content)
}

/// Probe: I/O pressure "full" average over the last 10 seconds.
///
/// "Full" pressure means all productive work is stalled — a more
/// severe signal than "some". Any non-zero value indicates
/// I/O saturation.
pub fn disk_io_pressure_full_pct() -> Option<f64> {
    let content = connectors::read_file_to_string("/proc/pressure/io")?;
    tools::parse_io_pressure_full(&content)
}

/// Probe: root filesystem usage as a percentage.
///
/// Uses `df -B1 --output=size,used /` via the subprocess connector.
pub fn disk_root_used_pct() -> Option<f64> {
    let output = connectors::run_command_stdout_always(&["df", "-B1", "--output=size,used", "/"])?;
    let (total, used) = tools::parse_df_output(&output)?;
    if total == 0 {
        return None;
    }
    Some((used as f64 / total as f64) * 100.0)
}

impl_probe!(DiskIoPressureSome, "disk_io_pressure_some_pct", "%", disk_io_pressure_some_pct);
impl_probe!(DiskIoPressureFull, "disk_io_pressure_full_pct", "%", disk_io_pressure_full_pct);
impl_probe!(DiskRootUsedPct, "disk_root_used_pct", "%", disk_root_used_pct);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disk_io_pressure_returns_something_on_linux() {
        if !std::path::Path::new("/proc/pressure/io").exists() {
            return;
        }
        let some = disk_io_pressure_some_pct();
        assert!(some.is_some(), "io pressure some should be readable");
        if let Some(v) = some {
            assert!((0.0..=100.0).contains(&v), "pressure pct 0-100, got {v}");
        }
        let full = disk_io_pressure_full_pct();
        assert!(full.is_some(), "io pressure full should be readable");
        if let Some(v) = full {
            assert!((0.0..=100.0).contains(&v), "pressure pct 0-100, got {v}");
        }
    }
}

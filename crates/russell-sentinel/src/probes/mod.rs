// SPDX-License-Identifier: MIT OR Apache-2.0
//! Probe orchestrator.
//!
//! Composes probe families into collection functions. Each family
//! lives in its own module; this module is the pipeline stage that
//! iterates them.
//!
//! CTHA: `ctha.pipeline.sentinel_collect.duration_ms`, `items_out`

pub mod connectors;
pub mod disks;
pub mod gpu;
pub mod memory;
pub mod process;
pub mod tools;

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
///
/// CTHA: `ctha.pipeline.sentinel_collect`
#[tracing::instrument(
    level = "debug",
    fields(
        ctha.pipeline.sentinel_collect.items_out,
    )
)]
pub fn collect() -> Vec<Sample> {
    let mut out = Vec::new();

    if let Some(v) = memory::mem_available_mib() {
        out.push(Sample {
            name: "mem_available_mib".into(),
            value_num: Some(v),
            value_text: None,
            unit: Some("MiB"),
        });
    }
    if let Some(v) = memory::swap_used_mib() {
        out.push(Sample {
            name: "swap_used_mib".into(),
            value_num: Some(v),
            value_text: None,
            unit: Some("MiB"),
        });
    }
    if let Some(v) = memory::load_avg_1m() {
        out.push(Sample {
            name: "loadavg_1m".into(),
            value_num: Some(v),
            value_text: None,
            unit: None,
        });
    }
    if let Some(v) = process::proc_total_count() {
        out.push(Sample {
            name: "proc_total_count".into(),
            value_num: Some(v),
            value_text: None,
            unit: Some("count"),
        });
    }
    if let Some(v) = process::proc_zombie_count() {
        out.push(Sample {
            name: "proc_zombie_count".into(),
            value_num: Some(v),
            value_text: None,
            unit: Some("count"),
        });
    }
    if let Some(v) = process::proc_stuck_count() {
        out.push(Sample {
            name: "proc_stuck_count".into(),
            value_num: Some(v),
            value_text: None,
            unit: Some("count"),
        });
    }
    if let Some(v) = process::proc_running_count() {
        out.push(Sample {
            name: "proc_running_count".into(),
            value_num: Some(v),
            value_text: None,
            unit: Some("count"),
        });
    }
    if let Some(v) = process::proc_top_cpu_name() {
        out.push(Sample {
            name: "proc_top_cpu_name".into(),
            value_num: None,
            value_text: Some(v),
            unit: None,
        });
    }
    if let Some(v) = process::proc_top_mem_name() {
        out.push(Sample {
            name: "proc_top_mem_name".into(),
            value_num: None,
            value_text: Some(v),
            unit: None,
        });
    }
    if let Some(v) = process::proc_top_mem_pct() {
        out.push(Sample {
            name: "proc_top_mem_pct".into(),
            value_num: Some(v),
            value_text: None,
            unit: Some("%"),
        });
    }
    if let Some(v) = gpu::gpu_vram_used_pct() {
        out.push(Sample {
            name: "gpu_vram_used_pct".into(),
            value_num: Some(v),
            value_text: None,
            unit: Some("%"),
        });
    }
    if let Some(v) = gpu::gpu_vram_used_mib() {
        out.push(Sample {
            name: "gpu_vram_used_mib".into(),
            value_num: Some(v),
            value_text: None,
            unit: Some("MiB"),
        });
    }
    if let Some(v) = gpu::gpu_vram_total_mib() {
        out.push(Sample {
            name: "gpu_vram_total_mib".into(),
            value_num: Some(v),
            value_text: None,
            unit: Some("MiB"),
        });
    }
    if let Some(v) = gpu::gpu_temp_c() {
        out.push(Sample {
            name: "gpu_temp_c".into(),
            value_num: Some(v),
            value_text: None,
            unit: Some("°C"),
        });
    }
    if let Some(v) = gpu::gpu_util_pct() {
        out.push(Sample {
            name: "gpu_util_pct".into(),
            value_num: Some(v),
            value_text: None,
            unit: Some("%"),
        });
    }
    if let Some(v) = disks::disk_io_pressure_some_pct() {
        out.push(Sample {
            name: "disk_io_pressure_some_pct".into(),
            value_num: Some(v),
            value_text: None,
            unit: Some("%"),
        });
    }
    if let Some(v) = disks::disk_io_pressure_full_pct() {
        out.push(Sample {
            name: "disk_io_pressure_full_pct".into(),
            value_num: Some(v),
            value_text: None,
            unit: Some("%"),
        });
    }

    tracing::Span::current().record("ctha.pipeline.sentinel_collect.items_out", out.len());
    out
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

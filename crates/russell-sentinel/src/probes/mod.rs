// SPDX-License-Identifier: MIT OR Apache-2.0
//! Probe orchestrator.
//!
//! Composes probe families into collection functions. Each family
//! lives in its own module; this module is the pipeline stage that
//! iterates them.
//!
//! Probes are registered in [`ProbeRegistry`] via zero-sized types
//! implementing [`ProbeDescriptor`]. The old `collect()` function
//! (which manually constructed each sample) is replaced by
//! `ProbeRegistry::collect_all()`.
//!
//! OKH: `okh.pipeline.sentinel_collect.duration_ms`, `items_out`

pub mod connectors;
pub mod descriptor;
pub mod disks;
pub mod gpu;
pub mod memory;
pub mod network;
pub mod process;
pub mod registry;
pub mod systemd;
pub mod tools;

pub use descriptor::ProbeDescriptor;
pub use registry::ProbeRegistry;

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
/// Uses the lazy-initialised singleton [`REGISTRY`]; the registry
/// is built once and reused across all Sentinel cycles.
///
/// OKH: `okh.pipeline.sentinel_collect`
#[tracing::instrument(
    level = "debug",
    fields(
        okh.pipeline.sentinel_collect.items_out,
    )
)]
pub fn collect() -> Vec<Sample> {
    REGISTRY.collect_all()
}

/// Lazy-initialised singleton — the registry is built once and
/// reused across Sentinel cycles.
static REGISTRY: std::sync::LazyLock<ProbeRegistry> =
    std::sync::LazyLock::new(ProbeRegistry::with_defaults);

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

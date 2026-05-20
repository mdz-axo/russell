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

pub use descriptor::{ProbeCollector, ProbeDescriptor, ProbeMetadata};
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

/// Collect one sample per probe using the default registry.
///
/// Uses the lazy-initialised singleton [`REGISTRY`]; the registry
/// is built once and reused across all Sentinel cycles.
///
/// For dependency injection (testing, subset probes), prefer
/// [`collect_with`] which accepts an explicit `&ProbeRegistry`.
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

/// Collect samples using an explicitly-provided registry.
///
/// This is the **capability-injected** version of [`collect`].
/// Callers pass the registry they constructed, enabling:
/// - Subset probes for fast tests
/// - Custom probes registered from skills
/// - Platform-conditional probes
///
/// Production code can use either this or [`collect()`]; the
/// singleton exists only as a convenience for the hot path.
pub fn collect_with(registry: &ProbeRegistry) -> Vec<Sample> {
    registry.collect_all()
}

/// Lazy-initialised singleton — the registry is built once and
/// reused across Sentinel cycles. Use [`collect_with`] to bypass
/// the singleton for testing or custom probe configurations.
static REGISTRY: std::sync::LazyLock<ProbeRegistry> =
    std::sync::LazyLock::new(ProbeRegistry::with_defaults);

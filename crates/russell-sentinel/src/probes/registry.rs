// SPDX-License-Identifier: MIT OR Apache-2.0
//! Probe registry — the collection of all host-scope probes.
//!
//! Constructed once via [`ProbeRegistry::with_defaults`], the
//! registry holds a vector of boxed [`ProbeDescriptor`] trait
//! objects. [`collect_all`](ProbeRegistry::collect_all) iterates
//! them in registration order and returns a [`Vec<Sample>`].
//!
//! Adding a new probe requires: (1) a new type implementing
//! [`ProbeDescriptor`], (2) one line in [`with_defaults`] to
//! register it.

use super::Sample;
use super::descriptor::ProbeDescriptor;

use super::disks::{DiskIoPressureFull, DiskIoPressureSome, DiskRootUsedPct};
use super::gpu::{GpuTempC, GpuUtilPct, GpuVramTotalMib, GpuVramUsedMib, GpuVramUsedPct};
use super::memory::{LoadAvg1m, MemAvailableMib, SwapUsedMib};
use super::network::{NetTcp6Connections, NetTcpConnections};
use super::process::{
    ProcRunningCount, ProcStuckCount, ProcTopMemPct, ProcTotalCount, ProcZombieCount,
};
use super::systemd::{SystemdDegraded, SystemdSystemFailedCount, SystemdUserFailedCount};

/// A collection of host-scope probes. Constructed once at startup;
/// `collect_all` is called every Sentinel cycle.
pub struct ProbeRegistry {
    probes: Vec<Box<dyn ProbeDescriptor>>,
}

impl ProbeRegistry {
    /// Build the full registry with all 25 MVP probes plus
    /// text-valued probes.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self {
            probes: vec![
                // Memory
                Box::new(MemAvailableMib),
                Box::new(SwapUsedMib),
                Box::new(LoadAvg1m),
                // Memory pressure
                Box::new(super::memory::MemPressureSome),
                Box::new(super::memory::MemPressureFull),
                // GPU
                Box::new(GpuVramUsedPct),
                Box::new(GpuVramUsedMib),
                Box::new(GpuVramTotalMib),
                Box::new(GpuTempC),
                Box::new(GpuUtilPct),
                // Disks
                Box::new(DiskIoPressureSome),
                Box::new(DiskIoPressureFull),
                Box::new(DiskRootUsedPct),
                // Network
                Box::new(NetTcpConnections),
                Box::new(NetTcp6Connections),
                // Processes — numeric
                Box::new(ProcTotalCount),
                Box::new(ProcZombieCount),
                Box::new(ProcStuckCount),
                Box::new(ProcRunningCount),
                Box::new(ProcTopMemPct),
                // Systemd
                Box::new(SystemdDegraded),
                Box::new(SystemdUserFailedCount),
                Box::new(SystemdSystemFailedCount),
            ],
        }
    }

    /// Collect all numeric probes in registration order, skipping
    /// any that return `None` (unavailable on this host).
    #[must_use]
    pub fn collect_numeric(&self) -> Vec<Sample> {
        self.probes.iter().filter_map(|p| p.sample()).collect()
    }

    /// Collect all samples — numeric probes via the registry, plus
    /// text-valued probes (process names).
    ///
    /// This is the preferred entry point for the Sentinel orchestrator.
    /// It matches the output of the old `collect()` function —
    /// numeric probes from the registry, text probes from their
    /// dedicated functions.
    #[must_use]
    pub fn collect_all(&self) -> Vec<Sample> {
        let mut out = self.collect_numeric();

        // Text-valued probes (no f64 value, names only).
        out.extend(super::process::process_text_samples());

        tracing::Span::current().record("okh.pipeline.sentinel_collect.items_out", out.len());
        out
    }

    /// Number of registered probes (numeric only).
    #[cfg(test)]
    #[must_use]
    pub fn len(&self) -> usize {
        self.probes.len()
    }

    /// Returns `true` if no probes are registered.
    #[cfg(test)]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.probes.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_has_all_mvp_numeric_probes() {
        let reg = ProbeRegistry::with_defaults();
        assert!(
            reg.len() >= 20,
            "expected at least 20 numeric MVP probes, got {}",
            reg.len()
        );
    }

    #[test]
    fn each_probe_has_unique_name() {
        let reg = ProbeRegistry::with_defaults();
        let names: std::collections::BTreeSet<&str> = reg.probes.iter().map(|p| p.name()).collect();
        assert_eq!(names.len(), reg.probes.len(), "probe names must be unique");
    }

    #[test]
    fn collect_numeric_returns_results_on_linux() {
        if !std::path::Path::new("/proc/meminfo").exists() {
            return;
        }
        let reg = ProbeRegistry::with_defaults();
        let samples = reg.collect_numeric();
        assert!(
            !samples.is_empty(),
            "should have at least one numeric sample on Linux"
        );
    }
}

// SPDX-License-Identifier: MIT OR Apache-2.0
//! Probe descriptor trait — the port definition for a single
//! host-scope probe.
//!
//! Each probe is a zero-sized type implementing [`ProbeDescriptor`].
//! The trait provides declarative metadata (name, unit) and a
//! collection function returning `Option<f64>`.
//!
//! This replaces the manual `collect()` boilerplate with a
//! type-driven registry ([`super::ProbeRegistry`]) that iterates
//! all registered probes and produces [`super::Sample`] values.

use super::Sample;

/// A single host probe — name, unit, and collection logic.
///
/// Implementors are zero-sized types (no state). The `collect`
/// method reads from `/proc`, `/sys`, or subprocesses and returns
/// either a numeric value or `None` (probe unavailable).
pub trait ProbeDescriptor: Send + Sync {
    /// Immutable probe name, e.g. `"mem_available_mib"`.
    fn name(&self) -> &'static str;

    /// Unit string, e.g. `"MiB"`, `"%"`, `"°C"`, or `None` for
    /// unitless probes.
    fn unit(&self) -> Option<&'static str>;

    /// Collect the probe value. Returns `None` if the probe is
    /// unavailable on this host (missing sysfs, permission denied,
    /// no GPU, etc.).
    fn collect(&self) -> Option<f64>;
}

/// Convenience: convert a [`ProbeDescriptor`] into a [`Sample`].
impl dyn ProbeDescriptor {
    /// Produce a `Sample` if the probe succeeds, or `None` if the
    /// probe is unavailable.
    pub(crate) fn sample(&self) -> Option<Sample> {
        self.collect().map(|v| Sample {
            name: self.name().into(),
            value_num: Some(v),
            value_text: None,
            unit: self.unit(),
        })
    }
}

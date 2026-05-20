// SPDX-License-Identifier: MIT OR Apache-2.0
//! Probe descriptor traits — the port definitions for host-scope probes.
//!
//! ## Architecture (T13 — identity/collector separation)
//!
//! Two tiers:
//! - [`ProbeDescriptor`] — the unified trait (name + unit + collect).
//!   All existing probes implement this directly. The registry stores
//!   `Box<dyn ProbeDescriptor>`.
//! - [`ProbeMetadata`] + [`ProbeCollector`] — the split form for new
//!   code. `ProbeCollector: ProbeMetadata` so it's a refinement.
//!   Types implementing `ProbeCollector` also satisfy `ProbeDescriptor`
//!   via a blanket impl.
//!
//! ### Why both?
//!
//! The unified trait keeps existing code unchanged (JR-1: no gratuitous
//! churn). The split form enables:
//! - Pure introspection (`&dyn ProbeMetadata`) without I/O deps
//! - Mock probes in tests (implement `ProbeCollector` with stub)
//! - Skill-registered probes with metadata from manifest

use super::Sample;

/// The unified probe trait — name, unit, and collection in one.
///
/// The registry stores `Box<dyn ProbeDescriptor>`. Existing probes
/// implement this directly. New probes may prefer the split form
/// ([`ProbeMetadata`] + [`ProbeCollector`]).
pub trait ProbeDescriptor: Send + Sync {
    /// Immutable probe name, e.g. `"mem_available_mib"`.
    fn name(&self) -> &'static str;

    /// Unit string, e.g. `"MiB"`, `"%"`, `"°C"`, or `None` for
    /// unitless probes.
    fn unit(&self) -> Option<&'static str>;

    /// Collect the probe value. Returns `None` if the probe is
    /// unavailable on this host.
    fn collect(&self) -> Option<f64>;
}

/// Pure probe identity — no I/O, no side effects.
///
/// The "read" half of a probe. Enables introspection, serialization,
/// and display without importing collection logic.
pub trait ProbeMetadata: Send + Sync {
    /// Immutable probe name.
    fn name(&self) -> &'static str;
    /// Unit string.
    fn unit(&self) -> Option<&'static str>;
    /// Category for grouping. Default: "host".
    fn category(&self) -> &'static str {
        "host"
    }
}

/// A probe that can collect a value.
///
/// The "execute" half — extends [`ProbeMetadata`] with I/O.
/// Implementing this automatically provides [`ProbeDescriptor`]
/// via blanket impl.
pub trait ProbeCollector: ProbeMetadata {
    /// Collect the probe value. Returns `None` if unavailable.
    fn collect(&self) -> Option<f64>;
}

/// Blanket: anything implementing the split traits satisfies the
/// unified descriptor.
impl<T: ProbeCollector + 'static> ProbeDescriptor for T {
    fn name(&self) -> &'static str {
        ProbeMetadata::name(self)
    }
    fn unit(&self) -> Option<&'static str> {
        ProbeMetadata::unit(self)
    }
    fn collect(&self) -> Option<f64> {
        ProbeCollector::collect(self)
    }
}

/// Convenience: convert a [`dyn ProbeDescriptor`] into a [`Sample`].
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

/// Macro to generate ProbeDescriptor impls, reducing boilerplate.
///
/// Usage: `impl_probe!(StructName, "probe_name", "unit", function_name);`
///
/// For unitless probes: `impl_probe!(StructName, "probe_name", none, function_name);`
#[macro_export]
macro_rules! impl_probe {
    ($struct_name:ident, $name:literal, "unit", $func:ident) => {
        impl $crate::probes::descriptor::ProbeMetadata for $struct_name {
            fn name(&self) -> &'static str { $name }
            fn unit(&self) -> Option<&'static str> { Some("unit") }
        }
        impl $crate::probes::descriptor::ProbeCollector for $struct_name {
            fn collect(&self) -> Option<f64> { $func() }
        }
    };
    ($struct_name:ident, $name:literal, $unit:literal, $func:ident) => {
        impl $crate::probes::descriptor::ProbeMetadata for $struct_name {
            fn name(&self) -> &'static str { $name }
            fn unit(&self) -> Option<&'static str> { Some($unit) }
        }
        impl $crate::probes::descriptor::ProbeCollector for $struct_name {
            fn collect(&self) -> Option<f64> { $func() }
        }
    };
    ($struct_name:ident, $name:literal, none, $func:ident) => {
        impl $crate::probes::descriptor::ProbeMetadata for $struct_name {
            fn name(&self) -> &'static str { $name }
            fn unit(&self) -> Option<&'static str> { None }
        }
        impl $crate::probes::descriptor::ProbeCollector for $struct_name {
            fn collect(&self) -> Option<f64> { $func() }
        }
    };
}

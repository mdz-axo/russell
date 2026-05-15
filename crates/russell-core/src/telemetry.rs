// SPDX-License-Identifier: MIT OR Apache-2.0
//! `tracing` subscriber setup (basic logging only in MVP).
//!
//! Phase 0 wires up only the terminal `fmt` subscriber; the
//! journald bridge (`tracing-journald`) and full OpenTelemetry
//! observability stack are deferred per
//! [ADR-0010](../../../docs/adr/deferred/0010-observability-stack.md).

use tracing_subscriber::{EnvFilter, fmt};

/// Initialise global `tracing` subscribers.
///
/// Safe to call from main. Ignores "already initialised" errors
/// so tests and the CLI can co-exist in a single binary.
pub fn init() {
    let filter = EnvFilter::try_from_env("RUSSELL_LOG")
        .or_else(|_| EnvFilter::try_from_default_env())
        .unwrap_or_else(|_| EnvFilter::new("russell=info,warn"));

    let _ = fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_writer(std::io::stderr)
        .try_init();
}

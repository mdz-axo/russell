// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell-proprio` — Phase 0 placeholder.
//!
//! This crate is reserved per
//! [ADR-0013](../../docs/adr/0013-rust-workspace-layout.md).
//! Phase 0 (`cybernetic-health-harness.md` §20) only populates
//! [`russell-core`] and [`russell-cli`]; the other crates exist so
//! the dependency DAG is correct from day one and so imports do not
//! churn when implementation lands.

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms)]
#![warn(missing_docs)]

/// Phase-0 version marker. Removed when the crate gains real APIs.
pub const PHASE0_STUB: &str = "russell-proprio";

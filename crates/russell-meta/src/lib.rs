// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell-meta` — metacognitive layer (the Nurse).
//!
//! Collects telemetry and calls hKask for LLM inference.

#![deny(unsafe_code)]
#![deny(rust_2018_idioms)]
#![warn(missing_docs)]
#![allow(clippy::missing_docs_in_private_items)]

pub mod action;
pub mod error;
pub mod fallback;
pub mod fallback_adapter;
pub mod health;
pub mod help;
pub mod hkask_adapter;
pub mod okapi_adapter;

pub use error::{DoctorError, Result};
pub use fallback_adapter::FallbackInferenceAdapter;
pub use help::{HelpOutcome, run_help, run_help_with_endpoint};
pub use hkask_adapter::HkaskInferenceAdapter;
pub use okapi_adapter::OkapiInferenceAdapter;

/// Jack's nurse persona prompt (loaded from `prompts/jack.md`).
pub const JACK_PERSONA: &str = include_str!("../prompts/jack.md");

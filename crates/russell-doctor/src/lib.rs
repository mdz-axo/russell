// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell-doctor` — the MVP Doctor.
//!
//! Under JR-4 (small but present), this crate implements exactly
//! one capability: `russell help`. The LLM consults; Russell does
//! not act.
//!
//! See [ADR-0016](../../docs/adr/0016-doctor-and-llm-router.md).

#![deny(unsafe_code)]
#![deny(rust_2018_idioms)]
#![warn(missing_docs)]

pub mod client;
pub mod error;
pub mod fallback;
pub mod help;
pub mod mock;
pub mod openrouter;
pub mod prompt;

pub use client::{Backend, ClientConfig, EscalateMin, LlmClient, LlmResponse, SoapPrompt};
pub use error::{DoctorError, Result};
pub use help::{HelpOutcome, HelpSession, run_help};

/// The Jack persona, embedded at compile time so operators cannot
/// accidentally run Jack without his voice.
///
/// Source of truth: `crates/russell-doctor/prompts/jack.md`.
/// Design rationale: `docs/architecture/THE_JACK.md`.
pub const JACK_PERSONA: &str = include_str!("../prompts/jack.md");

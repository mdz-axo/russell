// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell-doctor` — the Nurse subsystem.
//!
//! Under JR-4 (small but present, the Nurse), this crate implements
//! the LLM consultation channel: `russell jack`. The LLM consults
//! as a nurse checking in on the patient; Russell does not act.
//!
//! See [ADR-0016](../../docs/adr/0016-doctor-and-llm-router.md).

#![deny(unsafe_code)]
#![deny(rust_2018_idioms)]
#![warn(missing_docs)]

pub mod action;
pub mod client;
pub mod error;
pub mod fallback;
pub mod health;
pub mod help;
pub mod mock;
pub mod oai_client;
pub mod prompt;
pub mod prompt_registry;

pub use client::{Backend, ClientConfig, EscalateMin, LlmClient, LlmResponse, SoapPrompt};
pub use error::{DoctorError, Result};
pub use help::{HelpOutcome, run_help};

/// The Jack persona, embedded at compile time so operators cannot
/// accidentally run Jack without his voice.
///
/// Source of truth: `crates/russell-doctor/prompts/jack.md`.
/// Design rationale: `docs/architecture/THE_JACK.md`.
pub const JACK_PERSONA: &str = include_str!("../prompts/jack.md");

/// The Jack chat persona — used in `russell chat` interactive mode.
///
/// More conversational, supports multi-turn dialogue, and gives Jack
/// permission to ask clarifying questions and design probes.
///
/// Source of truth: `crates/russell-doctor/prompts/jack-chat.md`.
pub const JACK_CHAT_PERSONA: &str = include_str!("../prompts/jack-chat.md");

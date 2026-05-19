// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell-meta` — metacognitive layer (the Nurse).
//!
//! **TOGAF Phase:** Phase C (Application Architecture) — implements the
//! LLM consultation capability (JR-4). Composes SOAP prompts, resolves
//! ACTION: syntax, injects skill knowledge, and routes to the configured
//! LLM backend (Okapi by default; OpenRouter opt-in).
//!
//! Previously `russell-doctor`. Renamed per ADR-0026.
//! this crate performs metacognitive functions (reasoning about
//! reasoning, attention allocation, self-critique) rather than the
//! narrower "doctor consultation" metaphor.
//!
//! The *persona* is still Jack the Nurse. The *crate* is the
//! metacognitive substrate that enables Jack to function.
//!
//! See [ADR-0026](../../docs/adr/0026-metacognitive-layer.md).

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
pub mod rate_limit;

pub use client::{Backend, ClientConfig, EscalateMin, LlmClient, LlmResponse, SoapPrompt};
pub use error::{DoctorError, Result};
pub use help::{HelpOutcome, run_help};

/// The Jack persona, embedded at compile time so operators cannot
/// accidentally run Jack without his voice.
///
/// Source of truth: `crates/russell-meta/prompts/jack.md`.
/// Design rationale: `docs/architecture/THE_JACK.md`.
pub const JACK_PERSONA: &str = include_str!("../prompts/jack.md");

/// The Jack chat persona — used in `russell chat` interactive mode.
///
/// More conversational, supports multi-turn dialogue, and gives Jack
/// permission to ask clarifying questions and design probes.
///
/// Source of truth: `crates/russell-meta/prompts/jack-chat.md`.
pub const JACK_CHAT_PERSONA: &str = include_str!("../prompts/jack-chat.md");

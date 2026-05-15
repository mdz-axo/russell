// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell-meta` — the metacognitive layer.
//!
//! This crate is Russell's System 4 (Intelligence) in VSM terms:
//! the subsystem that reasons *about* the system, decides *when* to
//! consult the LLM, selects *which* knowledge to inject, interprets
//! *what* the LLM proposes, and closes the feedback loop between
//! observation and action.
//!
//! ## Responsibilities
//!
//! - **Prompt composition** — template-driven SOAP assembly with
//!   relevance-scored knowledge injection and token budgeting.
//! - **LLM client abstraction** — Okapi routing, inference hint
//!   application, model resolution.
//! - **Action resolution** — parsing ACTION syntax from LLM output,
//!   dispatching to skills and Kask tools.
//! - **Help orchestration** — the `russell jack` pipeline (compose →
//!   dispatch → journal → escalate).
//! - **Self-assessment** — quality scoring, outcome tracking, and
//!   the ability to criticize and adapt its own behavior on the fly.
//! - **Fallback reasoning** — rule-based logic when the LLM is
//!   unavailable.
//!
//! ## Naming
//!
//! Previously `russell-doctor`. Renamed per ADR-0027 to reflect that
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

// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell-meta` — metacognitive layer (the Nurse).
//!
//! Collects telemetry and calls LLM backends for inference.

#![deny(unsafe_code)]
#![deny(rust_2018_idioms)]
#![warn(missing_docs)]
#![allow(clippy::missing_docs_in_private_items)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::while_let_on_iterator)]

pub mod action;
pub mod client;
pub mod error;
pub mod fallback;
pub mod fallback_adapter;
pub mod health;
pub mod help;

pub mod mock;
pub mod oai_client;
pub mod okapi_adapter;
pub mod prompt;
pub mod prompt_registry;
pub mod rate_limit;

pub use client::{Backend, ClientConfig, EscalateMin, LlmClient, LlmResponse, SoapPrompt};
pub use error::{DoctorError, Result};
pub use fallback_adapter::FallbackInferenceAdapter;
pub use help::{HelpOutcome, run_help, run_help_with_endpoint};

pub use okapi_adapter::OkapiInferenceAdapter;

/// Jack's nurse persona prompt (loaded from `prompts/jack.md`).
pub const JACK_PERSONA: &str = include_str!("../prompts/jack.md");

/// The Jack chat persona — used in `russell chat` interactive mode.
pub const JACK_CHAT_PERSONA: &str = include_str!("../prompts/jack-chat.md");

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
pub mod health;
pub mod help;

pub use error::{DoctorError, Result};
pub use help::{HelpOutcome, run_help, run_help_with_endpoint};

/// Jack's nurse persona prompt (loaded from `prompts/jack.md`).
pub const JACK_PERSONA: &str = include_str!("../prompts/jack.md");
/// Jack's chat-mode persona prompt (loaded from `prompts/jack-chat.md`).
pub const JACK_CHAT_PERSONA: &str = include_str!("../prompts/jack-chat.md");

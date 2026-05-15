// SPDX-License-Identifier: MIT OR Apache-2.0
//! Consent handling for the chat REPL.
//!
//! Manages pending actions (probes/interventions) and operator
//! consent via natural language affirmation or refusal.

use russell_meta::action::ResolvedAction;

/// A pending action (probe or intervention) awaiting operator consent.
/// Derived from [`ResolvedAction`] with UI-specific fields.
#[derive(Debug, Clone)]
pub struct PendingAction {
    pub action: ResolvedAction,
    /// Optional stdin content to pipe to the subprocess (used by
    /// interventions like `create-manifest` where the LLM produces
    /// content that must be piped to the CLI command).
    pub stdin_content: Option<String>,
}

/// Returns true if the input looks like natural-language consent
/// ("ok", "yes", "do it", "go ahead", "sure", "yep", etc.).
pub fn is_affirmative(input: &str) -> bool {
    let lower = input.to_lowercase();
    let lower = lower.trim();
    matches!(
        lower,
        "ok" | "okay"
            | "yes"
            | "yep"
            | "yeah"
            | "yea"
            | "sure"
            | "do it"
            | "go ahead"
            | "go for it"
            | "approved"
            | "run it"
            | "execute"
            | "please"
            | "y"
            | "yes please"
            | "ok do it"
            | "lets go"
            | "let's go"
    )
}

/// Returns true if the input is a refusal.
pub fn is_refusal(input: &str) -> bool {
    let lower = input.trim();
    matches!(
        lower,
        "/deny"
            | "no"
            | "nope"
            | "cancel"
            | "nah"
            | "not now"
            | "later"
            | "hang on"
            | "wait"
            | "hold on"
    )
}

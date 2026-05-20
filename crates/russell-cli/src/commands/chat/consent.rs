// SPDX-License-Identifier: MIT OR Apache-2.0
//! Consent handling for the chat REPL.
//!
//! Manages pending actions (probes/interventions) and operator
//! consent via natural language affirmation or refusal.
//!
//! ## Security model (T5 hardening)
//!
//! - Pending actions expire after [`CONSENT_EXPIRY_SECS`] seconds
//!   of inactivity to prevent stale approvals.
//! - Consent phrases are checked against an exact allowlist (no
//!   substring matching) to prevent LLM output from triggering
//!   unintended approvals.
//! - The `/approve` slash command is the canonical consent form;
//!   natural-language phrases are accepted as a UX convenience
//!   but only from exact-match input lines.

use std::time::Instant;

use russell_meta::action::ResolvedAction;

/// Pending actions expire after this many seconds without operator
/// input. Prevents a stale intervention from executing long after
/// context has shifted.
pub const CONSENT_EXPIRY_SECS: u64 = 300; // 5 minutes

/// A pending action (probe or intervention) awaiting operator consent.
/// Derived from [`ResolvedAction`] with UI-specific fields.
#[derive(Debug, Clone)]
pub struct PendingAction {
    pub action: ResolvedAction,
    /// Optional stdin content to pipe to the subprocess (used by
    /// interventions like `create-manifest` where the LLM produces
    /// content that must be piped to the CLI command).
    pub stdin_content: Option<String>,
    /// When this action was proposed. Used for expiry checking.
    pub proposed_at: Instant,
}

impl PendingAction {
    /// Create a new pending action with the current timestamp.
    pub fn new(action: ResolvedAction, stdin_content: Option<String>) -> Self {
        Self {
            action,
            stdin_content,
            proposed_at: Instant::now(),
        }
    }

    /// Whether this pending action has expired.
    pub fn is_expired(&self) -> bool {
        self.proposed_at.elapsed().as_secs() >= CONSENT_EXPIRY_SECS
    }

    /// Human-readable description for confirmation display.
    pub fn describe(&self) -> String {
        format!("{}/{}", self.action.skill_id(), self.action.action_id())
    }
}

/// Returns true if the input looks like natural-language consent
/// ("ok", "yes", "do it", "go ahead", "sure", "yep", etc.).
///
/// These are whole-line exact matches only — never substring or
/// partial matching, to prevent LLM-generated text from triggering
/// consent inadvertently.
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

// SPDX-License-Identifier: MIT OR Apache-2.0
//! Risk level for reflex actions.

/// Risk level for reflex actions.
///
/// Determines the approval workflow:
/// - **Low:** Auto-execute with operator pre-approval
/// - **Medium:** Require explicit operator consent
/// - **High:** Require explicit approval + rollback plan
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskLevel {
    /// Safe to auto-execute (no state mutation).
    Low,
    /// Requires operator consent.
    Medium,
    /// Requires explicit approval + rollback plan.
    High,
}

impl std::fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
        }
    }
}

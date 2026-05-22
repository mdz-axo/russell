// SPDX-License-Identifier: MIT OR Apache-2.0
//! Pod lifecycle state machine.

use serde::{Deserialize, Serialize};
use crate::persona::PersonaError;

/// Pod lifecycle states.
///
/// State machine: — → Populated → Registered → Activated → Deactivated —→
///
/// # State Transitions
///
/// | From | To | Description |
/// |------|-----|-------------|
/// | — | Populated | Crate loaded, persona validated |
/// | Populated | Registered | ACP runtime registration complete |
/// | Registered | Activated | Sentinel running, ACP serving |
/// | Activated | Deactivated | Capabilities revoked, cleanup pending |
///
/// # Invalid Transitions
///
/// The following transitions are invalid and will return an error:
/// - Populated → Activated (must register first)
/// - Registered → Deactivated (must activate first)
/// - Deactivated → * (terminal state)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PodLifecycleState {
    /// Crate loaded, persona validated
    Populated,
    /// ACP runtime registration complete
    Registered,
    /// Sentinel running, ACP serving
    Activated,
    /// Capabilities revoked, cleanup pending
    Deactivated,
}

impl std::fmt::Display for PodLifecycleState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Populated => write!(f, "populated"),
            Self::Registered => write!(f, "registered"),
            Self::Activated => write!(f, "activated"),
            Self::Deactivated => write!(f, "deactivated"),
        }
    }
}

/// Lifecycle errors.
#[derive(Debug, thiserror::Error)]
pub enum LifecycleError {
    /// Invalid state transition
    #[error("invalid state transition: {from} → {to}")]
    InvalidStateTransition {
        /// Source state
        from: PodLifecycleState,
        /// Target state
        to: PodLifecycleState,
    },
    
    /// ACP registration failed
    #[error("ACP registration failed: {0}")]
    AcpRegistrationError(String),
    
    /// Persona error
    #[error("persona error: {0}")]
    PersonaError(#[from] PersonaError),
    
    /// Sentinel error
    #[error("sentinel error: {0}")]
    SentinelError(String),
    
    /// ACP server error
    #[error("ACP server error: {0}")]
    AcpServerError(String),
}

/// Lifecycle result type alias.
pub type LifecycleResult<T> = Result<T, LifecycleError>;

/// Validate state transition.
///
/// # Valid Transitions
///
/// - Populated → Registered
/// - Registered → Activated
/// - Activated → Deactivated
///
/// # Invalid Transitions
///
/// - Populated → Activated (must register first)
/// - Populated → Deactivated (must register and activate first)
/// - Registered → Deactivated (must activate first)
/// - Registered → Populated (cannot go backwards)
/// - Activated → Populated (cannot go backwards)
/// - Activated → Registered (cannot go backwards)
/// - Deactivated → * (terminal state)
///
/// # Examples
///
/// ```
/// use russell_agent::lifecycle::{validate_transition, PodLifecycleState};
///
/// // Valid transition
/// assert!(validate_transition(&PodLifecycleState::Populated, &PodLifecycleState::Registered).is_ok());
///
/// // Invalid transition
/// assert!(validate_transition(&PodLifecycleState::Populated, &PodLifecycleState::Activated).is_err());
/// ```
pub fn validate_transition(from: &PodLifecycleState, to: &PodLifecycleState) -> LifecycleResult<()> {
    let valid = match (from, to) {
        (PodLifecycleState::Populated, PodLifecycleState::Registered) => true,
        (PodLifecycleState::Registered, PodLifecycleState::Activated) => true,
        (PodLifecycleState::Activated, PodLifecycleState::Deactivated) => true,
        _ => false,
    };
    
    if !valid {
        Err(LifecycleError::InvalidStateTransition {
            from: *from,
            to: *to,
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_valid_transitions() {
        // Populated → Registered
        assert!(validate_transition(&PodLifecycleState::Populated, &PodLifecycleState::Registered).is_ok());
        
        // Registered → Activated
        assert!(validate_transition(&PodLifecycleState::Registered, &PodLifecycleState::Activated).is_ok());
        
        // Activated → Deactivated
        assert!(validate_transition(&PodLifecycleState::Activated, &PodLifecycleState::Deactivated).is_ok());
    }
    
    #[test]
    fn test_invalid_transitions() {
        // Cannot skip states
        assert!(validate_transition(&PodLifecycleState::Populated, &PodLifecycleState::Activated).is_err());
        assert!(validate_transition(&PodLifecycleState::Populated, &PodLifecycleState::Deactivated).is_err());
        assert!(validate_transition(&PodLifecycleState::Registered, &PodLifecycleState::Deactivated).is_err());
        
        // Cannot go backwards
        assert!(validate_transition(&PodLifecycleState::Registered, &PodLifecycleState::Populated).is_err());
        assert!(validate_transition(&PodLifecycleState::Activated, &PodLifecycleState::Registered).is_err());
        assert!(validate_transition(&PodLifecycleState::Activated, &PodLifecycleState::Populated).is_err());
        assert!(validate_transition(&PodLifecycleState::Deactivated, &PodLifecycleState::Activated).is_err());
        
        // Deactivated is terminal
        assert!(validate_transition(&PodLifecycleState::Deactivated, &PodLifecycleState::Populated).is_err());
        assert!(validate_transition(&PodLifecycleState::Deactivated, &PodLifecycleState::Registered).is_err());
        assert!(validate_transition(&PodLifecycleState::Deactivated, &PodLifecycleState::Activated).is_err());
    }
    
    #[test]
    fn test_same_state_transition() {
        // Transitioning to same state is invalid
        assert!(validate_transition(&PodLifecycleState::Populated, &PodLifecycleState::Populated).is_err());
        assert!(validate_transition(&PodLifecycleState::Registered, &PodLifecycleState::Registered).is_err());
        assert!(validate_transition(&PodLifecycleState::Activated, &PodLifecycleState::Activated).is_err());
        assert!(validate_transition(&PodLifecycleState::Deactivated, &PodLifecycleState::Deactivated).is_err());
    }
    
    #[test]
    fn test_state_display() {
        assert_eq!(PodLifecycleState::Populated.to_string(), "populated");
        assert_eq!(PodLifecycleState::Registered.to_string(), "registered");
        assert_eq!(PodLifecycleState::Activated.to_string(), "activated");
        assert_eq!(PodLifecycleState::Deactivated.to_string(), "deactivated");
    }
}

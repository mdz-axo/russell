// SPDX-License-Identifier: MIT OR Apache-2.0
//! Identity port — unified authentication abstraction.
//!
//! Provides a common interface for different authentication systems
//! (macaroon OCAP, ACP capability tokens, etc.) following hexagonal
//! architecture principles.
//!
//! See [ADR-0026](../../../docs/adr/0026-macaroon-ocap.md).

/// Unified identity port for authentication and authorization.
///
/// Implementations provide principal identification and capability checking
/// regardless of the underlying token format (macaroon, JWT, etc.).
pub trait IdentityPort: Send + Sync {
    /// Get the principal identifier (e.g., token ID, WebID, user ID).
    fn principal_id(&self) -> &str;

    /// Check if this identity has a specific capability.
    ///
    /// Capability strings follow the format `"domain:action"` (e.g., `"acp:session"`,
    /// `"tool:inference"`).
    fn has_capability(&self, capability: &str) -> bool;

    /// Get all capabilities granted to this identity.
    fn capabilities(&self) -> Vec<String>;

    /// Check if the identity is still valid (not expired).
    fn is_valid(&self) -> bool;
}

/// A simple identity implementation for testing and dev mode.
#[derive(Debug, Clone)]
pub struct SimpleIdentity {
    /// Principal identifier.
    pub principal_id: String,
    /// Granted capabilities.
    pub capabilities: Vec<String>,
}

impl SimpleIdentity {
    /// Create a new simple identity.
    pub fn new(principal_id: impl Into<String>, capabilities: Vec<String>) -> Self {
        Self {
            principal_id: principal_id.into(),
            capabilities,
        }
    }

    /// Create an anonymous identity with no capabilities.
    pub fn anonymous() -> Self {
        Self {
            principal_id: "anonymous".to_string(),
            capabilities: Vec::new(),
        }
    }
}

impl IdentityPort for SimpleIdentity {
    fn principal_id(&self) -> &str {
        &self.principal_id
    }

    fn has_capability(&self, capability: &str) -> bool {
        self.capabilities.iter().any(|c| c == capability)
    }

    fn capabilities(&self) -> Vec<String> {
        self.capabilities.clone()
    }

    fn is_valid(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_identity() {
        let identity = SimpleIdentity::new("test-user", vec!["acp:session".to_string()]);
        assert_eq!(identity.principal_id(), "test-user");
        assert!(identity.has_capability("acp:session"));
        assert!(!identity.has_capability("acp:admin"));
        assert!(identity.is_valid());
    }

    #[test]
    fn test_anonymous_identity() {
        let identity = SimpleIdentity::anonymous();
        assert_eq!(identity.principal_id(), "anonymous");
        assert!(!identity.has_capability("acp:session"));
        assert!(identity.capabilities().is_empty());
    }
}

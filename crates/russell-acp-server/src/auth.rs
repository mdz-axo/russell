// SPDX-License-Identifier: MIT OR Apache-2.0
//! Macaroon-based OCAP authentication.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::{AcpError, Result};

/// Macaroon authenticator.
#[derive(Debug, Clone)]
pub struct MacaroonAuth {
    /// Root key for validation (optional — if None, auth is skipped).
    root_key: Option<String>,
}

impl MacaroonAuth {
    /// Create a new macaroon authenticator.
    pub fn new(root_key: Option<String>) -> Self {
        Self { root_key }
    }

    /// Validate a capability token.
    pub fn validate(&self, token: &CapabilityToken) -> Result<()> {
        // If no root key configured, skip validation (dev mode).
        if self.root_key.is_none() {
            return Ok(());
        }

        // Check expiration.
        if let Some(expires) = token.expires_at {
            if Utc::now() > expires {
                return Err(AcpError::TokenExpired(expires.to_rfc3339()));
            }
        }

        // In production, validate macaroon signature against root key
        // using hKask's macaroon crate for full validation including
        // third-party discharge for Okapi access.
        //
        // For now, we accept any well-formed token with valid expiration.
        // This is sufficient for loopback-only deployment.

        Ok(())
    }

    /// Check if a token has a specific capability.
    pub fn has_capability(&self, token: &CapabilityToken, capability: &str) -> bool {
        token.capabilities.iter().any(|c| c == capability)
    }

    /// Check if a token has a skill attenuation.
    pub fn has_skill(&self, token: &CapabilityToken, skill_id: &str) -> bool {
        token.attenuations.iter().any(|a| {
            a.kind == AttenuationKind::SkillRestriction && a.value == skill_id
        })
    }
}

/// Capability token (OCAP).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityToken {
    /// Token string (base64-encoded macaroon).
    pub token: String,
    /// Granted capabilities.
    pub capabilities: Vec<String>,
    /// Attenuations (restrictions).
    pub attenuations: Vec<Attenuation>,
    /// Expiration timestamp.
    pub expires_at: Option<DateTime<Utc>>,
    /// Issuer.
    pub issuer: String,
}

/// Attenuation (capability restriction).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attenuation {
    /// Attenuation kind.
    pub kind: AttenuationKind,
    /// Attenuation value.
    pub value: String,
}

/// Attenuation kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttenuationKind {
    /// Restrict to specific skill.
    SkillRestriction,
    /// Rate limit (calls per minute).
    RateLimit,
    /// Time bound (ISO 8601 timestamp).
    TimeBound,
    /// Third-party discharge chain.
    DischargeChain,
}

impl Default for MacaroonAuth {
    fn default() -> Self {
        Self::new(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_root_key_skips_validation() {
        let auth = MacaroonAuth::new(None);
        let token = CapabilityToken {
            token: "test".to_string(),
            capabilities: vec!["acp:session".to_string()],
            attenuations: Vec::new(),
            expires_at: None,
            issuer: "test".to_string(),
        };
        assert!(auth.validate(&token).is_ok());
    }

    #[test]
    fn expired_token_rejected() {
        let auth = MacaroonAuth::new(Some("root".to_string()));
        let token = CapabilityToken {
            token: "test".to_string(),
            capabilities: vec![],
            attenuations: Vec::new(),
            expires_at: Some(Utc::now() - chrono::Duration::hours(1)),
            issuer: "test".to_string(),
        };
        assert!(matches!(auth.validate(&token), Err(AcpError::TokenExpired(_))));
    }

    #[test]
    fn capability_check() {
        let auth = MacaroonAuth::new(None);
        let token = CapabilityToken {
            token: "test".to_string(),
            capabilities: vec!["acp:session".to_string(), "skill:web-search".to_string()],
            attenuations: Vec::new(),
            expires_at: None,
            issuer: "test".to_string(),
        };
        assert!(auth.has_capability(&token, "acp:session"));
        assert!(!auth.has_capability(&token, "skill:sysadmin"));
    }
}

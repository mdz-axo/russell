// SPDX-License-Identifier: MIT OR Apache-2.0
//! Macaroon-based OCAP authentication.
//!
//! Implements capability security per Mark Miller's design:
//! - Unforgeable tokens via HMAC-SHA256 signatures
//! - Attenuation via caveats (restrictions)
//! - Delegation via discharge macaroons
//!
//! See [ADR-0026](../../../docs/adr/0026-macaroon-ocap.md).

use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac as MacTrait};
use sha2::Sha256;
use serde::{Deserialize, Serialize};

use crate::error::{AcpError, Result};

type HmacSha256 = Hmac<Sha256>;

/// Macaroon authenticator.
#[derive(Debug, Clone)]
pub struct MacaroonAuth {
    /// Root key for validation (optional — if None, auth is skipped).
    root_key: Option<Vec<u8>>,
}

impl MacaroonAuth {
    /// Create a new macaroon authenticator.
    pub fn new(root_key: Option<String>) -> Self {
        Self {
            root_key: root_key.map(|k| k.into_bytes()),
        }
    }

    /// Create a new macaroon token with given capabilities.
    pub fn create_token(
        &self,
        capabilities: Vec<String>,
        attenuations: Vec<Attenuation>,
        expires_in: Option<chrono::Duration>,
    ) -> Result<CapabilityToken> {
        let expires_at = expires_in.map(|d| Utc::now() + d);
        let issuer = "russell-acp".to_string();

        // Build macaroon identifier
        let identifier = format!(
            "capabilities:{}|issuer:{}|expires:{}",
            capabilities.join(","),
            issuer,
            expires_at
                .map(|e| e.to_rfc3339())
                .unwrap_or_else(|| "never".to_string())
        );

        // Generate macaroon signature
        let signature = if let Some(ref root_key) = self.root_key {
            let mut mac = HmacSha256::new_from_slice(root_key)
                .map_err(|_| AcpError::Internal("invalid root key length".into()))?;
            mac.update(identifier.as_bytes());
            for attenuation in &attenuations {
                mac.update(attenuation.kind.as_str().as_bytes());
                mac.update(attenuation.value.as_bytes());
            }
            let result = mac.finalize();
            base64::encode(result.into_bytes())
        } else {
            // Dev mode: no signature
            base64::encode(&identifier.as_bytes())
        };

        Ok(CapabilityToken {
            token: signature,
            capabilities,
            attenuations,
            expires_at,
            issuer,
        })
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

        // Verify macaroon signature.
        let root_key = self
            .root_key
            .as_ref()
            .ok_or_else(|| AcpError::Internal("root key not configured".into()))?;

        // Rebuild the signed message
        let identifier = format!(
            "capabilities:{}|issuer:{}|expires:{}",
            token.capabilities.join(","),
            token.issuer,
            token
                .expires_at
                .map(|e| e.to_rfc3339())
                .unwrap_or_else(|| "never".to_string())
        );

        let expected_signature = {
            let mut mac = HmacSha256::new_from_slice(root_key)
                .map_err(|_| AcpError::Internal("invalid root key length".into()))?;
            mac.update(identifier.as_bytes());
            for attenuation in &token.attenuations {
                mac.update(attenuation.kind.as_str().as_bytes());
                mac.update(attenuation.value.as_bytes());
            }
            let result = mac.finalize();
            base64::encode(result.into_bytes())
        };

        // Constant-time comparison to prevent timing attacks
        if !constant_time_eq(token.token.as_bytes(), expected_signature.as_bytes()) {
            return Err(AcpError::InvalidToken("macaroon signature mismatch".into()));
        }

        Ok(())
    }

    /// Check if a token has a specific capability.
    pub fn has_capability(&self, token: &CapabilityToken, capability: &str) -> bool {
        token.capabilities.iter().any(|c| c == capability)
    }

    /// Check if a token has a skill attenuation.
    pub fn has_skill(&self, token: &CapabilityToken, skill_id: &str) -> bool {
        token
            .attenuations
            .iter()
            .any(|a| a.kind == AttenuationKind::SkillRestriction && a.value == skill_id)
    }

    /// Get the rate limit from token attenuations (calls per minute).
    pub fn get_rate_limit(&self, token: &CapabilityToken) -> Option<u32> {
        token
            .attenuations
            .iter()
            .find(|a| a.kind == AttenuationKind::RateLimit)
            .and_then(|a| a.value.parse().ok())
    }
}

/// Constant-time equality check to prevent timing attacks.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .fold(0, |acc, (x, y)| acc | (x ^ y))
        == 0
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

impl AttenuationKind {
    /// Get string representation for signing.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SkillRestriction => "skill_restriction",
            Self::RateLimit => "rate_limit",
            Self::TimeBound => "time_bound",
            Self::DischargeChain => "discharge_chain",
        }
    }
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
        assert!(matches!(
            auth.validate(&token),
            Err(AcpError::TokenExpired(_))
        ));
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

    #[test]
    fn create_and_validate_token() {
        let auth = MacaroonAuth::new(Some("test-root-key".to_string()));

        let token = auth
            .create_token(
                vec!["acp:session".to_string(), "skill:okapi-watcher".to_string()],
                vec![Attenuation {
                    kind: AttenuationKind::RateLimit,
                    value: "60".to_string(),
                }],
                Some(chrono::Duration::hours(1)),
            )
            .unwrap();

        // Token should validate
        assert!(auth.validate(&token).is_ok());

        // Token should have capabilities
        assert!(auth.has_capability(&token, "acp:session"));
        assert!(auth.has_capability(&token, "skill:okapi-watcher"));

        // Token should have rate limit
        assert_eq!(auth.get_rate_limit(&token), Some(60));
    }

    #[test]
    fn tampered_token_rejected() {
        let auth = MacaroonAuth::new(Some("test-root-key".to_string()));

        let token = auth
            .create_token(
                vec!["acp:session".to_string()],
                vec![],
                Some(chrono::Duration::hours(1)),
            )
            .unwrap();

        // Tamper with the token
        let mut tampered = token.clone();
        tampered.token = "tampered-signature".to_string();

        // Should fail validation
        assert!(matches!(
            auth.validate(&tampered),
            Err(AcpError::InvalidToken(_))
        ));
    }

    #[test]
    fn rate_limit_attenuation() {
        let auth = MacaroonAuth::new(Some("root".to_string()));

        let token = auth
            .create_token(
                vec!["acp:session".to_string()],
                vec![
                    Attenuation {
                        kind: AttenuationKind::RateLimit,
                        value: "100".to_string(),
                    },
                    Attenuation {
                        kind: AttenuationKind::SkillRestriction,
                        value: "okapi-watcher".to_string(),
                    },
                ],
                None,
            )
            .unwrap();

        assert_eq!(auth.get_rate_limit(&token), Some(100));
        assert!(auth.has_skill(&token, "okapi-watcher"));
    }

    #[test]
    fn constant_time_eq_works() {
        assert!(constant_time_eq(b"hello", b"hello"));
        assert!(!constant_time_eq(b"hello", b"world"));
        assert!(!constant_time_eq(b"short", b"longer"));
    }
}

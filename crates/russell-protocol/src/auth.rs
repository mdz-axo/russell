// SPDX-License-Identifier: MIT OR Apache-2.0
//! Capability token and attenuation types for OCAP authentication.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Capability token (OCAP).
///
/// Unforgeable token implementing capability security per Mark Miller's design.
/// Signed with HMAC-SHA256 for integrity; attenuations restrict the token's
/// authority without requiring re-issuance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityToken {
    /// Unique token ID (for revocation tracking).
    pub token_id: String,
    /// Token string (base64-encoded macaroon signature).
    pub token: String,
    /// Granted capabilities.
    pub capabilities: Vec<String>,
    /// Attenuations (restrictions).
    pub attenuations: Vec<Attenuation>,
    /// Expiration timestamp.
    pub expires_at: Option<DateTime<Utc>>,
    /// Issuer.
    pub issuer: String,
    /// Nonce for replay protection.
    pub nonce: String,
}

impl CapabilityToken {
    /// Check if the token has expired.
    pub fn is_expired(&self) -> bool {
        match self.expires_at {
            Some(expires) => Utc::now() > expires,
            None => false,
        }
    }

    /// Check if the token grants a specific capability.
    pub fn has_capability(&self, capability: &str) -> bool {
        self.capabilities.iter().any(|c| c == capability)
    }
}

/// Attenuation (capability restriction).
///
/// Attenuations reduce the authority of a capability token. They are
/// bound into the macaroon signature and cannot be removed without
/// invalidating the token.
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

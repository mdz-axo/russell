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

#[cfg(test)]
mod tests {
    use super::*;

    // REQ: CapabilityToken must round-trip through serialization.
    #[test]
    fn capability_token_round_trip() {
        let token = CapabilityToken {
            token_id: "tok-1".to_string(),
            token: "c2lnbmF0dXJl".to_string(),
            capabilities: vec![
                "acp/capabilities".to_string(),
                "acp/session.create".to_string(),
            ],
            attenuations: vec![],
            expires_at: None,
            issuer: "russell".to_string(),
            nonce: "nonce-abc".to_string(),
        };
        let json = serde_json::to_string(&token).unwrap();
        let back: CapabilityToken = serde_json::from_str(&json).unwrap();
        assert_eq!(back.token_id, "tok-1");
        assert_eq!(back.token, "c2lnbmF0dXJl");
        assert_eq!(back.capabilities.len(), 2);
        assert!(back.attenuations.is_empty());
        assert!(back.expires_at.is_none());
        assert_eq!(back.issuer, "russell");
        assert_eq!(back.nonce, "nonce-abc");
    }

    // REQ: CapabilityToken with attenuations must round-trip.
    #[test]
    fn capability_token_with_attenuation_round_trip() {
        let token = CapabilityToken {
            token_id: "tok-2".to_string(),
            token: "c2ln".to_string(),
            capabilities: vec!["acp/skill.run".to_string()],
            attenuations: vec![
                Attenuation {
                    kind: AttenuationKind::SkillRestriction,
                    value: "disk-check".to_string(),
                },
                Attenuation {
                    kind: AttenuationKind::RateLimit,
                    value: "10".to_string(),
                },
            ],
            expires_at: None,
            issuer: "russell".to_string(),
            nonce: "n2".to_string(),
        };
        let json = serde_json::to_string(&token).unwrap();
        let back: CapabilityToken = serde_json::from_str(&json).unwrap();
        assert_eq!(back.attenuations.len(), 2);
        assert_eq!(back.attenuations[0].kind, AttenuationKind::SkillRestriction);
        assert_eq!(back.attenuations[0].value, "disk-check");
        assert_eq!(back.attenuations[1].kind, AttenuationKind::RateLimit);
        assert_eq!(back.attenuations[1].value, "10");
    }

    // REQ: AttenuationKind serializes as snake_case.
    #[test]
    fn attenuation_kind_snake_case() {
        let kinds = vec![
            (AttenuationKind::SkillRestriction, "skill_restriction"),
            (AttenuationKind::RateLimit, "rate_limit"),
            (AttenuationKind::TimeBound, "time_bound"),
            (AttenuationKind::DischargeChain, "discharge_chain"),
        ];
        for (kind, expected) in kinds {
            let json = serde_json::to_string(&kind).unwrap();
            assert_eq!(json.trim_matches('"'), expected);
            let back: AttenuationKind = serde_json::from_str(&json).unwrap();
            assert_eq!(back, kind);
        }
    }

    // REQ: has_capability returns true for granted capability.
    #[test]
    fn has_capability_true() {
        let token = CapabilityToken {
            token_id: "t".to_string(),
            token: "x".to_string(),
            capabilities: vec!["acp/capabilities".to_string()],
            attenuations: vec![],
            expires_at: None,
            issuer: "russell".to_string(),
            nonce: "n".to_string(),
        };
        assert!(token.has_capability("acp/capabilities"));
    }

    // REQ: has_capability returns false for ungranted capability.
    #[test]
    fn has_capability_false() {
        let token = CapabilityToken {
            token_id: "t".to_string(),
            token: "x".to_string(),
            capabilities: vec!["acp/capabilities".to_string()],
            attenuations: vec![],
            expires_at: None,
            issuer: "russell".to_string(),
            nonce: "n".to_string(),
        };
        assert!(!token.has_capability("acp/session.create"));
    }

    // REQ: is_expired returns false when no expiration set.
    #[test]
    fn is_expired_no_expiry() {
        let token = CapabilityToken {
            token_id: "t".to_string(),
            token: "x".to_string(),
            capabilities: vec![],
            attenuations: vec![],
            expires_at: None,
            issuer: "russell".to_string(),
            nonce: "n".to_string(),
        };
        assert!(!token.is_expired());
    }

    // REQ: is_expired returns true for past expiry.
    #[test]
    fn is_expired_past() {
        let token = CapabilityToken {
            token_id: "t".to_string(),
            token: "x".to_string(),
            capabilities: vec![],
            attenuations: vec![],
            expires_at: Some(Utc::now() - chrono::Duration::hours(1)),
            issuer: "russell".to_string(),
            nonce: "n".to_string(),
        };
        assert!(token.is_expired());
    }

    // REQ: Missing required field on CapabilityToken causes deserialization error.
    #[test]
    fn capability_token_missing_token_id_fails() {
        let json =
            r#"{"token":"x","capabilities":[],"attenuations":[],"issuer":"russell","nonce":"n"}"#;
        let result = serde_json::from_str::<CapabilityToken>(json);
        assert!(result.is_err());
    }
}

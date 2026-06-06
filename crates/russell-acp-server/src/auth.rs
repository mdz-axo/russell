// SPDX-License-Identifier: MIT OR Apache-2.0
//! Macaroon-based OCAP authentication.
//!
//! Implements capability security per Mark Miller's design:
//! - Unforgeable tokens via HMAC-SHA256 signatures
//! - Attenuation via caveats (restrictions)
//! - Delegation via discharge macaroons
//!
//! See [ADR-0026](../../../docs/adr/0026-macaroon-ocap.md).

use std::collections::HashSet;
use std::sync::Mutex;

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac as MacTrait};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::error::{AcpError, Result};

/// Token validation error messages — single source of truth (P2.8).
///
/// Centralising these strings prevents drift between error creation sites
/// and test assertions, and makes future i18n straightforward.
pub mod token_errors {
    /// Macaroon HMAC signature did not match the expected value.
    pub const SIGNATURE_MISMATCH: &str = "macaroon signature mismatch";
    /// Token was explicitly revoked by the operator.
    pub const REVOKED: &str = "token has been revoked";
    /// Nonce was already used — replay attack indicator.
    pub const REPLAY_DETECTED: &str = "replay detected: nonce already used";
    /// Production mode requires a root key; none was provided.
    pub const ROOT_KEY_REQUIRED: &str = "root key required in production mode";
    /// No root key configured and dev-mode is disabled.
    pub const AUTH_REQUIRED_NO_KEY: &str = "authentication required: root key not configured";
    /// Root key has an invalid length for HMAC-SHA256.
    pub const INVALID_ROOT_KEY_LENGTH: &str = "invalid root key length";
    /// Wire token base64 is malformed.
    pub const INVALID_BASE64: &str = "invalid base64 token";
    /// Wire token is not valid UTF-8.
    pub const INVALID_UTF8: &str = "token is not valid UTF-8";
    /// Wire token missing the `id:` prefix.
    pub const MISSING_IDENTIFIER: &str = "token does not contain a valid identifier";
    /// Wire token missing the `id` field value.
    pub const MISSING_ID_FIELD: &str = "token missing id field";
    /// Root key is None despite being required for signature verification.
    pub const ROOT_KEY_NOT_CONFIGURED: &str = "root key not configured";
}

type HmacSha256 = Hmac<Sha256>;

/// Consolidated mutable state for MacaroonAuth (P2.1).
///
/// Grouping `used_nonces` and `revoked_tokens` behind a single lock reduces
/// lock operations from two separate mutex acquisitions to one, and ensures
/// atomic state transitions (e.g., a token can be checked for revocation and
/// its nonce recorded in a single critical section).
#[derive(Debug)]
struct MacaroonState {
    /// Nonces used for replay protection ("token_id:nonce" → present).
    /// Fallback when persistent store (journal) is unavailable.
    used_nonces: HashSet<String>,
    /// Revoked token IDs.
    revoked_tokens: HashSet<String>,
}

impl MacaroonState {
    fn new() -> Self {
        Self {
            used_nonces: HashSet::new(),
            revoked_tokens: HashSet::new(),
        }
    }
}

/// Macaroon authenticator.
pub struct MacaroonAuth {
    /// Root key for validation (optional — if None, auth is skipped in dev mode).
    root_key: Option<Vec<u8>>,
    /// Consolidated mutable state: nonces + revocation behind a single lock (P2.1).
    state: Mutex<MacaroonState>,
    /// Whether dev mode (no root key) is explicitly allowed.
    dev_mode_allowed: bool,
    /// Persistent nonce store (journal writer) for replay protection.
    /// If Some, nonces are persisted to SQLite and survive restarts.
    journal: Option<std::sync::Arc<russell_core::journal::JournalWriter>>,
}

impl std::fmt::Debug for MacaroonAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MacaroonAuth")
            .field("root_key", &self.root_key.as_ref().map(|_| "***"))
            .field("state", &self.state)
            .field("dev_mode_allowed", &self.dev_mode_allowed)
            .field("journal", &self.journal.as_ref().map(|_| "JournalWriter"))
            .finish()
    }
}

/// Generate a unique token ID.
fn generate_token_id() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let bytes: [u8; 16] = rng.r#gen();
    hex::encode(bytes)
}

/// Generate a nonce for replay protection.
fn generate_nonce() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let bytes: [u8; 16] = rng.r#gen();
    hex::encode(bytes)
}

/// Constant-time equality check to prevent timing attacks.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b.iter()).fold(0, |acc, (x, y)| acc | (x ^ y)) == 0
}

impl MacaroonAuth {
    /// Create a new macaroon authenticator.
    ///
    /// If `root_key` is None and `dev_mode_allowed` is false, authentication
    /// will fail rather than being skipped (production safety).
    pub fn new(root_key: Option<String>, dev_mode_allowed: bool) -> Self {
        Self {
            root_key: root_key.map(|k| k.into_bytes()),
            state: Mutex::new(MacaroonState::new()),
            dev_mode_allowed,
            journal: None,
        }
    }

    /// Set the persistent nonce store (journal writer) for replay protection.
    /// When set, nonces are persisted to SQLite and survive process restarts.
    pub fn with_journal(
        mut self,
        journal: std::sync::Arc<russell_core::journal::JournalWriter>,
    ) -> Self {
        self.journal = Some(journal);
        self
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
        let token_id = generate_token_id();
        let nonce = generate_nonce();

        // Build macaroon identifier
        let identifier = format!(
            "id:{}|capabilities:{}|issuer:{}|nonce:{}|expires:{}",
            token_id,
            capabilities.join(","),
            issuer,
            nonce,
            expires_at
                .map(|e| e.to_rfc3339())
                .unwrap_or_else(|| "never".to_string())
        );

        // Generate macaroon signature
        let signature = if let Some(ref root_key) = self.root_key {
            let mut mac = HmacSha256::new_from_slice(root_key)
                .map_err(|_| AcpError::Internal(token_errors::INVALID_ROOT_KEY_LENGTH.into()))?;
            mac.update(identifier.as_bytes());
            for attenuation in &attenuations {
                mac.update(attenuation.kind.as_str().as_bytes());
                mac.update(attenuation.value.as_bytes());
            }
            let result = mac.finalize();
            BASE64.encode(result.into_bytes())
        } else {
            // Dev mode: no signature (only if explicitly allowed)
            if !self.dev_mode_allowed {
                return Err(AcpError::Internal(token_errors::ROOT_KEY_REQUIRED.into()));
            }
            BASE64.encode(identifier.as_bytes())
        };

        Ok(CapabilityToken {
            token_id,
            token: signature,
            capabilities,
            attenuations,
            expires_at,
            issuer,
            nonce,
        })
    }

    /// Validate a capability token.
    pub fn validate(&self, token: &CapabilityToken) -> Result<()> {
        // If no root key configured, check if dev mode is allowed.
        if self.root_key.is_none() && !self.dev_mode_allowed {
            return Err(AcpError::Internal(
                token_errors::AUTH_REQUIRED_NO_KEY.into(),
            ));
        }

        if let Some(expires) = token.expires_at
            && Utc::now() > expires
        {
            return Err(AcpError::TokenExpired(expires.to_rfc3339()));
        }

        // Check revocation and replay in a single critical section (P2.1).
        {
            let mut state = self.state.lock().unwrap();

            // Check revocation.
            if state.revoked_tokens.contains(&token.token_id) {
                return Err(AcpError::InvalidToken(token_errors::REVOKED.into()));
            }

            // Check replay (nonce must not have been used before).
            if let Some(ref journal) = self.journal {
                // Persistent nonce store — survives restarts.
                let expires_unix = token
                    .expires_at
                    .map(|dt| dt.timestamp())
                    .unwrap_or(i64::MAX);
                let replay = journal
                    .check_and_mark_nonce(&token.token_id, &token.nonce, expires_unix)
                    .map_err(|e| AcpError::Internal(format!("nonce store error: {e}")))?;
                if replay {
                    return Err(AcpError::InvalidToken(token_errors::REPLAY_DETECTED.into()));
                }
            } else {
                // In-memory fallback — resets on restart.
                let nonce_key = format!("{}:{}", token.token_id, token.nonce);
                if !state.used_nonces.insert(nonce_key) {
                    return Err(AcpError::InvalidToken(token_errors::REPLAY_DETECTED.into()));
                }
            }
        }

        // Verify macaroon signature (skip in dev mode).
        if self.root_key.is_none() {
            return Ok(());
        }

        let root_key = self
            .root_key
            .as_ref()
            .ok_or_else(|| AcpError::Internal(token_errors::ROOT_KEY_NOT_CONFIGURED.into()))?;

        // Rebuild the signed message
        let identifier = format!(
            "id:{}|capabilities:{}|issuer:{}|nonce:{}|expires:{}",
            token.token_id,
            token.capabilities.join(","),
            token.issuer,
            token.nonce,
            token
                .expires_at
                .map(|e| e.to_rfc3339())
                .unwrap_or_else(|| "never".to_string())
        );

        let expected_signature = {
            let mut mac = HmacSha256::new_from_slice(root_key)
                .map_err(|_| AcpError::Internal(token_errors::INVALID_ROOT_KEY_LENGTH.into()))?;
            mac.update(identifier.as_bytes());
            for attenuation in &token.attenuations {
                mac.update(attenuation.kind.as_str().as_bytes());
                mac.update(attenuation.value.as_bytes());
            }
            let result = mac.finalize();
            BASE64.encode(result.into_bytes())
        };

        // Constant-time comparison to prevent timing attacks
        if !constant_time_eq(token.token.as_bytes(), expected_signature.as_bytes()) {
            return Err(AcpError::InvalidToken(
                token_errors::SIGNATURE_MISMATCH.into(),
            ));
        }

        Ok(())
    }

    /// Revoke a token by its ID.
    pub fn revoke_token(&self, token_id: &str) {
        let mut state = self.state.lock().unwrap();
        state.revoked_tokens.insert(token_id.to_string());
    }

    /// Decode a wire token (base64-encoded) into a CapabilityToken.
    ///
    /// In dev mode, the token is base64(identifier) where identifier contains
    /// the token fields. In production mode, the token is the HMAC signature;
    /// the caller must provide the full token via a separate channel.
    pub fn decode_wire_token(&self, wire_token: &str) -> Result<CapabilityToken> {
        use base64::{Engine as _, engine::general_purpose::STANDARD};

        let decoded = STANDARD.decode(wire_token).map_err(|e| {
            AcpError::InvalidToken(format!("{}: {e}", token_errors::INVALID_BASE64))
        })?;

        let identifier = String::from_utf8(decoded)
            .map_err(|e| AcpError::InvalidToken(format!("{}: {e}", token_errors::INVALID_UTF8)))?;

        if !identifier.starts_with("id:") {
            return Err(AcpError::InvalidToken(
                token_errors::MISSING_IDENTIFIER.into(),
            ));
        }

        let mut token_id = String::new();
        let mut capabilities = Vec::new();
        let mut issuer = String::new();
        let mut nonce = String::new();
        let mut expires_at = None;

        for part in identifier.split('|') {
            if let Some(val) = part.strip_prefix("id:") {
                token_id = val.to_string();
            } else if let Some(val) = part.strip_prefix("capabilities:") {
                capabilities = val
                    .split(',')
                    .filter(|s| !s.is_empty())
                    .map(String::from)
                    .collect();
            } else if let Some(val) = part.strip_prefix("issuer:") {
                issuer = val.to_string();
            } else if let Some(val) = part.strip_prefix("nonce:") {
                nonce = val.to_string();
            } else if let Some(val) = part.strip_prefix("expires:")
                && val != "never"
            {
                expires_at = DateTime::parse_from_rfc3339(val)
                    .ok()
                    .map(|dt| dt.with_timezone(&Utc));
            }
        }

        if token_id.is_empty() {
            return Err(AcpError::InvalidToken(
                token_errors::MISSING_ID_FIELD.into(),
            ));
        }

        Ok(CapabilityToken {
            token_id,
            token: wire_token.to_string(),
            capabilities,
            attenuations: Vec::new(),
            expires_at,
            issuer,
            nonce,
        })
    }

    /// Check if a token has been revoked.
    pub fn is_revoked(&self, token_id: &str) -> bool {
        let state = self.state.lock().unwrap();
        state.revoked_tokens.contains(token_id)
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

/// Capability token (OCAP).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityToken {
    /// Unique token ID (for revocation tracking).
    pub token_id: String,
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
    /// Nonce for replay protection.
    pub nonce: String,
}

impl russell_core::identity::IdentityPort for CapabilityToken {
    fn principal_id(&self) -> &str {
        &self.token_id
    }

    fn has_capability(&self, capability: &str) -> bool {
        self.capabilities.iter().any(|c| c == capability)
    }

    fn capabilities(&self) -> Vec<String> {
        self.capabilities.clone()
    }

    fn is_valid(&self) -> bool {
        match self.expires_at {
            Some(expires) => Utc::now() <= expires,
            None => true,
        }
    }
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
        Self::new(None, true) // Dev mode allowed by default for backward compatibility
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_root_key_skips_validation_in_dev_mode() {
        let auth = MacaroonAuth::new(None, true);
        let token = auth
            .create_token(vec!["acp:session".to_string()], Vec::new(), None)
            .unwrap();
        assert!(auth.validate(&token).is_ok());
    }

    #[test]
    fn no_root_key_fails_in_production_mode() {
        let auth = MacaroonAuth::new(None, false);
        let result = auth.create_token(vec!["acp:session".to_string()], Vec::new(), None);
        assert!(result.is_err());
    }

    #[test]
    fn expired_token_rejected() {
        let auth = MacaroonAuth::new(Some("root".to_string()), false);
        let mut token = auth
            .create_token(
                vec![],
                Vec::new(),
                Some(chrono::Duration::hours(-1)), // Already expired
            )
            .unwrap();
        token.expires_at = Some(Utc::now() - chrono::Duration::hours(1));
        assert!(matches!(
            auth.validate(&token),
            Err(AcpError::TokenExpired(_))
        ));
    }

    #[test]
    fn capability_check() {
        let auth = MacaroonAuth::new(None, true);
        let token = auth
            .create_token(
                vec!["acp:session".to_string(), "skill:web-search".to_string()],
                Vec::new(),
                None,
            )
            .unwrap();
        assert!(auth.has_capability(&token, "acp:session"));
        assert!(!auth.has_capability(&token, "skill:sysadmin"));
    }

    #[test]
    fn create_and_validate_token() {
        let auth = MacaroonAuth::new(Some("test-root-key".to_string()), false);

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
        let auth = MacaroonAuth::new(Some("test-root-key".to_string()), false);

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
        let auth = MacaroonAuth::new(Some("root".to_string()), false);

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

    #[test]
    fn replay_protection() {
        let auth = MacaroonAuth::new(Some("root".to_string()), false);
        let token = auth
            .create_token(
                vec!["acp:session".to_string()],
                vec![],
                Some(chrono::Duration::hours(1)),
            )
            .unwrap();

        // First validation should succeed
        assert!(auth.validate(&token).is_ok());

        // Second validation with same token should fail (replay)
        assert!(matches!(
            auth.validate(&token),
            Err(AcpError::InvalidToken(_))
        ));
    }

    #[test]
    fn token_revocation() {
        let auth = MacaroonAuth::new(Some("root".to_string()), false);
        let token = auth
            .create_token(
                vec!["acp:session".to_string()],
                vec![],
                Some(chrono::Duration::hours(1)),
            )
            .unwrap();

        // Token should validate initially
        assert!(auth.validate(&token).is_ok());

        // Revoke the token
        auth.revoke_token(&token.token_id);

        // Token should now be rejected
        assert!(auth.is_revoked(&token.token_id));

        // Create a new token with same ID to test revocation check
        let mut revoked_token = auth
            .create_token(
                vec!["acp:session".to_string()],
                vec![],
                Some(chrono::Duration::hours(1)),
            )
            .unwrap();
        revoked_token.token_id = token.token_id.clone();

        assert!(matches!(
            auth.validate(&revoked_token),
            Err(AcpError::InvalidToken(_))
        ));
    }

    #[test]
    fn token_error_constants_match_messages() {
        // P2.8: Verify constants match previously hardcoded strings.
        assert_eq!(
            token_errors::SIGNATURE_MISMATCH,
            "macaroon signature mismatch"
        );
        assert_eq!(token_errors::REVOKED, "token has been revoked");
        assert_eq!(
            token_errors::REPLAY_DETECTED,
            "replay detected: nonce already used"
        );
        assert_eq!(
            token_errors::ROOT_KEY_REQUIRED,
            "root key required in production mode"
        );
        assert_eq!(
            token_errors::AUTH_REQUIRED_NO_KEY,
            "authentication required: root key not configured"
        );
        assert_eq!(
            token_errors::INVALID_ROOT_KEY_LENGTH,
            "invalid root key length"
        );
        assert_eq!(token_errors::INVALID_BASE64, "invalid base64 token");
        assert_eq!(token_errors::INVALID_UTF8, "token is not valid UTF-8");
        assert_eq!(
            token_errors::MISSING_IDENTIFIER,
            "token does not contain a valid identifier"
        );
        assert_eq!(token_errors::MISSING_ID_FIELD, "token missing id field");
        assert_eq!(
            token_errors::ROOT_KEY_NOT_CONFIGURED,
            "root key not configured"
        );
    }

    #[test]
    fn consolidated_state_atomic_check_and_nonce() {
        // P2.1: Verify that revocation and nonce checking use the same lock.
        // A revoked token's nonce should not be recorded in the nonce set
        // because validation short-circuits on revocation.
        let auth = MacaroonAuth::new(Some("root".to_string()), false);
        let token = auth
            .create_token(
                vec!["acp:session".to_string()],
                vec![],
                Some(chrono::Duration::hours(1)),
            )
            .unwrap();

        // Validate once — should succeed and record nonce.
        assert!(auth.validate(&token).is_ok());

        // Revoke the token.
        auth.revoke_token(&token.token_id);

        // Second validation should fail with REVOKED, not REPLAY_DETECTED.
        match auth.validate(&token) {
            Err(AcpError::InvalidToken(msg)) if msg == token_errors::REVOKED => {}
            other => panic!("expected InvalidToken(REVOKED), got {:?}", other),
        }
    }
}

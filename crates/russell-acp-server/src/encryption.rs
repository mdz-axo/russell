// SPDX-License-Identifier: MIT OR Apache-2.0
//! Token encryption at rest.
//!
//! Re-exports from `russell-core` for backward compatibility.
//! The canonical implementation lives in `russell_core::encryption`.

pub use russell_core::encryption::EncryptionKey;

use crate::auth::CapabilityToken;
use crate::error::{AcpError, Result};

/// Encrypt capability token JSON.
pub fn encrypt_token(token: &CapabilityToken, key: &EncryptionKey) -> Result<String> {
    let json = serde_json::to_vec(token).map_err(AcpError::Serialization)?;
    key.encrypt(&json)
        .map_err(|e| AcpError::Internal(format!("encryption failed: {e}")))
}

/// Decrypt capability token JSON.
pub fn decrypt_token(ciphertext: &str, key: &EncryptionKey) -> Result<CapabilityToken> {
    let plaintext = key
        .decrypt(ciphertext)
        .map_err(|e| AcpError::InvalidToken(format!("decryption failed: {e}")))?;
    serde_json::from_slice(&plaintext)
        .map_err(|e| AcpError::InvalidToken(format!("invalid token JSON: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};

    #[test]
    fn test_key_generation() {
        let key = EncryptionKey::generate();
        assert!(!key.to_encoded_string().is_empty());
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = EncryptionKey::generate();
        let plaintext = b"secret token data";

        let ciphertext = key.encrypt(plaintext).unwrap();
        let decrypted = key.decrypt(&ciphertext).unwrap();

        assert_eq!(plaintext, &decrypted[..]);
    }

    #[test]
    fn test_token_encryption() {
        let key = EncryptionKey::generate();

        let token = CapabilityToken {
            token_id: "test-token-id".to_string(),
            token: "macaroon-signature".to_string(),
            capabilities: vec!["acp:session".to_string()],
            attenuations: Vec::new(),
            expires_at: Some(Utc::now() + Duration::hours(1)),
            issuer: "test".to_string(),
            nonce: "test-nonce".to_string(),
        };

        let encrypted = encrypt_token(&token, &key).unwrap();
        let decrypted = decrypt_token(&encrypted, &key).unwrap();

        assert_eq!(token.capabilities, decrypted.capabilities);
        assert_eq!(token.issuer, decrypted.issuer);
    }

    #[test]
    fn test_key_export_import() {
        let key1 = EncryptionKey::generate();
        let encoded = key1.to_encoded_string();

        let key2 = EncryptionKey::from_string(&encoded).unwrap();

        let plaintext = b"test data";
        let ciphertext = key1.encrypt(plaintext).unwrap();
        let decrypted = key2.decrypt(&ciphertext).unwrap();

        assert_eq!(plaintext, &decrypted[..]);
    }

    #[test]
    fn test_invalid_ciphertext_rejected() {
        let key = EncryptionKey::generate();
        let result = key.decrypt("not-valid-base64!!!");
        assert!(result.is_err());
    }
}

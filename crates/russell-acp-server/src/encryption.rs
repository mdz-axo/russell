// SPDX-License-Identifier: MIT OR Apache-2.0
//! Token encryption at rest.
//!
//! Encrypts capability tokens when stored to prevent leakage from memory dumps
//! or disk forensics. Uses age encryption (modern, secure, simple).
//!
//! See [ADR-0027](../../../docs/adr/0027-token-encryption.md).

use age::{secrecy::Secret, Encryptor, Decryptor};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::io::{Read, Write};
use crate::error::{AcpError, Result};

/// Encryption key for token encryption.
pub struct EncryptionKey {
    /// Age secret key (X25519).
    secret_key: age::x25519::Identity,
}

impl EncryptionKey {
    /// Generate a new random encryption key.
    pub fn generate() -> Self {
        Self {
            secret_key: age::x25519::Identity::generate(),
        }
    }

    /// Load from a base64-encoded secret key string.
    pub fn from_base64(encoded: &str) -> Result<Self> {
        let decoded = STANDARD.decode(encoded)
            .map_err(|e| AcpError::Config(format!("invalid key encoding: {}", e)))?;
        
        let secret_key = age::x25519::Identity::from_slice(&decoded)
            .map_err(|e| AcpError::Config(format!("invalid key format: {}", e)))?;
        
        Ok(Self { secret_key })
    }

    /// Export as base64-encoded string (for storage).
    pub fn to_base64(&self) -> String {
        STANDARD.encode(self.secret_key.to_bytes())
    }

    /// Get the public key (for encryption).
    pub fn public_key(&self) -> age::x25519::Recipient {
        self.secret_key.to_public()
    }

    /// Encrypt a token.
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<String> {
        let mut encrypted = Vec::new();
        let recipient = self.public_key();
        
        let mut encryptor = Encryptor::with_recipients(std::iter::once(&recipient as _))
            .expect("failed to create encryptor");
        
        let mut writer = encryptor
            .wrap_output(&mut encrypted)
            .expect("failed to wrap output");
        
        writer.write_all(plaintext)
            .expect("failed to write encrypted data");
        writer.finish()
            .expect("failed to finish encryption");
        
        Ok(STANDARD.encode(encrypted))
    }

    /// Decrypt a token.
    pub fn decrypt(&self, ciphertext: &str) -> Result<Vec<u8>> {
        let decoded = STANDARD.decode(ciphertext)
            .map_err(|e| AcpError::InvalidToken(format!("invalid ciphertext encoding: {}", e)))?;
        
        let identity: &dyn age::Identity = &self.secret_key;
        
        let mut decryptor = Decryptor::new(&decoded[..])
            .map_err(|e| AcpError::InvalidToken(format!("decryption failed: {}", e)))?;
        
        let mut reader = decryptor
            .decrypt(std::iter::once(identity))
            .map_err(|e| AcpError::InvalidToken(format!("decryption failed: {}", e)))?;
        
        let mut plaintext = Vec::new();
        reader.read_to_end(&mut plaintext)
            .map_err(|e| AcpError::InvalidToken(format!("read failed: {}", e)))?;
        
        Ok(plaintext)
    }
}

/// Encrypt capability token JSON.
pub fn encrypt_token(token: &crate::auth::CapabilityToken, key: &EncryptionKey) -> Result<String> {
    let json = serde_json::to_vec(token)
        .map_err(|e| AcpError::Serialization(e))?;
    
    key.encrypt(&json)
}

/// Decrypt capability token JSON.
pub fn decrypt_token(ciphertext: &str, key: &EncryptionKey) -> Result<crate::auth::CapabilityToken> {
    let plaintext = key.decrypt(ciphertext)?;
    
    serde_json::from_slice(&plaintext)
        .map_err(|e| AcpError::InvalidToken(format!("invalid token JSON: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Utc, Duration};
    use crate::auth::CapabilityToken;

    #[test]
    fn test_key_generation() {
        let key = EncryptionKey::generate();
        assert!(!key.to_base64().is_empty());
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
            token: "macaroon-signature".to_string(),
            capabilities: vec!["acp:session".to_string()],
            attenuations: Vec::new(),
            expires_at: Some(Utc::now() + Duration::hours(1)),
            issuer: "test".to_string(),
        };
        
        let encrypted = encrypt_token(&token, &key).unwrap();
        let decrypted = decrypt_token(&encrypted, &key).unwrap();
        
        assert_eq!(token.capabilities, decrypted.capabilities);
        assert_eq!(token.issuer, decrypted.issuer);
    }

    #[test]
    fn test_key_export_import() {
        let key1 = EncryptionKey::generate();
        let encoded = key1.to_base64();
        
        let key2 = EncryptionKey::from_base64(&encoded).unwrap();
        
        // Both keys should decrypt the same ciphertext
        let plaintext = b"test data";
        let ciphertext = key1.encrypt(plaintext).unwrap();
        let decrypted = key2.decrypt(&ciphertext).unwrap();
        
        assert_eq!(plaintext, &decrypted[..]);
    }

    #[test]
    fn test_invalid_ciphertext_rejected() {
        let key = EncryptionKey::generate();
        
        // Tampered ciphertext should fail
        let result = key.decrypt("not-valid-base64!!!");
        assert!(result.is_err());
    }
}

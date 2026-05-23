// SPDX-License-Identifier: MIT OR Apache-2.0
//! Token encryption at rest.
//!
//! Encrypts capability tokens when stored to prevent leakage from memory dumps
//! or disk forensics. Uses age encryption (modern, secure, simple).
//!
//! See [ADR-0027](../../../docs/adr/0027-token-encryption.md).

use age::secrecy::ExposeSecret;
use age::{Decryptor, Encryptor};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::io::{Read, Write};

use crate::error::{CoreError, Result};

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

    /// Load from a string-encoded secret key (Bech32 format).
    pub fn from_string(s: &str) -> Result<Self> {
        let secret_key = s
            .parse::<age::x25519::Identity>()
            .map_err(|e| CoreError::Invariant(format!("invalid key format: {}", e)))?;

        Ok(Self { secret_key })
    }

    /// Export as string (Bech32 format for storage).
    pub fn to_encoded_string(&self) -> String {
        self.secret_key.to_string().expose_secret().to_string()
    }

    /// Get the public key (for encryption).
    pub fn public_key(&self) -> age::x25519::Recipient {
        self.secret_key.to_public()
    }

    /// Encrypt plaintext data.
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<String> {
        let mut encrypted = Vec::new();
        let recipient = self.public_key();

        let encryptor = Encryptor::with_recipients(std::iter::once(&recipient as _))
            .expect("failed to create encryptor");

        let mut writer = encryptor
            .wrap_output(&mut encrypted)
            .expect("failed to wrap output");

        writer
            .write_all(plaintext)
            .expect("failed to write encrypted data");
        writer.finish().expect("failed to finish encryption");

        Ok(STANDARD.encode(encrypted))
    }

    /// Decrypt ciphertext data.
    pub fn decrypt(&self, ciphertext: &str) -> Result<Vec<u8>> {
        let decoded = STANDARD
            .decode(ciphertext)
            .map_err(|e| CoreError::Invariant(format!("invalid ciphertext encoding: {}", e)))?;

        let identity: &dyn age::Identity = &self.secret_key;

        let decryptor = Decryptor::new(&decoded[..])
            .map_err(|e| CoreError::Invariant(format!("decryption failed: {}", e)))?;

        let mut reader = decryptor
            .decrypt(std::iter::once(identity))
            .map_err(|e| CoreError::Invariant(format!("decryption failed: {}", e)))?;

        let mut plaintext = Vec::new();
        reader
            .read_to_end(&mut plaintext)
            .map_err(|e| CoreError::Invariant(format!("read failed: {}", e)))?;

        Ok(plaintext)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_key_export_import() {
        let key1 = EncryptionKey::generate();
        let encoded = key1.to_encoded_string();

        let key2 = EncryptionKey::from_string(&encoded).unwrap();

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

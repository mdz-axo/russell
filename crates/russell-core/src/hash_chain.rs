// SPDX-License-Identifier: MIT OR Apache-2.0
//! Event integrity chain — SHA-256 hash linking (T6).
//!
//! Each event carries a `prev_hash` field linking it to its
//! predecessor. The hash covers `prev_hash || event_json`,
//! creating a tamper-evident chain. A broken link indicates
//! that an event was modified, deleted, or inserted after the
//! fact.
//!
//! ## Threat model
//!
//! This protects against:
//! - Silent deletion of journal events
//! - Retroactive modification of event fields
//! - Insertion of fabricated events into the middle of the chain
//!
//! It does NOT protect against:
//! - An attacker who can rewrite the entire chain from genesis
//! - An attacker with write access to the machine-id / seed
//!
//! ## Design
//!
//! - Genesis hash: SHA-256 of `/etc/machine-id` or a persisted random seed.
//! - Each event hash: `SHA-256(prev_hash_hex || event_json)`.
//! - Stored as 64-char lowercase hex string.
//! - Verification: walk the chain forwards, recomputing each
//!   hash and comparing against stored values.

use sha2::{Digest, Sha256};

/// Length of a SHA-256 hex digest.
pub const HASH_HEX_LEN: usize = 64;

/// Compute the genesis hash (chain seed).
///
/// Priority:
/// 1. `/etc/machine-id` — stable per-host identifier.
/// 2. `~/.local/state/harness/chain-seed` — generated random seed (persisted).
/// 3. Generate and persist a new random seed.
///
/// The old fallback constant has been removed — a known seed allows
/// an attacker to forge the entire hash chain (Schneier: never use
/// a known constant as a cryptographic seed).
#[must_use]
pub fn genesis_hash() -> String {
    let seed = resolve_genesis_seed();
    let mut hasher = Sha256::new();
    hasher.update(seed.trim().as_bytes());
    hex::encode(hasher.finalize())
}

fn resolve_genesis_seed() -> String {
    if let Ok(contents) = std::fs::read_to_string("/etc/machine-id") {
        let trimmed = contents.trim();
        if !trimmed.is_empty() {
            return trimmed.to_owned();
        }
    }

    let seed_path = seed_file_path();
    if let Ok(contents) = std::fs::read_to_string(&seed_path) {
        let trimmed = contents.trim();
        if !trimmed.is_empty() {
            return trimmed.to_owned();
        }
    }

    let seed = generate_random_seed();
    if let Some(parent) = seed_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let _ = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&seed_path)
            .and_then(|mut f| std::io::Write::write_all(&mut f, seed.as_bytes()));
    }
    #[cfg(not(unix))]
    {
        let _ = std::fs::write(&seed_path, &seed);
    }
    seed
}

fn seed_file_path() -> std::path::PathBuf {
    let base = std::env::var("XDG_STATE_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            std::env::var("HOME")
                .map(|h| std::path::PathBuf::from(h).join(".local/state"))
                .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"))
        });
    base.join("harness").join("chain-seed")
}

fn generate_random_seed() -> String {
    let mut buf = [0u8; 32];
    match getrandom::fill(&mut buf) {
        Ok(()) => hex::encode(buf),
        Err(_) => {
            tracing::warn!("getrandom failed; falling back to /etc/machine-id + timestamp mix");
            let mut fallback = String::new();
            if let Ok(mid) = std::fs::read_to_string("/etc/machine-id") {
                fallback.push_str(mid.trim());
            }
            fallback.push_str(&format!(
                ":{}:{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos(),
                std::process::id()
            ));
            let mut hasher = Sha256::new();
            hasher.update(fallback.as_bytes());
            hex::encode(hasher.finalize())
        }
    }
}

/// Compute the hash of an event given the previous hash and the
/// event's JSON representation.
///
/// `hash = SHA-256(prev_hash_hex || event_json_bytes)`
#[must_use]
pub fn compute_event_hash(prev_hash: &str, event_json: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prev_hash.as_bytes());
    hasher.update(event_json.as_bytes());
    hex::encode(hasher.finalize())
}

/// Verify a single link in the chain.
///
/// Returns `true` if `expected_hash == SHA-256(prev_hash || event_json)`.
#[must_use]
pub fn verify_link(prev_hash: &str, event_json: &str, expected_hash: &str) -> bool {
    compute_event_hash(prev_hash, event_json) == expected_hash
}

/// Result of verifying the full chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChainVerdict {
    /// All links verified successfully.
    Intact {
        /// Number of events verified.
        count: usize,
    },
    /// A break was detected at the given position.
    Broken {
        /// 0-indexed position of the first broken link.
        position: usize,
        /// Expected hash (recomputed).
        expected: String,
        /// Actual hash found in the journal.
        found: String,
    },
    /// The chain is empty (no events to verify).
    Empty,
}

/// Verify a sequence of (prev_hash, event_json, stored_hash) tuples.
///
/// The caller is responsible for reading the chain from the journal
/// in order. This function is pure — no I/O.
pub fn verify_chain(links: &[(String, String, String)]) -> ChainVerdict {
    if links.is_empty() {
        return ChainVerdict::Empty;
    }

    for (i, (prev_hash, event_json, stored_hash)) in links.iter().enumerate() {
        if !verify_link(prev_hash, event_json, stored_hash) {
            let expected = compute_event_hash(prev_hash, event_json);
            return ChainVerdict::Broken {
                position: i,
                expected,
                found: stored_hash.clone(),
            };
        }
    }

    ChainVerdict::Intact { count: links.len() }
}

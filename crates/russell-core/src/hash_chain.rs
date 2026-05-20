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
//! - Genesis hash: SHA-256 of `/etc/machine-id` contents (or a
//!   fixed fallback if unavailable). This seeds the chain.
//! - Each event hash: `SHA-256(prev_hash_hex || event_json)`.
//! - Stored as 64-char lowercase hex string.
//! - Verification: walk the chain forwards, recomputing each
//!   hash and comparing against stored values.

use sha2::{Digest, Sha256};

/// Length of a SHA-256 hex digest.
pub const HASH_HEX_LEN: usize = 64;

/// Compute the genesis hash (chain seed).
#[must_use]
pub fn genesis_hash() -> String {
    let seed = std::fs::read_to_string("/etc/machine-id")
        .unwrap_or_else(|_| "russell-genesis-seed-no-machine-id".to_string());
    let mut hasher = Sha256::new();
    hasher.update(seed.trim().as_bytes());
    hex::encode(hasher.finalize())
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

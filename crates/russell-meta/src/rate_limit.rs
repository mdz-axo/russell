// SPDX-License-Identifier: MIT OR Apache-2.0
//! Token-bucket rate limiter for LLM calls (T14).
//!
//! Prevents resource exhaustion (local GPU VRAM, Okapi connection
//! pool) from runaway chat sessions or repeated
//! `russell chat` invocations.
//!
//! ## Design
//!
//! Uses a simple token-bucket algorithm: N tokens per minute,
//! refilled continuously. If no token is available, returns a
//! structured "rate limited" error that the caller (Jack's persona
//! layer) can interpret as "I need a moment."
//!
//! This is NOT a retry mechanism — it's backpressure. The caller
//! decides whether to wait, degrade, or inform the operator.

use std::sync::Mutex;
use std::time::Instant;

use crate::error::{DoctorError, Result};

/// Configuration for the rate limiter.
#[derive(Debug, Clone, Copy)]
pub struct RateLimitConfig {
    /// Maximum tokens (requests) in the bucket.
    pub capacity: u32,
    /// Tokens refilled per second.
    pub refill_rate: f64,
}

impl Default for RateLimitConfig {
    /// Default: 3 requests per minute (0.05 per second), burst of 3.
    fn default() -> Self {
        Self {
            capacity: 3,
            refill_rate: 3.0 / 60.0, // 0.05 tokens/sec = 3/min
        }
    }
}

/// Token-bucket rate limiter state.
///
/// Thread-safe via internal `Mutex` — allows shared reference
/// usage in async contexts without requiring `&mut self`.
#[derive(Debug)]
pub struct RateLimiter {
    config: RateLimitConfig,
    state: Mutex<BucketState>,
}

#[derive(Debug)]
struct BucketState {
    /// Current token count (fractional for smooth refill).
    tokens: f64,
    /// Last time tokens were refilled.
    last_refill: Instant,
}

impl RateLimiter {
    /// Create a new rate limiter with the given configuration.
    #[must_use]
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            state: Mutex::new(BucketState {
                tokens: config.capacity as f64,
                last_refill: Instant::now(),
            }),
            config,
        }
    }

    /// Attempt to acquire a token. Returns `Ok(())` if a token is
    pub fn try_acquire(&self) -> Result<()> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());

        // Refill based on elapsed time.
        let now = Instant::now();
        let elapsed = now.duration_since(state.last_refill).as_secs_f64();
        state.tokens =
            (state.tokens + elapsed * self.config.refill_rate).min(self.config.capacity as f64);
        state.last_refill = now;

        if state.tokens >= 1.0 {
            state.tokens -= 1.0;
            Ok(())
        } else {
            // Calculate how long until a token is available.
            let wait_secs = (1.0 - state.tokens) / self.config.refill_rate;
            Err(DoctorError::RateLimited {
                retry_after_seconds: Some(wait_secs.ceil() as u64),
            })
        }
    }

    /// Current available tokens (for diagnostics/proprioception).
    #[must_use]
    pub fn available_tokens(&self) -> f64 {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.tokens
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new(RateLimitConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_up_to_capacity() {
        let rl = RateLimiter::new(RateLimitConfig {
            capacity: 2,
            refill_rate: 0.0, // no refill — tests pure depletion
        });
        assert!(rl.try_acquire().is_ok());
        assert!(rl.try_acquire().is_ok());
        assert!(rl.try_acquire().is_err());
    }

    #[test]
    fn rate_limited_error_has_retry_info() {
        let rl = RateLimiter::new(RateLimitConfig {
            capacity: 1,
            refill_rate: 1.0, // 1 per second
        });
        rl.try_acquire().unwrap(); // deplete
        let err = rl.try_acquire().unwrap_err();
        match err {
            DoctorError::RateLimited {
                retry_after_seconds,
            } => {
                assert!(retry_after_seconds.unwrap() >= 1);
            }
            _ => panic!("expected RateLimited error"),
        }
    }

    #[test]
    fn default_config_is_3_per_minute() {
        let cfg = RateLimitConfig::default();
        assert_eq!(cfg.capacity, 3);
        assert!((cfg.refill_rate - 0.05).abs() < 0.001);
    }
}

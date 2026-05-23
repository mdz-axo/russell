// SPDX-License-Identifier: MIT OR Apache-2.0
//! Rate limiting for ACP calls.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::error::{AcpError, Result};

/// Rate limiter — tracks calls per token.
pub struct RateLimiter {
    /// Calls per token (token → timestamps).
    calls: HashMap<String, Vec<Instant>>,
    /// Limit (calls per window).
    limit: u32,
    /// Window duration.
    window: Duration,
}

impl RateLimiter {
    /// Create a new rate limiter.
    pub fn new(limit: u32, window_secs: u64) -> Self {
        Self {
            calls: HashMap::new(),
            limit,
            window: Duration::from_secs(window_secs),
        }
    }

    /// Check rate limit for a token.
    pub fn check(&mut self, token: &str) -> Result<()> {
        let now = Instant::now();
        let calls = self.calls.entry(token.to_string()).or_insert(Vec::new());

        // Remove old timestamps outside the window.
        calls.retain(|t| now - *t < self.window);

        // Check limit.
        if calls.len() as u32 >= self.limit {
            return Err(AcpError::RateLimitExceeded(self.limit));
        }

        // Record this call.
        calls.push(now);
        Ok(())
    }

    /// Get remaining calls for a token.
    pub fn remaining(&mut self, token: &str) -> u32 {
        let now = Instant::now();
        let calls = self.calls.entry(token.to_string()).or_insert(Vec::new());
        calls.retain(|t| now - *t < self.window);
        self.limit.saturating_sub(calls.len() as u32)
    }

    /// Clear rate limit state for a token (e.g., on token expiration).
    pub fn clear(&mut self, token: &str) {
        self.calls.remove(token);
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new(100, 60) // 100 calls/minute
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_limit_allows_under_limit() {
        let mut limiter = RateLimiter::new(3, 60);
        assert!(limiter.check("token1").is_ok());
        assert!(limiter.check("token1").is_ok());
        assert!(limiter.check("token1").is_ok());
    }

    #[test]
    fn rate_limit_rejects_over_limit() {
        let mut limiter = RateLimiter::new(2, 60);
        assert!(limiter.check("token1").is_ok());
        assert!(limiter.check("token1").is_ok());
        assert!(matches!(
            limiter.check("token1"),
            Err(AcpError::RateLimitExceeded(2))
        ));
    }

    #[test]
    fn rate_limit_per_token() {
        let mut limiter = RateLimiter::new(2, 60);
        assert!(limiter.check("token1").is_ok());
        assert!(limiter.check("token1").is_ok());
        assert!(limiter.check("token2").is_ok()); // Different token
        assert!(limiter.check("token2").is_ok());
    }
}

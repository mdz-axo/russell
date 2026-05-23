// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell self-triage` — Russell diagnoses his own health.
//!
//! This command runs proprioception (Russell watches Russell) and then
//! consults Jack (the Nurse persona) to interpret the results and recommend
//! actions. This is the self-triage capability described in ADR-0015 and
//! ADR-0021.
//!
//! ## Rate Limiting
//!
//! Self-triage is rate-limited to 60 requests per hour (1 per minute average)
//! to prevent resource exhaustion attacks. This is a DoS prevention measure
//! per Bruce Schneier's security principles.

use anyhow::{Context, Result};
use russell_core::journal::JournalWriter;
use russell_core::paths::Paths;
use russell_meta::run_help_with_endpoint;
use russell_proprio::ProprioReflex;
use russell_proprio::run_once as run_proprio;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Rate limiter state for self-triage (60 requests/hour).
struct RateLimitState {
    /// Timestamps of requests in the current hour window.
    requests: Vec<Instant>,
    /// Total rate-limited requests (for monitoring).
    rejected_count: u64,
}

impl RateLimitState {
    fn new() -> Self {
        Self {
            requests: Vec::new(),
            rejected_count: 0,
        }
    }

    /// Check if request is allowed. Returns true if allowed, false if rate limited.
    fn check(&mut self) -> bool {
        let now = Instant::now();
        let one_hour_ago = now - Duration::from_secs(3600);

        // Remove old requests outside the hour window
        self.requests.retain(|&t| t > one_hour_ago);

        // Check if under limit (60 per hour)
        if self.requests.len() < 60 {
            self.requests.push(now);
            true
        } else {
            self.rejected_count += 1;
            false
        }
    }

    /// Get the number of rejected requests.
    fn rejected_count(&self) -> u64 {
        self.rejected_count
    }
}

/// Global rate limiter for self-triage.
static RATE_LIMITER: std::sync::LazyLock<Mutex<RateLimitState>> =
    std::sync::LazyLock::new(|| Mutex::new(RateLimitState::new()));

/// Run self-triage — proprioception + LLM interpretation.
pub async fn run(paths: &Paths) -> Result<()> {
    // Check rate limit first (DoS prevention)
    {
        let mut limiter = RATE_LIMITER.lock().unwrap();
        if !limiter.check() {
            let rejected = limiter.rejected_count();
            drop(limiter);

            eprintln!("⚠ Rate limit exceeded (60 requests/hour)");
            eprintln!("  This is request #{} over the limit.", rejected);
            eprintln!("  Please wait before running self-triage again.");
            eprintln!();
            eprintln!("  If you need immediate assistance, run: russell proprio");
            return Ok(());
        }
    }

    println!("Russell Self-Triage");
    println!("===================\n");

    // Open journal
    let writer = JournalWriter::open(&paths.journal())
        .with_context(|| format!("opening journal {}", paths.journal().display()))?;
    let reader = writer.reader();

    // Run proprioception
    println!("Running proprioception (5 self-vitals)...");
    let proprio_result = match run_proprio(&writer, &reader) {
        Ok(result) => {
            println!("✓ Proprioception complete\n");
            result
        }
        Err(e) => {
            eprintln!("✗ Proprioception failed: {}", e);
            return Ok(());
        }
    };

    // Display self-vitals
    println!("Self-Vitals:");
    println!("  sentinel_last_run_age_s:   {:?}", proprio_result.age_s);
    println!(
        "  journal_writer_stall_s:    {:?}",
        proprio_result.journal_stall_s
    );
    println!(
        "  llm_p95_latency_ms:        {:?}",
        proprio_result.llm_p95_latency_ms
    );
    println!(
        "  timer_drift_s:             {:?}",
        proprio_result.timer_drift_s
    );
    println!(
        "  help_error_rate_pct:       {:?}",
        proprio_result.help_error_rate_pct
    );
    println!();

    // Show severity
    println!("Severity Assessment:");
    println!("  Overall:                {:?}", proprio_result.severity);
    println!("  Sentinel age:           {:?}", proprio_result.severity);
    println!(
        "  Journal stall:          {:?}",
        proprio_result.journal_stall_severity
    );
    println!(
        "  LLM latency:            {:?}",
        proprio_result.llm_p95_severity
    );
    println!(
        "  Timer drift:            {:?}",
        proprio_result.timer_drift_severity
    );
    println!(
        "  Help error rate:        {:?}",
        proprio_result.help_error_rate_severity
    );
    println!();

    // Run reflex arcs
    let mut reflex = ProprioReflex::new();
    reflex.evaluate(&proprio_result);

    if !reflex.actions().is_empty() {
        println!("Reflex Arc Actions (Phase 2A: recommendations only):");
        for action in reflex.actions() {
            println!(
                "  [{}] {} (risk: {:?})",
                action.action_id, action.description, action.risk
            );
        }
        println!();
    }

    // Consult Jack for interpretation
    println!("Consulting Jack (Nurse persona)...");

    // Build self-triage note
    let note = format!(
        "Self-triage request. Current proprioception results:\n\
         - Sentinel age: {:?}s (severity: {:?})\n\
         - Journal stall: {:?}s (severity: {:?})\n\
         - LLM p95 latency: {:?}ms (severity: {:?})\n\
         - Timer drift: {:?}s (severity: {:?})\n\
         - Help error rate: {:?}% (severity: {:?})\n\
         \n\
         Reflex arcs triggered: {}\n\
         \n\
         Please interpret these results and recommend actions for Russell's health.",
        proprio_result.age_s,
        proprio_result.severity,
        proprio_result.journal_stall_s,
        proprio_result.journal_stall_severity,
        proprio_result.llm_p95_latency_ms,
        proprio_result.llm_p95_severity,
        proprio_result.timer_drift_s,
        proprio_result.timer_drift_severity,
        proprio_result.help_error_rate_pct,
        proprio_result.help_error_rate_severity,
        reflex.actions().len()
    );

    // Try to use configured Okapi endpoint, fall back to offline mode
    let endpoint = std::env::var("OKAPI_ENDPOINT")
        .unwrap_or_else(|_| String::from("http://localhost:5000/v1"));

    match run_help_with_endpoint(paths, &writer, Some(&note), &endpoint).await {
        Ok(outcome) => {
            println!("\nJack's Assessment:");
            println!("==================");
            println!("{}", outcome.response);
            println!("\n[evidence: {}]", outcome.evidence_dir.display());
        }
        Err(e) => {
            println!("\n✗ LLM consultation failed: {}", e);
            println!("Running in offline mode...\n");

            // Offline interpretation
            offline_interpretation(&proprio_result, &reflex);
        }
    }

    Ok(())
}

/// Offline interpretation when LLM is unavailable.
fn offline_interpretation(proprio_result: &russell_proprio::ProprioResult, reflex: &ProprioReflex) {
    println!("Offline Health Assessment:");
    println!("==========================\n");

    // Check for critical issues
    let mut issues = Vec::new();

    if let Some(age) = proprio_result.age_s {
        if age > 1800 {
            issues.push(format!(
                "CRITICAL: Sentinel hasn't run in {}s (>30 min)",
                age
            ));
        } else if age > 450 {
            issues.push(format!("WARNING: Sentinel age {}s (>7.5 min)", age));
        }
    }

    if let Some(stall) = proprio_result.journal_stall_s {
        if stall > 300 {
            issues.push(format!("CRITICAL: Journal stalled for {}s (>5 min)", stall));
        } else if stall > 60 {
            issues.push(format!("WARNING: Journal stall {}s (>1 min)", stall));
        }
    }

    if let Some(llm) = proprio_result.llm_p95_latency_ms {
        if llm > 20_000.0 {
            issues.push(format!("CRITICAL: LLM p95 latency {}ms (>20s)", llm as u64));
        } else if llm > 8_000.0 {
            issues.push(format!("WARNING: LLM p95 latency {}ms (>8s)", llm as u64));
        }
    }

    if let Some(drift) = proprio_result.timer_drift_s {
        if drift > 300 {
            issues.push(format!("CRITICAL: Timer drift {}s (>5 min)", drift));
        } else if drift > 90 {
            issues.push(format!("WARNING: Timer drift {}s (>90s)", drift));
        }
    }

    if let Some(error_rate) = proprio_result.help_error_rate_pct {
        if error_rate > 50.0 {
            issues.push(format!(
                "CRITICAL: Help error rate {}% (>50%)",
                error_rate as u64
            ));
        } else if error_rate > 20.0 {
            issues.push(format!(
                "WARNING: Help error rate {}% (>20%)",
                error_rate as u64
            ));
        }
    }

    if issues.is_empty() {
        println!("✓ All self-vitals within normal ranges");
        println!("  Russell is healthy.\n");
    } else {
        println!("Issues Detected:");
        for issue in &issues {
            println!("  • {}", issue);
        }
        println!();
    }

    // Show recommended actions
    if !reflex.actions().is_empty() {
        println!("Recommended Actions:");
        for action in reflex.actions() {
            println!(
                "  [{}] {} (risk: {:?})",
                action.action_id, action.description, action.risk
            );
        }
        println!();
        println!("Note: Phase 2A — Actions are recommendations only.");
        println!("      Phase 3+ will execute automatic remediation.\n");
    }

    println!("Next Steps:");
    println!("  1. Review issues above");
    println!("  2. Check systemd service status: systemctl --user status russell-*");
    println!("  3. Review journal: russell list --limit 50");
    println!(
        "  4. If critical issues persist, restart: systemctl --user restart russell-sentinel.timer\n"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limit_allows_under_limit() {
        let mut state = RateLimitState::new();

        // First 60 requests should be allowed
        for i in 0..60 {
            assert!(state.check(), "Request {} should be allowed", i);
        }

        // 61st request should be rejected
        assert!(!state.check(), "Request 61 should be rejected");
        assert_eq!(state.rejected_count(), 1);
    }

    #[test]
    fn test_rate_limit_window_expires() {
        // This test verifies that old requests are removed from the window
        // We can't easily test the time-based expiry without mocking time,
        // but we can verify the basic logic works
        let mut state = RateLimitState::new();

        // Fill up the limit
        for _ in 0..60 {
            state.check();
        }

        // Should be rejected
        assert!(!state.check());

        // Manually clear old requests (simulating time passing)
        state.requests.clear();

        // Should be allowed again
        assert!(state.check());
    }

    #[test]
    fn test_rate_limit_rejected_count() {
        let mut state = RateLimitState::new();

        // Fill up and exceed limit
        for _ in 0..65 {
            state.check();
        }

        // Should have rejected 5 requests
        assert_eq!(state.rejected_count(), 5);
    }
}

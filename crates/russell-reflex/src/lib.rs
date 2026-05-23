// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell-reflex` — Reflex arcs for automatic responses to proprioception breaches.
//!
//! ## Phase Boundary
//!
//! **Phase 2A (Current):** Detection-only. Reflex arcs recommend actions but do not execute.
//! This respects JR-2 (observe > recommend > act) while building the foundation for
//! automatic remediation.
//!
//! **Phase 3+ (Future):** Corrective arcs will execute automatic remediation for Low-risk
//! actions with operator pre-approval.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
//! │  Proprioception │ ──► │   Reflex Arc    │ ──► │  Recommendation │
//! │  (observation)  │     │  (coordination) │     │  (Phase 2A)     │
//! └─────────────────┘     └─────────────────┘     └─────────────────┘
//! ```
//!
//! ## Reflex Actions
//!
//! | Trigger | Action | Risk | Phase |
//! |---------|--------|------|-------|
//! | `sentinel_last_run_age_s > 1800s` | `restart-sentinel` | Low | 3+ |
//! | `journal_writer_stall_s > 300s` | `flush-journal` | Low | 3+ |
//! | `llm_p95_latency_ms > 20000ms` | `llm-fallback` | Medium | 3+ |
//! | `timer_drift_s > 300s` | `restart-timer` | Medium | 3+ |
//! | `help_error_rate_pct > 50%` | `disable-llm-help` | High | 3+ |
//!
//! See [ADR-0021](../../../docs/adr/0021-proprioception-phase2-reflex-arcs.md).

#![deny(unsafe_code)]
#![deny(rust_2018_idioms)]
#![warn(missing_docs)]

mod action;
mod arc;
mod risk;

pub use action::{ReflexAction, ACTION_RESTART_SENTINEL, ACTION_FLUSH_JOURNAL, ACTION_LLM_FALLBACK, ACTION_RESTART_TIMER, ACTION_DISABLE_LLM_HELP};
pub use arc::ReflexArc;
pub use risk::RiskLevel;

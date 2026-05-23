// SPDX-License-Identifier: MIT OR Apache-2.0
//! Journal port traits (T1) — hexagonal boundary for persistence.
//!
//! These traits define the minimal interface that consumers need
//! from the journal, without coupling to SQLite or any concrete
//! storage mechanism. Following the capability model (Miller):
//!
//! - [`JournalWritePort`] — the authority to append events/samples.
//! - [`JournalReadPort`] — the authority to query history (pure supertrait).
//!
//! Possession of `&dyn JournalWritePort` IS the permission to write.
//! Code that only needs to read receives `&dyn JournalReadPort` or
//! the narrowest sub-trait.
//! This enforces principle-of-least-authority at the type level.
//!
//! ## Scoped Sub-Traits (Task 6: Least-Privilege Access)
//!
//! - [`HostTelemetryPort`] — sentinel needs (rate-of-change, last sample)
//! - [`EventQueryPort`] — meta/ACP needs (severity counts, recent events, baselines)
//! - [`SelfTelemetryPort`] — proprio needs (self-vitals, reflex events)
//! - [`EventDetailPort`] — detailed event access by ID

use crate::Result;
use crate::event::{Event, Scope};

/// Rows returned by read queries.
pub use super::EventRow;

/// Write-side journal port — append events and samples.
///
/// Represents the capability to persist observations. In production
/// this is backed by SQLite WAL; in tests it may be an in-memory vec.
///
/// Note: `Send` but not `Sync` — `JournalWriter` owns a SQLite
/// connection which is not thread-safe. Single-ownership is enforced
/// at the call site (passed by `&self` within one thread).
pub trait JournalWritePort: Send {
    /// Append a structured event to the journal.
    fn append(&self, event: &Event) -> Result<()>;

    /// Append a single numeric/text sample (one probe reading).
    fn append_sample(
        &self,
        ts: i64,
        scope: Scope,
        probe: &str,
        value_num: Option<f64>,
        value_text: Option<&str>,
        unit: Option<&str>,
    ) -> Result<()>;

    /// Append a help session record.
    fn append_help_session(&self, input: &super::HelpSessionInput<'_>) -> Result<()>;
}

/// Read-side journal port — pure supertrait combining all read sub-traits.
///
/// Consumers should prefer the narrowest sub-trait:
/// - [`HostTelemetryPort`] for sentinel rate-of-change
/// - [`EventQueryPort`] for SOAP objective composition
/// - [`SelfTelemetryPort`] for proprio self-vitals
/// - [`EventDetailPort`] for event inspection by ID
pub trait JournalReadPort:
    Send + Sync + HostTelemetryPort + EventQueryPort + SelfTelemetryPort + EventDetailPort
{
}

/// Host telemetry port — sentinel needs for rate-of-change calculations.
///
/// Provides access to historical sample data needed for:
/// - Rate-of-change detection (previous_sample)
/// - Cadence monitoring (last_host_sample_ts)
pub trait HostTelemetryPort: Send + Sync {
    /// Get last host sample timestamp.
    fn last_host_sample_ts(&self) -> Result<Option<i64>>;

    /// Get previous sample value for rate-of-change calculation.
    fn previous_sample(&self, probe: &str, now: i64) -> Result<Option<(f64, i64)>>;
}

/// Event query port — meta/ACP needs for SOAP objective composition.
///
/// Provides access to event history needed for:
/// - Severity counting (severity_counts)
/// - Recent event retrieval (recent)
/// - Sample summaries (host_samples_summary)
/// - Baseline data (read_baselines)
/// - Action-filtered queries (list_events_by_action)
/// - Evidence-filtered queries (recent_with_evidence)
pub trait EventQueryPort: Send + Sync {
    /// Retrieve the N most recent events.
    fn recent(&self, limit: usize) -> Result<Vec<EventRow>>;

    /// Retrieve the N most recent events that have non-NULL evidence_ref.
    fn recent_with_evidence(&self, limit: usize) -> Result<Vec<EventRow>>;

    /// Count events by severity in a time window.
    fn severity_counts(&self, since: i64, until: i64) -> Result<super::SeverityCounts>;

    /// Get host samples summary for a time window.
    fn host_samples_summary(&self, since: i64, until: i64) -> Result<Vec<super::SampleSummary>>;

    /// Read baselines for all probes.
    fn read_baselines(&self) -> Result<Vec<super::BaselineRow>>;

    /// List events by action type in a time window.
    fn list_events_by_action(
        &self,
        action: &str,
        since_unix: i64,
        until_unix: i64,
    ) -> Result<Vec<EventRow>>;
}

/// Self-telemetry port — proprio needs for self-vital monitoring.
///
/// Provides access to self-observation data needed for:
/// - Cadence monitoring (last_host_sample_ts)
/// - Reflex event tracking (count_reflex_events)
/// - Help session error rate (help_error_rate_pct)
/// - LLM latency monitoring (llm_latency_p95_ms)
/// - Journal chain integrity (check_chain_integrity)
pub trait SelfTelemetryPort: Send + Sync {
    /// Get last host sample timestamp.
    fn last_host_sample_ts(&self) -> Result<Option<i64>>;

    /// Count reflex events for a probe in a time window.
    fn count_reflex_events(&self, probe: &str, since: i64, until: i64) -> Result<usize>;

    /// Get help session error rate as percentage (0.0-100.0).
    fn help_error_rate_pct(&self) -> Result<Option<f64>>;

    /// Get LLM p95 latency in milliseconds.
    fn llm_latency_p95_ms(&self) -> Result<Option<f64>>;

    /// Check journal hash chain integrity (last 10 events).
    fn check_chain_integrity(&self) -> Option<bool>;
}

/// Event detail port — detailed event access by ID.
///
/// Provides access to specific events for:
/// - Event inspection (get_event)
pub trait EventDetailPort: Send + Sync {
    /// Get a specific event by its row ID.
    fn get_event(&self, id: i64) -> Result<Event>;
}

// ---------------------------------------------------------------------------
// Production implementations
// ---------------------------------------------------------------------------

impl JournalWritePort for super::JournalWriter {
    fn append(&self, event: &Event) -> Result<()> {
        super::JournalWriter::append(self, event)
    }

    fn append_sample(
        &self,
        ts: i64,
        scope: Scope,
        probe: &str,
        value_num: Option<f64>,
        value_text: Option<&str>,
        unit: Option<&str>,
    ) -> Result<()> {
        super::JournalWriter::append_sample(self, ts, scope, probe, value_num, value_text, unit)
    }

    fn append_help_session(&self, input: &super::HelpSessionInput<'_>) -> Result<()> {
        super::JournalWriter::append_help_session(self, input)
    }
}

/// Pure supertrait — all methods come from sub-traits.
impl JournalReadPort for super::JournalReader {}

impl HostTelemetryPort for super::JournalReader {
    fn last_host_sample_ts(&self) -> Result<Option<i64>> {
        super::JournalReader::last_host_sample_ts(self)
    }

    fn previous_sample(&self, probe: &str, now: i64) -> Result<Option<(f64, i64)>> {
        Ok(super::JournalReader::previous_sample(self, probe, now))
    }
}

impl EventQueryPort for super::JournalReader {
    fn recent(&self, limit: usize) -> Result<Vec<EventRow>> {
        super::JournalReader::recent(self, limit)
    }

    fn recent_with_evidence(&self, limit: usize) -> Result<Vec<EventRow>> {
        super::JournalReader::recent_with_evidence(self, limit)
    }

    fn severity_counts(&self, since: i64, until: i64) -> Result<super::SeverityCounts> {
        super::JournalReader::severity_counts(self, since, until)
    }

    fn host_samples_summary(&self, since: i64, until: i64) -> Result<Vec<super::SampleSummary>> {
        super::JournalReader::host_samples_summary(self, since, until)
    }

    fn read_baselines(&self) -> Result<Vec<super::BaselineRow>> {
        super::JournalReader::read_baselines(self)
    }

    fn list_events_by_action(
        &self,
        action: &str,
        since_unix: i64,
        until_unix: i64,
    ) -> Result<Vec<EventRow>> {
        super::JournalReader::list_events_by_action(self, action, since_unix, until_unix)
    }
}

impl SelfTelemetryPort for super::JournalReader {
    fn last_host_sample_ts(&self) -> Result<Option<i64>> {
        super::JournalReader::last_host_sample_ts(self)
    }

    fn count_reflex_events(&self, probe: &str, since: i64, until: i64) -> Result<usize> {
        Ok(super::JournalReader::count_reflex_events(self, probe, since, until)? as usize)
    }

    fn help_error_rate_pct(&self) -> Result<Option<f64>> {
        super::JournalReader::help_error_rate_pct(self)
    }

    fn llm_latency_p95_ms(&self) -> Result<Option<f64>> {
        super::JournalReader::llm_latency_p95_ms(self)
    }

    fn check_chain_integrity(&self) -> Option<bool> {
        super::JournalReader::check_chain_integrity(self)
    }
}

impl EventDetailPort for super::JournalReader {
    fn get_event(&self, id: i64) -> Result<Event> {
        super::JournalReader::get_event(self, id)
    }
}

// ---------------------------------------------------------------------------
// Test double implementations
// ---------------------------------------------------------------------------

/// In-memory journal for tests — captures writes without
/// requiring a real SQLite database.
///
/// Available to all crates in the workspace for integration testing.
/// Implements [`JournalWritePort`] with simple `Mutex<Vec<_>>` storage.
#[derive(Default)]
pub struct InMemoryJournal {
    /// Captured events.
    pub events: std::sync::Mutex<Vec<Event>>,
    /// Captured samples as (ts, probe_name, value).
    pub samples: std::sync::Mutex<Vec<(i64, String, Option<f64>)>>,
}

impl JournalWritePort for InMemoryJournal {
    fn append(&self, event: &Event) -> Result<()> {
        self.events.lock().unwrap().push(event.clone());
        Ok(())
    }

    fn append_sample(
        &self,
        ts: i64,
        _scope: Scope,
        probe: &str,
        value_num: Option<f64>,
        _value_text: Option<&str>,
        _unit: Option<&str>,
    ) -> Result<()> {
        self.samples
            .lock()
            .unwrap()
            .push((ts, probe.to_string(), value_num));
        Ok(())
    }

    fn append_help_session(&self, _input: &super::HelpSessionInput<'_>) -> Result<()> {
        Ok(())
    }
}

impl JournalReadPort for InMemoryJournal {}

impl HostTelemetryPort for InMemoryJournal {
    fn last_host_sample_ts(&self) -> Result<Option<i64>> {
        let samples = self.samples.lock().unwrap();
        Ok(samples.last().map(|(ts, _, _)| *ts))
    }

    fn previous_sample(&self, _probe: &str, _now: i64) -> Result<Option<(f64, i64)>> {
        Ok(None)
    }
}

impl EventQueryPort for InMemoryJournal {
    fn recent(&self, limit: usize) -> Result<Vec<EventRow>> {
        let events = self.events.lock().unwrap();
        Ok(events
            .iter()
            .rev()
            .take(limit)
            .map(|e| EventRow {
                id: String::new(),
                ts: String::new(),
                scope: e.scope,
                severity: e.severity,
                tier: e.tier.clone(),
                module: e.module.clone(),
                action: String::new(),
                summary: e.summary.clone(),
                evidence_ref: e.evidence_ref.clone(),
            })
            .collect())
    }

    fn recent_with_evidence(&self, limit: usize) -> Result<Vec<EventRow>> {
        self.recent(limit)
    }

    fn severity_counts(&self, _since: i64, _until: i64) -> Result<super::SeverityCounts> {
        let events = self.events.lock().unwrap();
        let mut counts = super::SeverityCounts::default();
        for event in events.iter() {
            match event.severity {
                crate::event::Severity::Crit => counts.crit += 1,
                crate::event::Severity::Alert => counts.alert += 1,
                crate::event::Severity::Warn => counts.warn += 1,
                crate::event::Severity::Info => counts.info += 1,
            }
        }
        Ok(counts)
    }

    fn host_samples_summary(&self, _since: i64, _until: i64) -> Result<Vec<super::SampleSummary>> {
        Ok(Vec::new())
    }

    fn read_baselines(&self) -> Result<Vec<super::BaselineRow>> {
        Ok(Vec::new())
    }

    fn list_events_by_action(
        &self,
        _action: &str,
        _since_unix: i64,
        _until_unix: i64,
    ) -> Result<Vec<EventRow>> {
        Ok(Vec::new())
    }
}

impl SelfTelemetryPort for InMemoryJournal {
    fn last_host_sample_ts(&self) -> Result<Option<i64>> {
        let samples = self.samples.lock().unwrap();
        Ok(samples.last().map(|(ts, _, _)| *ts))
    }

    fn count_reflex_events(&self, _probe: &str, _since: i64, _until: i64) -> Result<usize> {
        Ok(0)
    }

    fn help_error_rate_pct(&self) -> Result<Option<f64>> {
        Ok(None)
    }

    fn llm_latency_p95_ms(&self) -> Result<Option<f64>> {
        Ok(None)
    }

    fn check_chain_integrity(&self) -> Option<bool> {
        Some(true)
    }
}

impl EventDetailPort for InMemoryJournal {
    fn get_event(&self, _id: i64) -> Result<Event> {
        Err(crate::error::CoreError::Invariant(
            "event not found in InMemoryJournal".into(),
        ))
    }
}

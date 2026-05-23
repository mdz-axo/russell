// SPDX-License-Identifier: MIT OR Apache-2.0
//! Journal port traits (T1) — hexagonal boundary for persistence.
//!
//! These traits define the minimal interface that consumers need
//! from the journal, without coupling to SQLite or any concrete
//! storage mechanism. Following the capability model (Miller):
//!
//! - [`JournalWritePort`] — the authority to append events/samples.
//! - [`JournalReadPort`] — the authority to query history.
//!
//! Possession of `&dyn JournalWritePort` IS the permission to write.
//! Code that only needs to read receives `&dyn JournalReadPort`.
//! This enforces principle-of-least-authority at the type level.
//!
//! ## Implementation
//!
//! `JournalWriter` implements `JournalWritePort`.
//! `JournalReader` implements `JournalReadPort`.
//! In-memory test doubles can implement either or both.

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

/// Read-side journal port — query events and samples.
///
/// Represents the capability to read history. Separated from write
/// to enforce least-authority: sentinel can read previous samples
/// for rate-of-change without having write access.
///
/// ## Task A2: Hexagonal port expansion
///
/// This trait now includes all read operations needed by consumers
/// (sentinel, meta, proprio) without coupling to SQLite implementation.
///
/// ## Scoped Sub-Traits (Task 6: Least-Privilege Access)
///
/// For finer-grained access control, consumers should depend on the
/// narrowest sub-trait that satisfies their needs:
///
/// - [`HostTelemetryPort`] — sentinel needs (rate-of-change, last sample)
/// - [`EventQueryPort`] — meta/ACP needs (severity counts, recent events, baselines)
/// - [`SelfTelemetryPort`] — proprio needs (self-vitals, reflex events)
/// - [`EventDetailPort`] — detailed event access by ID
pub trait JournalReadPort: Send + Sync + HostTelemetryPort + EventQueryPort + SelfTelemetryPort + EventDetailPort {
    /// Retrieve the N most recent events.
    fn recent(&self, limit: usize) -> Result<Vec<EventRow>>;

    /// Count events by severity in a time window.
    fn severity_counts(&self, since: i64, until: i64) -> Result<super::SeverityCounts>;

    /// Get last host sample timestamp.
    fn last_host_sample_ts(&self) -> Result<Option<i64>>;

    /// Get previous sample value for rate-of-change calculation.
    fn previous_sample(&self, probe: &str, now: i64) -> Result<Option<(f64, i64)>>;

    /// Get host samples summary for a time window.
    fn host_samples_summary(&self, since: i64, until: i64) -> Result<Vec<super::SampleSummary>>;

    /// Read baselines for all probes.
    fn read_baselines(&self) -> Result<Vec<super::BaselineRow>>;

    /// Count reflex events for a probe in a time window.
    fn count_reflex_events(&self, probe: &str, since: i64, until: i64) -> Result<usize>;

    /// Get a specific event by its row ID.
    fn get_event(&self, id: i64) -> Result<Event>;
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
pub trait EventQueryPort: Send + Sync {
    /// Retrieve the N most recent events.
    fn recent(&self, limit: usize) -> Result<Vec<EventRow>>;

    /// Count events by severity in a time window.
    fn severity_counts(&self, since: i64, until: i64) -> Result<super::SeverityCounts>;

    /// Get host samples summary for a time window.
    fn host_samples_summary(&self, since: i64, until: i64) -> Result<Vec<super::SampleSummary>>;

    /// Read baselines for all probes.
    fn read_baselines(&self) -> Result<Vec<super::BaselineRow>>;
}

/// Self-telemetry port — proprio needs for self-vital monitoring.
///
/// Provides access to self-observation data needed for:
/// - Cadence monitoring (last_host_sample_ts)
/// - Reflex event tracking (count_reflex_events)
pub trait SelfTelemetryPort: Send + Sync {
    /// Get last host sample timestamp.
    fn last_host_sample_ts(&self) -> Result<Option<i64>>;

    /// Count reflex events for a probe in a time window.
    fn count_reflex_events(&self, probe: &str, since: i64, until: i64) -> Result<usize>;
}

/// Event detail port — detailed event access by ID.
///
/// Provides access to specific events for:
/// - Event inspection (get_event)
pub trait EventDetailPort: Send + Sync {
    /// Get a specific event by its row ID.
    fn get_event(&self, id: i64) -> Result<Event>;
}

/// Implement `JournalWritePort` for the production `JournalWriter`.
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

/// Implement `JournalReadPort` for the production `JournalReader`.
impl JournalReadPort for super::JournalReader {
    fn recent(&self, limit: usize) -> Result<Vec<EventRow>> {
        super::JournalReader::recent(self, limit)
    }

    fn severity_counts(&self, since: i64, until: i64) -> Result<super::SeverityCounts> {
        super::JournalReader::severity_counts(self, since, until)
    }

    fn last_host_sample_ts(&self) -> Result<Option<i64>> {
        super::JournalReader::last_host_sample_ts(self)
    }

    fn previous_sample(&self, probe: &str, now: i64) -> Result<Option<(f64, i64)>> {
        Ok(super::JournalReader::previous_sample(self, probe, now))
    }

    fn host_samples_summary(&self, since: i64, until: i64) -> Result<Vec<super::SampleSummary>> {
        super::JournalReader::host_samples_summary(self, since, until)
    }

    fn read_baselines(&self) -> Result<Vec<super::BaselineRow>> {
        super::JournalReader::read_baselines(self)
    }

    fn count_reflex_events(&self, probe: &str, since: i64, until: i64) -> Result<usize> {
        Ok(super::JournalReader::count_reflex_events(self, probe, since, until)? as usize)
    }

    fn get_event(&self, id: i64) -> Result<Event> {
        super::JournalReader::get_event(self, id)
    }
}

/// Implement scoped sub-traits for the production `JournalReader`.
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

    fn severity_counts(&self, since: i64, until: i64) -> Result<super::SeverityCounts> {
        super::JournalReader::severity_counts(self, since, until)
    }

    fn host_samples_summary(&self, since: i64, until: i64) -> Result<Vec<super::SampleSummary>> {
        super::JournalReader::host_samples_summary(self, since, until)
    }

    fn read_baselines(&self) -> Result<Vec<super::BaselineRow>> {
        super::JournalReader::read_baselines(self)
    }
}

impl SelfTelemetryPort for super::JournalReader {
    fn last_host_sample_ts(&self) -> Result<Option<i64>> {
        super::JournalReader::last_host_sample_ts(self)
    }

    fn count_reflex_events(&self, probe: &str, since: i64, until: i64) -> Result<usize> {
        Ok(super::JournalReader::count_reflex_events(self, probe, since, until)? as usize)
    }
}

impl EventDetailPort for super::JournalReader {
    fn get_event(&self, id: i64) -> Result<Event> {
        super::JournalReader::get_event(self, id)
    }
}

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

/// Implement scoped sub-traits for `InMemoryJournal` test double.
///
/// These implementations provide minimal/stub behavior suitable for testing.
/// Production code should use `JournalReader` which has full SQLite-backed implementations.
impl HostTelemetryPort for InMemoryJournal {
    fn last_host_sample_ts(&self) -> Result<Option<i64>> {
        let samples = self.samples.lock().unwrap();
        Ok(samples.last().map(|(ts, _, _)| *ts))
    }

    fn previous_sample(&self, _probe: &str, _now: i64) -> Result<Option<(f64, i64)>> {
        // In-memory journal doesn't track historical samples for rate-of-change
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
                scope: e.scope.clone(),
                severity: e.severity.clone(),
                tier: e.tier.clone(),
                module: e.module.clone(),
                action: String::new(),
                summary: e.summary.clone(),
            })
            .collect())
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
        // In-memory journal doesn't track sample summaries
        Ok(Vec::new())
    }

    fn read_baselines(&self) -> Result<Vec<super::BaselineRow>> {
        // In-memory journal doesn't track baselines
        Ok(Vec::new())
    }
}

impl SelfTelemetryPort for InMemoryJournal {
    fn last_host_sample_ts(&self) -> Result<Option<i64>> {
        let samples = self.samples.lock().unwrap();
        Ok(samples.last().map(|(ts, _, _)| *ts))
    }

    fn count_reflex_events(&self, _probe: &str, _since: i64, _until: i64) -> Result<usize> {
        // In-memory journal doesn't track reflex events
        Ok(0)
    }
}

impl EventDetailPort for InMemoryJournal {
    fn get_event(&self, _id: i64) -> Result<Event> {
        // In-memory journal doesn't support ID-based lookup
        Err(crate::error::CoreError::Invariant("event not found in InMemoryJournal".into()))
    }
}

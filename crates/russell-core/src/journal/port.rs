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
pub trait JournalReadPort: Send + Sync {
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
}

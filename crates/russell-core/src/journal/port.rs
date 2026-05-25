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

    /// Batch-fetch previous sample values for multiple probes.
    ///
    /// Avoids N+1 query pattern — single connection/query instead of one per probe.
    fn previous_samples_batch(
        &self,
        probes: &[&str],
        before_ts: i64,
    ) -> Result<std::collections::HashMap<String, (f64, i64)>>;
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

    fn previous_samples_batch(
        &self,
        probes: &[&str],
        before_ts: i64,
    ) -> Result<std::collections::HashMap<String, (f64, i64)>> {
        super::JournalReader::previous_samples_batch(self, probes, before_ts)
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
/// Implements all port traits with meaningful in-memory semantics so
/// that tests exercising read paths don't silently pass with stubs.
#[derive(Default)]
pub struct InMemoryJournal {
    /// Captured events.
    pub events: std::sync::Mutex<Vec<Event>>,
    /// Hash chain data per event: (prev_hash, hash), same index as `events`.
    hash_chain: std::sync::Mutex<Vec<(String, String)>>,
    /// Captured samples as (ts, scope, probe_name, value_num, value_text, unit).
    pub samples: std::sync::Mutex<Vec<(i64, Scope, String, Option<f64>, Option<String>, Option<String>)>>,
    /// Help sessions: (ts_unix, status) where status is "ok", "error", "fallback", "threshold_skip".
    help_sessions: std::sync::Mutex<Vec<(i64, String)>>,
    /// Stored baselines for `read_baselines()`.
    baselines: std::sync::Mutex<Vec<super::BaselineRow>>,
}

impl InMemoryJournal {
    /// Insert a baseline row for `read_baselines()` queries.
    pub fn add_baseline(&self, row: super::BaselineRow) {
        self.baselines.lock().unwrap().push(row);
    }
}

impl JournalWritePort for InMemoryJournal {
    fn append(&self, event: &Event) -> Result<()> {
        let prev_hash = self
            .hash_chain
            .lock()
            .unwrap()
            .last()
            .map(|(_, h)| h.clone())
            .unwrap_or_else(crate::hash_chain::genesis_hash);
        let payload = serde_json::to_string(event)
            .map_err(|e| crate::error::CoreError::Json(e))?;
        let hash = crate::hash_chain::compute_event_hash(&prev_hash, &payload);
        self.hash_chain.lock().unwrap().push((prev_hash, hash));
        self.events.lock().unwrap().push(event.clone());
        Ok(())
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
        self.samples.lock().unwrap().push((
            ts,
            scope,
            probe.to_string(),
            value_num,
            value_text.map(|s| s.to_string()),
            unit.map(|s| s.to_string()),
        ));
        Ok(())
    }

    fn append_help_session(&self, input: &super::HelpSessionInput<'_>) -> Result<()> {
        let status = input.status.as_str().to_string();
        self.help_sessions
            .lock()
            .unwrap()
            .push((input.ts_unix, status));
        Ok(())
    }
}

impl JournalReadPort for InMemoryJournal {}

impl HostTelemetryPort for InMemoryJournal {
    fn last_host_sample_ts(&self) -> Result<Option<i64>> {
        let samples = self.samples.lock().unwrap();
        Ok(samples
            .iter()
            .rev()
            .find(|(_, scope, _, _, _, _)| *scope == Scope::Host)
            .map(|(ts, _, _, _, _, _)| *ts))
    }

    fn previous_sample(&self, probe: &str, now: i64) -> Result<Option<(f64, i64)>> {
        let samples = self.samples.lock().unwrap();
        Ok(samples
            .iter()
            .rev()
            .find(|(ts, _, name, val, _, _)| *name == probe && *ts < now && val.is_some())
            .map(|(ts, _, _, val, _, _)| (val.unwrap(), *ts)))
    }

    fn previous_samples_batch(
        &self,
        probes: &[&str],
        before_ts: i64,
    ) -> Result<std::collections::HashMap<String, (f64, i64)>> {
        let mut result = std::collections::HashMap::new();
        let samples = self.samples.lock().unwrap();
        for (ts, _, name, val, _, _) in samples.iter().rev() {
            if *ts >= before_ts {
                continue;
            }
            if result.contains_key(name) {
                continue;
            }
            if probes.contains(&name.as_str()) {
                if let Some(v) = val {
                    result.insert(name.clone(), (*v, *ts));
                }
            }
        }
        Ok(result)
    }
}

impl EventQueryPort for InMemoryJournal {
    fn recent(&self, limit: usize) -> Result<Vec<EventRow>> {
        let events = self.events.lock().unwrap();
        Ok(events
            .iter()
            .rev()
            .take(limit)
            .enumerate()
            .map(|(i, e)| EventRow {
                id: format!("mem-{i}"),
                ts: e.ts.clone(),
                scope: e.scope,
                severity: e.severity,
                tier: e.tier.clone(),
                module: e.module.clone(),
                action: e.action.clone(),
                summary: e.summary.clone(),
                evidence_ref: e.evidence_ref.clone(),
            })
            .collect())
    }

    fn recent_with_evidence(&self, limit: usize) -> Result<Vec<EventRow>> {
        let events = self.events.lock().unwrap();
        let filtered: Vec<_> = events
            .iter()
            .rev()
            .filter(|e| e.evidence_ref.is_some())
            .take(limit)
            .enumerate()
            .map(|(i, e)| EventRow {
                id: format!("mem-{i}"),
                ts: e.ts.clone(),
                scope: e.scope,
                severity: e.severity,
                tier: e.tier.clone(),
                module: e.module.clone(),
                action: e.action.clone(),
                summary: e.summary.clone(),
                evidence_ref: e.evidence_ref.clone(),
            })
            .collect();
        Ok(filtered)
    }

    fn severity_counts(&self, since: i64, until: i64) -> Result<super::SeverityCounts> {
        let events = self.events.lock().unwrap();
        let mut counts = super::SeverityCounts::default();
        for event in events.iter() {
            if event.ts_unix >= since && event.ts_unix <= until {
                match event.severity {
                    crate::event::Severity::Crit => counts.crit += 1,
                    crate::event::Severity::Alert => counts.alert += 1,
                    crate::event::Severity::Warn => counts.warn += 1,
                    crate::event::Severity::Info => counts.info += 1,
                }
            }
        }
        Ok(counts)
    }

    fn host_samples_summary(&self, since: i64, until: i64) -> Result<Vec<super::SampleSummary>> {
        let samples = self.samples.lock().unwrap();
        let mut by_probe: std::collections::HashMap<String, Vec<(f64, i64)>> = std::collections::HashMap::new();
        for (ts, _, name, val, _, _) in samples.iter() {
            if *ts >= since && *ts <= until {
                if let Some(v) = val {
                    by_probe.entry(name.clone()).or_default().push((*v, *ts));
                }
            }
        }
        let mut result = Vec::new();
        for (probe, values) in by_probe {
            if values.is_empty() {
                continue;
            }
            let count = values.len() as i64;
            let min = values.iter().map(|(v, _)| *v).fold(f64::INFINITY, f64::min);
            let max = values.iter().map(|(v, _)| *v).fold(f64::NEG_INFINITY, f64::max);
            let sum: f64 = values.iter().map(|(v, _)| *v).sum();
            let avg = sum / count as f64;
            let last = values.last().unwrap();
            result.push(super::SampleSummary {
                probe,
                unit: None,
                min: Some(min),
                avg: Some(avg),
                max: Some(max),
                last: Some(last.0),
                last_ts: Some(last.1),
                count,
            });
        }
        Ok(result)
    }

    fn read_baselines(&self) -> Result<Vec<super::BaselineRow>> {
        Ok(self.baselines.lock().unwrap().clone())
    }

    fn list_events_by_action(
        &self,
        action: &str,
        since_unix: i64,
        until_unix: i64,
    ) -> Result<Vec<EventRow>> {
        let events = self.events.lock().unwrap();
        Ok(events
            .iter()
            .rev()
            .filter(|e| e.action == action && e.ts_unix >= since_unix && e.ts_unix <= until_unix)
            .enumerate()
            .map(|(i, e)| EventRow {
                id: format!("mem-{i}"),
                ts: e.ts.clone(),
                scope: e.scope,
                severity: e.severity,
                tier: e.tier.clone(),
                module: e.module.clone(),
                action: e.action.clone(),
                summary: e.summary.clone(),
                evidence_ref: e.evidence_ref.clone(),
            })
            .collect())
    }
}

impl SelfTelemetryPort for InMemoryJournal {
    fn last_host_sample_ts(&self) -> Result<Option<i64>> {
        let samples = self.samples.lock().unwrap();
        Ok(samples
            .iter()
            .rev()
            .find(|(_, scope, _, _, _, _)| *scope == Scope::Host)
            .map(|(ts, _, _, _, _, _)| *ts))
    }

    fn count_reflex_events(&self, probe: &str, since: i64, until: i64) -> Result<usize> {
        let events = self.events.lock().unwrap();
        Ok(events
            .iter()
            .filter(|e| {
                e.action == "reflex_proposed"
                    && e.module.as_deref() == Some(probe)
                    && e.ts_unix >= since
                    && e.ts_unix <= until
            })
            .count())
    }

    fn help_error_rate_pct(&self) -> Result<Option<f64>> {
        let sessions = self.help_sessions.lock().unwrap();
        if sessions.is_empty() {
            return Ok(None);
        }
        let errors = sessions.iter().filter(|(_, s)| s == "error").count();
        Ok(Some((errors as f64 / sessions.len() as f64) * 100.0))
    }

    fn llm_latency_p95_ms(&self) -> Result<Option<f64>> {
        let events = self.events.lock().unwrap();
        let mut latencies: Vec<u64> = events
            .iter()
            .filter(|e| e.module.as_deref() == Some("llm"))
            .filter_map(|e| e.duration_ms)
            .collect();
        if latencies.is_empty() {
            return Ok(None);
        }
        latencies.sort_unstable();
        let idx = ((latencies.len() as f64) * 0.95).ceil() as usize;
        let idx = idx.saturating_sub(1).min(latencies.len() - 1);
        Ok(Some(latencies[idx] as f64))
    }

    fn check_chain_integrity(&self) -> Option<bool> {
        let chain = self.hash_chain.lock().unwrap();
        if chain.is_empty() {
            return Some(true);
        }
        let events = self.events.lock().unwrap();
        let start = chain.len().saturating_sub(10);
        let links: Vec<(String, String, String)> = (start..chain.len())
            .filter_map(|i| {
                let (prev_hash, stored_hash) = &chain[i];
                let payload = serde_json::to_string(&events[i]).ok()?;
                Some((prev_hash.clone(), payload, stored_hash.clone()))
            })
            .collect();
        if links.is_empty() {
            return Some(true);
        }
        match crate::hash_chain::verify_chain(&links) {
            crate::hash_chain::ChainVerdict::Intact { .. } => Some(true),
            crate::hash_chain::ChainVerdict::Empty => Some(true),
            crate::hash_chain::ChainVerdict::Broken { .. } => Some(false),
        }
    }
}

impl EventDetailPort for InMemoryJournal {
    fn get_event(&self, id: i64) -> Result<Event> {
        let events = self.events.lock().unwrap();
        let idx = (id - 1) as usize;
        events
            .get(idx)
            .cloned()
            .ok_or(crate::error::CoreError::Invariant(
                "event not found in InMemoryJournal".into(),
            ))
    }
}

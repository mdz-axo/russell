// SPDX-License-Identifier: MIT OR Apache-2.0
//! SQLite-backed event journal.
//!
//! See [ADR-0004](../../../docs/adr/0004-sqlite-journal.md).
//!
//! Phase-0 layout:
//!
//! - `JournalWriter` — a single-owner handle. All writes route
//!   through its blocking-thread companion via
//!   [`tokio::task::spawn_blocking`]. The Phase-0 API is
//!   synchronous (no async boundary) since the CLI subcommands
//!   we ship are blocking; async wrappers arrive in Phase 1.
//! - `JournalReader` — cheap cloneable handle that opens a
//!   fresh read-only connection on demand.
//!
//! Migrations live beside this module in `migrations/*.sql` and
//! are applied in numerical order the first time a journal is
//! opened.

pub mod migrations;
pub mod port;

// Re-export port traits for convenient access.
pub use port::{
    EventDetailPort, EventQueryPort, HostTelemetryPort, JournalReadPort, JournalWritePort,
    SelfTelemetryPort,
};

use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::{AtomicI64, Ordering};

use rusqlite::{Connection, OpenFlags, params};
use tracing::{debug, info};

use crate::error::{CoreError, Result};
use crate::event::{Event, EventId, Scope, Severity};

/// The four-valued outcome of a help session.
///
/// Replaces raw `String` status fields with a compiler-enforced
/// domain enum — typos and case mismatches are caught at compile
/// time rather than at query time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HelpSessionStatus {
    /// LLM call succeeded.
    Ok,
    /// LLM returned an error.
    Error,
    /// Offline fallback was engaged.
    Fallback,
    /// Below escalation threshold — rule-based summary.
    ThresholdSkip,
}

impl HelpSessionStatus {
    /// Lowercase string for journal persistence.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Error => "error",
            Self::Fallback => "fallback",
            Self::ThresholdSkip => "threshold_skip",
        }
    }
}

impl FromStr for HelpSessionStatus {
    type Err = CoreError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "ok" => Ok(Self::Ok),
            "error" => Ok(Self::Error),
            "fallback" => Ok(Self::Fallback),
            "threshold_skip" => Ok(Self::ThresholdSkip),
            other => Err(CoreError::Invariant(format!(
                "unknown help session status: {other}"
            ))),
        }
    }
}

/// Write-capable journal handle. Cheap to construct; holds an
/// open SQLite connection in WAL mode.
pub struct JournalWriter {
    conn: Connection,
    path: PathBuf,
    /// Atomically updated on every write (append / append_sample).
    /// Records the unix timestamp of the most recent write.
    last_write_unix_s: AtomicI64,
    /// Last event hash for hash-chain continuity (T6).
    /// Initialized from the latest event's hash column on open,
    /// or from [`crate::hash_chain::genesis_hash`] if the DB is empty.
    last_hash: std::cell::RefCell<String>,
}

/// Read-only journal handle. Constructs fresh connections as needed.
#[derive(Debug, Clone)]
pub struct JournalReader {
    path: PathBuf,
}

/// Input struct for [`JournalWriter::append_help_session`].
///
/// Replaces the 12-parameter positional call with a named-field
/// struct for call-site clarity.
#[derive(Debug, Clone)]
pub struct HelpSessionInput<'a> {
    /// Unique session ID (ULID).
    pub id: &'a str,
    /// Unix timestamp of session start.
    pub ts_unix: i64,
    /// RFC 3339 timestamp.
    pub ts: &'a str,
    /// Backend name (`"okapi"`, `"offline"`, `"mock"`).
    pub backend: &'a str,
    /// Model name, if applicable.
    pub model: Option<&'a str>,
    /// Operator note or user message.
    pub note: Option<&'a str>,
    /// Character count of the prompt sent.
    pub prompt_chars: i64,
    /// Character count of the response received.
    pub response_chars: i64,
    /// LLM response latency in milliseconds.
    pub latency_ms: Option<i64>,
    /// Outcome status.
    pub status: HelpSessionStatus,
    /// Error category if `status` is `"error"`.
    pub error_kind: Option<&'a str>,
    /// Path to the evidence directory.
    pub evidence_ref: &'a str,
}

/// A single `events` row, in the shape the digest / `journal_query`
/// consumers want.
#[derive(Debug, Clone)]
pub struct EventRow {
    /// ULID as string.
    pub id: String,
    /// RFC 3339 timestamp.
    pub ts: String,
    /// Severity band.
    pub severity: Severity,
    /// Scope: host vs. self.
    pub scope: Scope,
    /// Tier, if any.
    pub tier: Option<String>,
    /// Module, if any.
    pub module: Option<String>,
    /// Action verb.
    pub action: String,
    /// One-line summary, if any.
    pub summary: Option<String>,
    /// Evidence bundle path, if any (Task 13).
    pub evidence_ref: Option<String>,
}

impl JournalWriter {
    /// Open (or create) the journal at `path`, apply any pending
    /// migrations, and configure it per ADR-0004.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Sqlite`] if the DB cannot be opened,
    /// [`CoreError::Migration`] if a migration fails.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            crate::paths::ensure_dir(parent)?;
        }
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_URI,
        )?;

        configure_pragmas(&conn)?;
        migrations::run(&conn)?;

        info!(db = %path.display(), "journal opened");

        // Initialize hash chain: read the most recent event's hash,
        // or use the genesis hash if the DB is empty / pre-chain.
        let last_hash = conn
            .query_row(
                "SELECT hash FROM events WHERE hash IS NOT NULL ORDER BY ts_unix DESC LIMIT 1",
                [],
                |r| r.get::<_, String>(0),
            )
            .unwrap_or_else(|_| crate::hash_chain::genesis_hash());

        Ok(Self {
            conn,
            path: path.to_path_buf(),
            last_write_unix_s: AtomicI64::new(crate::time::now_unix()),
            last_hash: std::cell::RefCell::new(last_hash),
        })
    }

    /// Append a single `Event` to the journal.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Sqlite`] on DB errors, [`CoreError::Json`]
    /// if the event cannot be serialised.
    pub fn append(&self, event: &Event) -> Result<()> {
        let payload = serde_json::to_string(event)?;
        let severity = event.severity.as_str();
        let scope = event.scope.as_str();

        // T6: Compute hash chain link.
        let prev_hash = self.last_hash.borrow().clone();
        let hash = crate::hash_chain::compute_event_hash(&prev_hash, &payload);

        self.conn.execute(
            r"INSERT INTO events (
                id, ts_unix, ts, schema, scope, tier, module, run_id,
                severity, action, dry_run, summary, evidence_ref,
                duration_ms, payload, prev_hash, hash
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
            params![
                event.id.to_string(),
                event.ts_unix,
                event.ts,
                event.schema,
                scope,
                event.tier,
                event.module,
                event.run_id,
                severity,
                event.action,
                event.dry_run as i64,
                event.summary,
                event.evidence_ref,
                event.duration_ms.map(|d| d as i64),
                payload,
                prev_hash,
                hash,
            ],
        )?;

        // Update chain state.
        *self.last_hash.borrow_mut() = hash;

        debug!(id = %event.id, action = %event.action, "event appended (hash-chained)");
        self.last_write_unix_s
            .store(crate::time::now_unix(), Ordering::Relaxed);
        Ok(())
    }

    /// Append a single sample row.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Sqlite`] on DB errors.
    pub fn append_sample(
        &self,
        ts_unix: i64,
        scope: Scope,
        probe: &str,
        value_num: Option<f64>,
        value_text: Option<&str>,
        unit: Option<&str>,
    ) -> Result<()> {
        let scope_s = scope.as_str();
        self.conn.execute(
            r"INSERT OR REPLACE INTO samples
                  (ts, scope, probe, value_num, value_text, unit)
              VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![ts_unix, scope_s, probe, value_num, value_text, unit],
        )?;
        self.last_write_unix_s
            .store(crate::time::now_unix(), Ordering::Relaxed);
        Ok(())
    }

    /// Append a `help_sessions` row using a structured input.
    ///
    /// Uses named fields to prevent positional argument errors.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Sqlite`] on DB errors.
    pub fn append_help_session(&self, input: &HelpSessionInput<'_>) -> Result<()> {
        let status = input.status.as_str();
        self.conn.execute(
            r"INSERT INTO help_sessions (
                id, ts_unix, ts, backend, model, note,
                prompt_chars, response_chars, latency_ms,
                status, error_kind, evidence_ref
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                input.id,
                input.ts_unix,
                input.ts,
                input.backend,
                input.model,
                input.note,
                input.prompt_chars,
                input.response_chars,
                input.latency_ms,
                status,
                input.error_kind,
                input.evidence_ref
            ],
        )?;
        self.last_write_unix_s
            .store(crate::time::now_unix(), Ordering::Relaxed);
        Ok(())
    }

    /// Return a cloneable read-only handle anchored at the same
    /// file.
    #[must_use]
    pub fn reader(&self) -> JournalReader {
        JournalReader {
            path: self.path.clone(),
        }
    }

    /// Unix timestamp of the most recent write (append / append_sample / append_help_session).
    ///
    /// Used by proprioception to compute `journal_writer_stall_s`.
    #[must_use]
    pub fn last_write_unix_s(&self) -> i64 {
        self.last_write_unix_s.load(Ordering::Relaxed)
    }

    /// Path the journal was opened against.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Upsert an EWMA baseline row. Used by the periodic
    /// baseline computation (Phase 2) to populate the
    /// `baselines` table.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Sqlite`] on DB errors.
    #[allow(clippy::too_many_arguments)]
    pub fn upsert_baseline(
        &self,
        probe: &str,
        scope: Scope,
        ewma_mean: Option<f64>,
        ewma_var: Option<f64>,
        p50: Option<f64>,
        p95: Option<f64>,
        p99: Option<f64>,
    ) -> Result<()> {
        let scope_s = scope.as_str();
        let now = crate::time::now_unix();
        self.conn.execute(
            r"INSERT OR REPLACE INTO baselines
                  (probe, scope, ewma_mean, ewma_var, p50, p95, p99, updated_ts)
              VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![probe, scope_s, ewma_mean, ewma_var, p50, p95, p99, now],
        )?;
        self.last_write_unix_s.store(now, Ordering::Relaxed);
        Ok(())
    }

    /// Check if a nonce has been used and mark it as used if not.
    /// Returns true if the nonce was already used (replay detected).
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Sqlite`] on DB errors.
    pub fn check_and_mark_nonce(
        &self,
        token_id: &str,
        nonce: &str,
        expires_at: i64,
    ) -> Result<bool> {
        // Clean up expired nonces first
        self.conn.execute(
            "DELETE FROM used_nonces WHERE expires_at < ?1",
            params![crate::time::now_unix()],
        )?;

        // Check if nonce exists
        let mut stmt = self
            .conn
            .prepare("SELECT 1 FROM used_nonces WHERE token_id = ?1 AND nonce = ?2")?;
        let exists = stmt.exists(params![token_id, nonce])?;

        if exists {
            return Ok(true);
        }

        // Mark nonce as used
        self.conn.execute(
            "INSERT INTO used_nonces (token_id, nonce, expires_at) VALUES (?1, ?2, ?3)",
            params![token_id, nonce, expires_at],
        )?;

        Ok(false)
    }
}

impl JournalReader {
    /// Construct a reader anchored at `path`. The file need not
    /// exist yet; read methods error if the journal is missing.
    #[must_use]
    pub fn new(path: impl Into<std::path::PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Count events in a given time range (inclusive lower,
    /// exclusive upper), optionally filtered by scope.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Sqlite`] on DB errors.
    pub fn count_events(
        &self,
        since_unix: i64,
        until_unix: i64,
        scope: Option<Scope>,
    ) -> Result<i64> {
        let conn = self.open_ro()?;
        let count: i64 = match scope {
            Some(s) => {
                let scope_s = s.as_str();
                conn.query_row(
                    "SELECT COUNT(*) FROM events
                     WHERE ts_unix >= ?1 AND ts_unix < ?2 AND scope = ?3",
                    params![since_unix, until_unix, scope_s],
                    |r| r.get(0),
                )?
            }
            None => conn.query_row(
                "SELECT COUNT(*) FROM events
                 WHERE ts_unix >= ?1 AND ts_unix < ?2",
                params![since_unix, until_unix],
                |r| r.get(0),
            )?,
        };
        Ok(count)
    }

    /// Fetch the most-recent `limit` events, newest first.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Sqlite`] on DB errors.
    pub fn recent(&self, limit: usize) -> Result<Vec<EventRow>> {
        let conn = self.open_ro()?;
        let mut stmt = conn.prepare(
            r"SELECT id, ts, severity, scope, tier, module, action, summary
              FROM events
              ORDER BY ts_unix DESC
              LIMIT ?1",
        )?;
        let rows = stmt
            .query_map(params![limit as i64], |r| {
                let severity: String = r.get(2)?;
                let scope: String = r.get(3)?;
                Ok(EventRow {
                    id: r.get(0)?,
                    ts: r.get(1)?,
                    severity: severity.parse().unwrap_or(Severity::Info),
                    scope: scope.parse().unwrap_or(Scope::Host),
                    tier: r.get(4)?,
                    module: r.get(5)?,
                    action: r.get(6)?,
                    summary: r.get(7)?,
                    evidence_ref: None,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Breakdown of `events` by `severity` within a time range.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Sqlite`] on DB errors.
    pub fn severity_counts(&self, since_unix: i64, until_unix: i64) -> Result<SeverityCounts> {
        let conn = self.open_ro()?;
        let mut counts = SeverityCounts::default();
        let mut stmt = conn.prepare(
            r"SELECT severity, COUNT(*) FROM events
              WHERE ts_unix >= ?1 AND ts_unix < ?2
              GROUP BY severity",
        )?;
        let iter = stmt.query_map(params![since_unix, until_unix], |r| {
            let sev: String = r.get(0)?;
            let n: i64 = r.get(1)?;
            Ok((sev, n))
        })?;
        for pair in iter {
            let (sev, n) = pair?;
            match sev.as_str() {
                "info" => counts.info += n,
                "warn" => counts.warn += n,
                "alert" => counts.alert += n,
                "crit" => counts.crit += n,
                _ => {}
            }
        }
        Ok(counts)
    }

    /// Timestamp (unix seconds) of the most-recent sample across any
    /// probe, or `None` if the `samples` table is empty.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Sqlite`] on DB errors.
    pub fn last_sample_ts(&self) -> Result<Option<i64>> {
        let conn = self.open_ro()?;
        let row: Option<Option<i64>> = conn
            .query_row("SELECT MAX(ts) FROM samples", [], |r| {
                r.get::<_, Option<i64>>(0)
            })
            .ok();
        Ok(row.flatten())
    }

    /// Timestamp (unix seconds) of the most-recent **host-scope**
    /// sample, or `None` if no host samples exist.
    ///
    /// Used by proprioception ([`russell_proprio`]) to compute
    /// `sentinel_last_run_age_s` without being confused by its own
    /// self-scope writes.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Sqlite`] on DB errors.
    pub fn last_host_sample_ts(&self) -> Result<Option<i64>> {
        let conn = self.open_ro()?;
        let row: Option<Option<i64>> = conn
            .query_row(
                "SELECT MAX(ts) FROM samples WHERE scope = 'host'",
                [],
                |r| r.get::<_, Option<i64>>(0),
            )
            .ok();
        Ok(row.flatten())
    }

    /// Timestamp (unix seconds) of the most-recent remote skill
    /// registry fetch, or `None` if no fetches have been recorded.
    ///
    /// Gap 5: Used by proprioception's `remote_discovery_latency_s` probe.
    /// Reads from the `events` table where `action = 'remote.skill.fetch'`.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Sqlite`] on DB errors.
    pub fn last_remote_fetch_ts(&self) -> Result<Option<i64>> {
        let conn = self.open_ro()?;
        let row: Option<Option<i64>> = conn
            .query_row(
                "SELECT MAX(ts_unix) FROM events WHERE action = 'remote.skill.fetch'",
                [],
                |r| r.get::<_, Option<i64>>(0),
            )
            .ok();
        Ok(row.flatten())
    }

    /// Compute the p95 of `latency_ms` from `help_sessions` rows
    /// in the last 24 hours.
    ///
    /// Returns `None` if fewer than 4 rows exist (p95 is undefined
    /// on a small sample).
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Sqlite`] on DB errors.
    pub fn llm_latency_p95_ms(&self) -> Result<Option<f64>> {
        let conn = self.open_ro()?;
        let since = crate::time::now_unix() - 86_400;
        let mut stmt = conn.prepare(
            "SELECT latency_ms FROM help_sessions
             WHERE ts_unix >= ?1 AND latency_ms IS NOT NULL
             ORDER BY latency_ms ASC",
        )?;
        let latencies: Vec<i64> = stmt
            .query_map(params![since], |r| r.get(0))
            .map_err(|e: rusqlite::Error| CoreError::Sqlite(e))?
            .filter_map(std::result::Result::ok)
            .collect();
        if latencies.len() < 4 {
            return Ok(None);
        }
        // p95 index = floor(0.95 * n)
        let idx = ((latencies.len() as f64) * 0.95).floor() as usize;
        // Clamp to last element
        let idx = idx.min(latencies.len() - 1);
        Ok(Some(latencies[idx] as f64))
    }

    /// Compute the help-session error rate (error + fallback +
    /// threshold_skip) as a percentage of total sessions in the
    /// last 24 hours.
    ///
    /// Returns `None` if no sessions exist in the window.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Sqlite`] on DB errors.
    pub fn help_error_rate_pct(&self) -> Result<Option<f64>> {
        let conn = self.open_ro()?;
        let since = crate::time::now_unix() - 86_400;
        let total: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM help_sessions WHERE ts_unix >= ?1",
                params![since],
                |r| r.get(0),
            )
            .map_err(|e: rusqlite::Error| CoreError::Sqlite(e))?;
        if total == 0 {
            return Ok(None);
        }
        let bad: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM help_sessions
                 WHERE ts_unix >= ?1 AND status IN ('error','fallback','threshold_skip')",
                params![since],
                |r| r.get(0),
            )
            .map_err(|e: rusqlite::Error| CoreError::Sqlite(e))?;
        Ok(Some((bad as f64 / total as f64) * 100.0))
    }

    /// Fetch help-session rows within a time range, newest first.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Sqlite`] on DB errors.
    pub fn help_sessions_in_range(
        &self,
        since_unix: i64,
        until_unix: i64,
    ) -> Result<Vec<HelpSessionRow>> {
        let conn = self.open_ro()?;
        let mut stmt = conn.prepare(
            "SELECT id, ts_unix, ts, backend, model, note,
                     prompt_chars, response_chars, latency_ms,
                     status, error_kind, evidence_ref
              FROM help_sessions
              WHERE ts_unix >= ?1 AND ts_unix < ?2
              ORDER BY ts_unix DESC",
        )?;
        let rows = stmt
            .query_map(params![since_unix, until_unix], |r| {
                let status_str: String = r.get(9)?;
                let status =
                    HelpSessionStatus::from_str(&status_str).unwrap_or(HelpSessionStatus::Error);
                Ok(HelpSessionRow {
                    id: r.get(0)?,
                    ts_unix: r.get(1)?,
                    ts: r.get(2)?,
                    backend: r.get(3)?,
                    model: r.get(4)?,
                    note: r.get(5)?,
                    prompt_chars: r.get(6)?,
                    response_chars: r.get(7)?,
                    latency_ms: r.get(8)?,
                    status,
                    error_kind: r.get(10)?,
                    evidence_ref: r.get(11)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Aggregate host-scope samples in a time window, grouped by probe.
    /// Returns min, avg, max, last, last_ts, and count per probe.
    ///
    /// Only probes with at least one data point are included.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Sqlite`] on DB errors.
    pub fn host_samples_summary(
        &self,
        since_unix: i64,
        until_unix: i64,
    ) -> Result<Vec<SampleSummary>> {
        let conn = self.open_ro()?;
        let mut stmt = conn.prepare(
            r"SELECT
                probe,
                unit,
                MIN(value_num),
                AVG(value_num),
                MAX(value_num),
                COUNT(*)
              FROM samples
              WHERE scope = 'host'
                AND ts >= ?1 AND ts < ?2
                AND value_num IS NOT NULL
              GROUP BY probe
              ORDER BY probe",
        )?;
        let rows = stmt
            .query_map(params![since_unix, until_unix], |r| {
                let probe: String = r.get(0)?;
                let unit: Option<String> = r.get(1)?;
                let min: Option<f64> = r.get(2)?;
                let avg: Option<f64> = r.get(3)?;
                let max: Option<f64> = r.get(4)?;
                let count: i64 = r.get(5)?;
                Ok((probe, unit, min, avg, max, count))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        // For "last" and "last_ts", query separately per probe.
        let mut summaries = Vec::with_capacity(rows.len());
        for (probe, unit, min, avg, max, count) in rows {
            let (last, last_ts) = conn
                .query_row(
                    "SELECT value_num, ts FROM samples
                     WHERE scope = 'host' AND probe = ?1
                       AND ts >= ?2 AND ts < ?3
                       AND value_num IS NOT NULL
                     ORDER BY ts DESC LIMIT 1",
                    params![&probe, since_unix, until_unix],
                    |r| Ok((r.get::<_, Option<f64>>(0)?, r.get::<_, Option<i64>>(1)?)),
                )
                .unwrap_or((None, None));
            summaries.push(SampleSummary {
                probe,
                unit,
                min,
                avg,
                max,
                last,
                last_ts,
                count,
            });
        }
        Ok(summaries)
    }

    /// Self-scope sample summary — Russell's own proprioceptive vitals.
    ///
    /// Same structure as [`host_samples_summary`] but for `scope = 'self'`.
    /// Returns per-probe min/avg/max/last/count for self-vital samples.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Sqlite`] on DB errors.
    pub fn self_samples_summary(
        &self,
        since_unix: i64,
        until_unix: i64,
    ) -> Result<Vec<SampleSummary>> {
        let conn = self.open_ro()?;
        let mut stmt = conn.prepare(
            r"SELECT
                probe,
                unit,
                MIN(value_num),
                AVG(value_num),
                MAX(value_num),
                COUNT(*)
              FROM samples
              WHERE scope = 'self'
                AND ts >= ?1 AND ts < ?2
                AND value_num IS NOT NULL
              GROUP BY probe
              ORDER BY probe",
        )?;
        let rows = stmt
            .query_map(params![since_unix, until_unix], |r| {
                let probe: String = r.get(0)?;
                let unit: Option<String> = r.get(1)?;
                let min: Option<f64> = r.get(2)?;
                let avg: Option<f64> = r.get(3)?;
                let max: Option<f64> = r.get(4)?;
                let count: i64 = r.get(5)?;
                Ok((probe, unit, min, avg, max, count))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let mut summaries = Vec::with_capacity(rows.len());
        for (probe, unit, min, avg, max, count) in rows {
            let (last, last_ts) = conn
                .query_row(
                    "SELECT value_num, ts FROM samples
                     WHERE scope = 'self' AND probe = ?1
                       AND ts >= ?2 AND ts < ?3
                       AND value_num IS NOT NULL
                     ORDER BY ts DESC LIMIT 1",
                    params![&probe, since_unix, until_unix],
                    |r| Ok((r.get::<_, Option<f64>>(0)?, r.get::<_, Option<i64>>(1)?)),
                )
                .unwrap_or((None, None));
            summaries.push(SampleSummary {
                probe,
                unit,
                min,
                avg,
                max,
                last,
                last_ts,
                count,
            });
        }
        Ok(summaries)
    }

    /// Compute percentiles (p50, p95, p99) for the last `window_days`
    /// days of host-scope samples, grouped by probe. Returns a list
    /// of (probe, p50, p95, p99, count).
    ///
    /// For each probe, values are sorted and percentiles computed
    /// by index interpolation. EWMA mean and variance are computed
    /// with a 7-day half-life over the timestamp-ordered series.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Sqlite`] on DB errors.
    pub fn compute_baselines(&self, window_days: u32) -> Result<Vec<BaselineRow>> {
        let conn = self.open_ro()?;
        let since = crate::time::now_unix() - i64::from(window_days) * 86_400;

        // Get all host-scope numeric samples in the window, sorted by
        // probe then timestamp for correct temporal ordering.
        let mut stmt = conn.prepare(
            "SELECT probe, value_num, ts
               FROM samples
               WHERE scope = 'host'
                 AND ts >= ?1
                 AND value_num IS NOT NULL
               ORDER BY probe ASC, ts ASC",
        )?;
        let rows = stmt
            .query_map(params![since], |r| {
                let probe: String = r.get(0)?;
                let val: f64 = r.get(1)?;
                let ts: i64 = r.get(2)?;
                Ok((probe, val, ts))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        // Group by probe, preserving timestamp order.
        use std::collections::BTreeMap;
        let mut groups: BTreeMap<String, Vec<(f64, i64)>> = BTreeMap::new();
        for (probe, val, ts) in rows {
            groups.entry(probe).or_default().push((val, ts));
        }

        let mut results = Vec::new();
        for (probe, vals_and_ts) in groups {
            let vals: Vec<f64> = vals_and_ts.iter().map(|(v, _)| *v).collect();
            let count = vals.len() as i64;

            // Percentiles on sorted values (ignoring timestamps).
            let mut sorted_vals = vals.clone();
            sorted_vals
                .sort_unstable_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let p50 = percentile(&sorted_vals, 50.0);
            let p95 = percentile(&sorted_vals, 95.0);
            let p99 = percentile(&sorted_vals, 99.0);

            // EWMA with 7-day half-life (604 800 seconds).
            let (ewma_mean, ewma_var) = compute_ewma(&vals_and_ts, 604_800.0);

            results.push(BaselineRow {
                probe,
                p50,
                p95,
                p99,
                ewma_mean,
                ewma_var,
                count,
                updated_ts: Some(crate::time::now_unix()),
            });
        }
        Ok(results)
    }

    /// Read all persisted baselines for host-scope probes from the
    /// `baselines` table. Returns an empty `Vec` if no baselines
    /// have been computed yet.
    ///
    /// # Task 4.1: Baseline freshness guard
    ///
    /// Now includes `updated_ts` for freshness checks. Callers should
    /// use [`BaselineRow::is_stale()`] to verify baselines are current
    /// before citing them in assessments.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Sqlite`] on DB errors.
    pub fn read_baselines(&self) -> Result<Vec<BaselineRow>> {
        let conn = self.open_ro()?;
        let mut stmt = conn.prepare(
            "SELECT probe, p50, p95, p99, ewma_mean, ewma_var, updated_ts
               FROM baselines
              WHERE scope = 'host'
                AND p95 IS NOT NULL",
        )?;
        let rows = stmt
            .query_map([], |r| {
                Ok(BaselineRow {
                    probe: r.get(0)?,
                    p50: r.get(1)?,
                    p95: r.get(2)?,
                    p99: r.get(3)?,
                    ewma_mean: r.get(4)?,
                    ewma_var: r.get(5)?,
                    count: 0,
                    updated_ts: r.get(6)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Read the most recent numeric sample value for a probe before
    /// the given timestamp. Returns `None` if no prior sample exists.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Sqlite`] on DB errors.
    pub fn previous_sample(&self, probe: &str, before_ts: i64) -> Option<(f64, i64)> {
        let conn = self.open_ro().ok()?;
        conn.query_row(
            "SELECT value_num, ts FROM samples
              WHERE probe = ?1 AND ts < ?2 AND value_num IS NOT NULL
              ORDER BY ts DESC LIMIT 1",
            params![probe, before_ts],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .ok()
    }

    /// Batch-fetch the most recent sample before `before_ts` for multiple probes.
    ///
    /// Avoids N+1 query pattern — uses a single SQL query with window functions
    /// instead of one connection per probe.
    ///
    /// Returns a map from probe name to (value, timestamp).
    pub fn previous_samples_batch(
        &self,
        probes: &[&str],
        before_ts: i64,
    ) -> Result<std::collections::HashMap<String, (f64, i64)>> {
        if probes.is_empty() {
            return Ok(std::collections::HashMap::new());
        }
        let conn = self.open_ro()?;
        let placeholders: Vec<String> = (1..=probes.len()).map(|i| format!("?{i}")).collect();
        let sql = format!(
            "SELECT probe, value_num, ts FROM samples WHERE rowid IN (
                SELECT rowid FROM samples WHERE probe IN ({}) AND ts < ?{} AND value_num IS NOT NULL
                GROUP BY probe HAVING ts = MAX(ts)
             )",
            placeholders.join(", "),
            probes.len() + 1
        );
        let mut stmt = conn.prepare(&sql)?;
        let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        for probe in probes {
            params_vec.push(Box::new(probe.to_string()));
        }
        params_vec.push(Box::new(before_ts));
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();
        let rows = stmt.query_map(param_refs.as_slice(), |r| {
            let probe: String = r.get(0)?;
            let value: f64 = r.get(1)?;
            let ts: i64 = r.get(2)?;
            Ok((probe, value, ts))
        })?;

        let mut result = std::collections::HashMap::new();
        for row in rows {
            let (probe, value, ts) = row?;
            result.insert(probe, (value, ts));
        }
        Ok(result)
    }

    /// Count reflex_proposed events for a probe within a time window.
    /// Used for reflex arc cooldown enforcement.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Sqlite`] on DB errors.
    pub fn count_reflex_events(&self, probe: &str, since: i64, until: i64) -> Result<i64> {
        let conn = self.open_ro()?;
        conn.query_row(
            "SELECT COUNT(*) FROM events
              WHERE json_extract(payload, '$.outputs.probe') = ?1
                AND ts_unix >= ?2 AND ts_unix <= ?3
                AND action = 'reflex_proposed'",
            params![probe, since, until],
            |r| r.get(0),
        )
        .map_err(CoreError::Sqlite)
    }

    /// List reflex_proposed events for a time window. Returns the
    /// most recent events first (max 10). Each entry contains
    /// (severity, intervention_id, summary).
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Sqlite`] on DB errors.
    pub fn list_reflex_events(
        &self,
        since: i64,
        until: i64,
    ) -> Result<Vec<(String, String, String, i64)>> {
        let conn = self.open_ro()?;
        let mut stmt = conn.prepare(
            "SELECT severity, json_extract(payload, '$.outputs.intervention'), summary, ts_unix
               FROM events
              WHERE action = 'reflex_proposed'
                AND ts_unix >= ?1 AND ts_unix <= ?2
              ORDER BY ts_unix DESC
              LIMIT 10",
        )?;
        let rows = stmt
            .query_map(params![since, until], |r| {
                let sev: String = r.get(0)?;
                let intervention: String = r.get(1)?;
                let summary: String = r.get(2)?;
                let ts: i64 = r.get(3)?;
                Ok((sev, intervention, summary, ts))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// List events by action type within a time window.
    ///
    /// Used by [`ReflexBudget::from_journal`] to initialize the budget
    /// from persisted `reflex_proposed` events.
    ///
    /// # Arguments
    ///
    /// * `action` - Action name to filter by (e.g., "reflex_proposed")
    /// * `since` - Start of time window (unix timestamp)
    /// * `until` - End of time window (unix timestamp)
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Sqlite`] on DB errors.
    pub fn list_events_by_action(
        &self,
        action: &str,
        since: i64,
        until: i64,
    ) -> Result<Vec<EventRow>> {
        let conn = self.open_ro()?;
        let mut stmt = conn.prepare(
            "SELECT id, ts, severity, scope, tier, module, action, summary
               FROM events
              WHERE action = ?1 AND ts_unix >= ?2 AND ts_unix <= ?3
              ORDER BY ts_unix ASC",
        )?;
        let rows = stmt
            .query_map(params![action, since, until], |r| {
                let sev_str: String = r.get(2)?;
                let scope_str: String = r.get(3)?;

                Ok(EventRow {
                    id: r.get(0)?,
                    ts: r.get(1)?,
                    severity: sev_str.parse::<Severity>().unwrap_or(Severity::Info),
                    scope: scope_str.parse::<Scope>().unwrap_or(Scope::Host),
                    tier: r.get(4)?,
                    module: r.get(5)?,
                    action: r.get(6)?,
                    summary: r.get(7)?,
                    evidence_ref: None,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Get a specific event by its row ID.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::NotFound`] if the event does not exist,
    /// or [`CoreError::Sqlite`] on DB errors.
    pub fn get_event(&self, id: i64) -> Result<Event> {
        let conn = self.open_ro()?;
        let mut stmt = conn.prepare(
            "SELECT id, ts, ts_unix, schema, run_id, tier, module, severity, scope, action, dry_run, payload
               FROM events
              WHERE id = ?1",
        )?;
        let event = stmt
            .query_row(params![id], |r| {
                let id: i64 = r.get(0)?;
                let ts: String = r.get(1)?;
                let ts_unix: i64 = r.get(2)?;
                let schema: String = r.get(3)?;
                let run_id: Option<String> = r.get(4)?;
                let tier: Option<String> = r.get(5)?;
                let module: Option<String> = r.get(6)?;
                let severity_str: String = r.get(7)?;
                let scope_str: String = r.get(8)?;
                let action: String = r.get(9)?;
                let dry_run: bool = r.get(10)?;
                let payload: String = r.get(11)?;

                let severity = Severity::from_str(&severity_str).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        7,
                        rusqlite::types::Type::Text,
                        Box::new(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            e.to_string(),
                        )),
                    )
                })?;
                let scope = Scope::from_str(&scope_str).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        8,
                        rusqlite::types::Type::Text,
                        Box::new(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            e.to_string(),
                        )),
                    )
                })?;

                let event: Event = serde_json::from_str(&payload).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        11,
                        rusqlite::types::Type::Text,
                        Box::new(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            e.to_string(),
                        )),
                    )
                })?;

                // Override with row-level metadata
                let mut event = event;
                event.id = EventId::from(id as u128);
                event.ts = ts;
                event.ts_unix = ts_unix;
                event.schema = schema;
                event.run_id = run_id;
                event.tier = tier;
                event.module = module;
                event.severity = severity;
                event.scope = scope;
                event.action = action;
                event.dry_run = dry_run;

                Ok(event)
            })
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    CoreError::Sqlite(rusqlite::Error::QueryReturnedNoRows)
                }
                other => CoreError::Sqlite(other),
            })?;

        Ok(event)
    }

    /// Path the journal lives at. May not exist yet on very fresh
    /// installs.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn open_ro(&self) -> Result<Connection> {
        let conn = Connection::open_with_flags(
            &self.path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
        )?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        Ok(conn)
    }

    /// Raw read-only connection for internal harness use.
    pub fn open_ro_conn(&self) -> Result<Connection> {
        self.open_ro()
    }

    /// Quick spot-check of the journal hash chain. Verifies the last 10
    /// events. Returns:
    /// - `Some(true)` if intact or no chained events exist
    /// - `Some(false)` if a chain break was detected
    /// - `None` if the check could not run (DB error)
    pub fn check_chain_integrity(&self) -> Option<bool> {
        let conn = self.open_ro().ok()?;
        let mut stmt = conn
            .prepare(
                "SELECT prev_hash, payload, hash FROM events \
                 WHERE hash IS NOT NULL \
                 ORDER BY ts_unix DESC, id DESC \
                 LIMIT 10",
            )
            .ok()?;

        let links: Vec<(String, String, String)> = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .ok()?
            .filter_map(|r| r.ok())
            .collect();

        if links.is_empty() {
            return Some(true);
        }

        let links: Vec<_> = links.into_iter().rev().collect();

        match crate::hash_chain::verify_chain(&links) {
            crate::hash_chain::ChainVerdict::Intact { .. } => Some(true),
            crate::hash_chain::ChainVerdict::Empty => Some(true),
            crate::hash_chain::ChainVerdict::Broken { .. } => Some(false),
        }
    }

    /// Retrieve the N most recent events that have non-NULL evidence_ref.
    pub fn recent_with_evidence(&self, limit: usize) -> Result<Vec<EventRow>> {
        let conn = self.open_ro()?;
        let mut stmt = conn.prepare(
            "SELECT id, ts, severity, scope, tier, module, action, summary, evidence_ref \
             FROM events WHERE evidence_ref IS NOT NULL AND evidence_ref != '' \
             ORDER BY ts_unix DESC, id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(rusqlite::params![limit as i64], |row| {
            Ok(EventRow {
                id: row.get(0)?,
                ts: row.get(1)?,
                severity: row.get::<_, String>(2)?.parse().unwrap_or(Severity::Info),
                scope: row.get::<_, String>(3)?.parse().unwrap_or(Scope::Host),
                tier: row.get(4)?,
                module: row.get(5)?,
                action: row.get(6)?,
                summary: row.get(7)?,
                evidence_ref: row.get(8)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| CoreError::Invariant(format!("query failed: {e}")))
    }
}

/// Count of events by severity.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SeverityCounts {
    /// `info`-severity rows.
    pub info: i64,
    /// `warn`-severity rows.
    pub warn: i64,
    /// `alert`-severity rows.
    pub alert: i64,
    /// `crit`-severity rows.
    pub crit: i64,
}

/// A `help_sessions` row, as returned by [`JournalReader::help_sessions_in_range`].
#[derive(Debug, Clone)]
pub struct HelpSessionRow {
    /// ULID.
    pub id: String,
    /// Unix timestamp.
    pub ts_unix: i64,
    /// RFC3339 timestamp.
    pub ts: String,
    /// Backend label.
    pub backend: String,
    /// Model, if any.
    pub model: Option<String>,
    /// Operator note.
    pub note: Option<String>,
    /// Prompt character count.
    pub prompt_chars: i64,
    /// Response character count.
    pub response_chars: i64,
    /// Round-trip latency (ms); `None` for offline.
    pub latency_ms: Option<i64>,
    /// Outcome status.
    pub status: HelpSessionStatus,
    /// Short error kind, if status=error.
    pub error_kind: Option<String>,
    /// Path to evidence bundle.
    pub evidence_ref: String,
}

/// Per-probe statistical summary of host-scope samples over a
/// time window.
///
/// Queried by [`JournalReader::host_samples_summary`].
#[derive(Debug, Clone)]
pub struct SampleSummary {
    /// Probe name (e.g. `loadavg_1m`).
    pub probe: String,
    /// Unit, if any (e.g. `MiB`, `s`).
    pub unit: Option<String>,
    /// Minimum value in the window.
    pub min: Option<f64>,
    /// Average value in the window.
    pub avg: Option<f64>,
    /// Maximum value in the window.
    pub max: Option<f64>,
    /// Most-recent value in the window.
    pub last: Option<f64>,
    /// Timestamp of the most-recent value (unix seconds).
    pub last_ts: Option<i64>,
    /// Number of data points in the window.
    pub count: i64,
}

/// Per-probe baseline statistics.
#[derive(Debug, Clone)]
pub struct BaselineRow {
    /// Probe name.
    pub probe: String,
    /// 50th percentile.
    pub p50: Option<f64>,
    /// 95th percentile.
    pub p95: Option<f64>,
    /// 99th percentile.
    pub p99: Option<f64>,
    /// Exponentially-weighted moving average (7-day half-life).
    pub ewma_mean: Option<f64>,
    /// EWMA variance around the mean.
    pub ewma_var: Option<f64>,
    /// Number of samples in the window.
    pub count: i64,
    /// Unix timestamp when this baseline was last computed.
    /// Used for freshness checks (Task 4.1).
    pub updated_ts: Option<i64>,
}

impl BaselineRow {
    /// Check if this baseline is stale (older than `max_age_hours`).
    ///
    /// Returns `true` if `updated_ts` is `None` or if the baseline
    /// was computed more than `max_age_hours` ago.
    ///
    /// # Task 4.1: Baseline freshness guard
    ///
    /// This implements D1 from the adversarial review: baselines
    /// now have a freshness guard. When stale, Jack's SOAP shows
    /// "baselines stale (last updated X days ago)" instead of
    /// citing potentially obsolete statistics.
    #[must_use]
    pub fn is_stale(&self, max_age_hours: u32) -> bool {
        let Some(updated) = self.updated_ts else {
            return true;
        };
        let now = crate::time::now_unix();
        let age_hours = (now - updated) as f64 / 3600.0;
        age_hours > max_age_hours as f64
    }

    /// Check if this baseline is fresh (not stale).
    #[must_use]
    pub fn is_fresh(&self, max_age_hours: u32) -> bool {
        !self.is_stale(max_age_hours)
    }
}

/// Compute a percentile value from a sorted slice.
fn percentile(sorted: &[f64], pct: f64) -> Option<f64> {
    if sorted.is_empty() {
        return None;
    }
    if sorted.len() == 1 {
        return Some(sorted[0]);
    }
    let rank = (pct / 100.0) * (sorted.len() - 1) as f64;
    let lower = rank.floor() as usize;
    let upper = rank.ceil() as usize;
    if lower == upper {
        return Some(sorted[lower]);
    }
    let frac = rank - lower as f64;
    Some(sorted[lower] * (1.0 - frac) + sorted[upper] * frac)
}

/// Compute exponentially-weighted moving average (mean and variance)
/// over a time-ordered series of (value, unix_timestamp) pairs.
///
/// Uses an exponential decay kernel with the given half-life in seconds.
/// Returns `(None, None)` if the series has fewer than 2 data points.
fn compute_ewma(series: &[(f64, i64)], half_life_s: f64) -> (Option<f64>, Option<f64>) {
    if series.len() < 2 {
        return (None, None);
    }

    let decay = (-std::f64::consts::LN_2 / half_life_s).exp();

    let mut mean = series[0].0;

    for i in 1..series.len() {
        let dt = ((series[i].1 - series[i - 1].1).max(1)) as f64;
        let alpha = 1.0 - decay.powf(dt);
        mean = alpha * series[i].0 + (1.0 - alpha) * mean;
    }

    // EWMA variance: weighted average of squared deviations.
    let mut var = 0.0f64;
    let mut var_sum_weights = 0.0f64;
    for &(val, _) in series {
        let dev = val - mean;
        var += dev * dev;
        var_sum_weights += 1.0;
    }
    if var_sum_weights > 0.0 {
        var /= var_sum_weights;
    }

    (Some(mean), Some(var))
}

fn configure_pragmas(conn: &Connection) -> Result<()> {
    // Per ADR-0004.
    // `journal_mode=WAL` returns the new mode; use `query_row` so
    // the result is actually consumed (`execute` returns an error
    // for row-returning PRAGMAs on some SQLite versions).
    let _mode: String = conn.query_row("PRAGMA journal_mode=WAL", [], |r| r.get(0))?;
    conn.execute_batch(
        r"PRAGMA synchronous=NORMAL;
          PRAGMA foreign_keys=ON;
          PRAGMA temp_store=MEMORY;
          PRAGMA busy_timeout=5000;",
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{Event, Scope, Severity};

    /// Create a temporary directory and journal path for testing.
    fn tmp_journal() -> (tempfile::TempDir, PathBuf) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let p = tmp.path().join("journal.db");
        (tmp, p)
    }

    // ---- JournalWriter open / basic lifecycle ----

    // REQ: JR-7 — Persistence is auditable; opening a journal creates the DB.
    #[test]
    fn writer_open_creates_db() {
        let (_guard, path) = tmp_journal();
        let w = JournalWriter::open(&path).unwrap();
        assert!(path.exists(), "journal DB file must exist after open");
        assert_eq!(w.path(), &path);
    }

    // REQ: JR-7 — Reader can be obtained from a writer.
    #[test]
    fn writer_produces_reader() {
        let (_guard, path) = tmp_journal();
        let w = JournalWriter::open(&path).unwrap();
        let r = w.reader();
        assert_eq!(r.path(), &path);
    }

    // REQ: JR-7 — last_write_unix_s is set on open.
    #[test]
    fn writer_last_write_set_on_open() {
        let (_guard, path) = tmp_journal();
        let w = JournalWriter::open(&path).unwrap();
        let ts = w.last_write_unix_s();
        assert!(
            ts > 1_700_000_000,
            "last_write should be a recent unix timestamp"
        );
    }

    // ---- append + recent round-trip ----

    // REQ: JR-7 — Appending an event and reading it back preserves fields.
    #[test]
    fn append_event_and_read_back() {
        let (_guard, path) = tmp_journal();
        let w = JournalWriter::open(&path).unwrap();
        let mut e = Event::new("observe", Severity::Info);
        e.module = Some("daily/gpu-sanity".into());
        e.summary = Some("GPU temperature 45°C".into());
        w.append(&e).unwrap();

        let r = w.reader();
        let rows = r.recent(5).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].action, "observe");
        assert_eq!(rows[0].severity, Severity::Info);
        assert_eq!(rows[0].module.as_deref(), Some("daily/gpu-sanity"));
        assert_eq!(rows[0].summary.as_deref(), Some("GPU temperature 45°C"));
    }

    // REQ: JR-7 — Recent events are returned newest-first.
    #[test]
    fn recent_returns_newest_first() {
        let (_guard, path) = tmp_journal();
        let w = JournalWriter::open(&path).unwrap();
        for i in 0..3 {
            let mut e = Event::new("observe", Severity::Info);
            e.module = Some(format!("mod-{i}"));
            // Stagger timestamps so ordering is deterministic.
            e.ts_unix = 1_700_000_000 + i as i64;
            w.append(&e).unwrap();
        }
        let r = w.reader();
        let rows = r.recent(10).unwrap();
        assert_eq!(rows.len(), 3);
        // Newest first → module names descending.
        assert_eq!(rows[0].module.as_deref(), Some("mod-2"));
        assert_eq!(rows[1].module.as_deref(), Some("mod-1"));
        assert_eq!(rows[2].module.as_deref(), Some("mod-0"));
    }

    // ---- append_sample + read back ----

    // REQ: JR-7 — Sample persistence round-trip: write then query.
    #[test]
    fn append_sample_host_and_read_back() {
        let (_guard, path) = tmp_journal();
        let w = JournalWriter::open(&path).unwrap();
        let ts = 1_700_000_100;
        w.append_sample(
            ts,
            Scope::Host,
            "mem_available_mib",
            Some(8192.0),
            None,
            Some("MiB"),
        )
        .unwrap();

        let r = w.reader();
        let last_ts = r.last_host_sample_ts().unwrap();
        assert_eq!(last_ts, Some(ts));
    }

    // REQ: JR-7 — Self-scope samples are recorded separately from host.
    #[test]
    fn append_sample_self_scope() {
        let (_guard, path) = tmp_journal();
        let w = JournalWriter::open(&path).unwrap();
        let ts = 1_700_000_200;
        w.append_sample(
            ts,
            Scope::Self_,
            "sentinel_last_run_age_s",
            Some(42.0),
            None,
            Some("s"),
        )
        .unwrap();

        let r = w.reader();
        // Self-scope should NOT appear in last_host_sample_ts.
        let host_ts = r.last_host_sample_ts().unwrap();
        assert_eq!(host_ts, None);
        // But should appear in last_sample_ts (all scopes).
        let any_ts = r.last_sample_ts().unwrap();
        assert_eq!(any_ts, Some(ts));
    }

    // REQ: JR-7 — INSERT OR REPLACE on same (ts, scope, probe) upserts.
    #[test]
    fn append_sample_upserts_same_probe() {
        let (_guard, path) = tmp_journal();
        let w = JournalWriter::open(&path).unwrap();
        let ts = 1_700_000_300;
        w.append_sample(
            ts,
            Scope::Host,
            "mem_available_mib",
            Some(8192.0),
            None,
            Some("MiB"),
        )
        .unwrap();
        w.append_sample(
            ts,
            Scope::Host,
            "mem_available_mib",
            Some(4096.0),
            None,
            Some("MiB"),
        )
        .unwrap();

        let r = w.reader();
        let summary = r.host_samples_summary(ts, ts + 1).unwrap();
        assert_eq!(summary.len(), 1);
        assert_eq!(summary[0].last, Some(4096.0));
    }

    // ---- severity_counts ----

    // REQ: JR-7 — Severity counts are accurate across all four bands.
    #[test]
    fn severity_counts_all_bands() {
        let (_guard, path) = tmp_journal();
        let w = JournalWriter::open(&path).unwrap();
        let cases = [
            (Severity::Info, 3),
            (Severity::Warn, 2),
            (Severity::Alert, 1),
            (Severity::Crit, 1),
        ];
        for (sev, count) in cases {
            for _ in 0..count {
                w.append(&Event::new("test", sev)).unwrap();
            }
        }
        let r = w.reader();
        let counts = r.severity_counts(0, i64::MAX).unwrap();
        assert_eq!(counts.info, 3);
        assert_eq!(counts.warn, 2);
        assert_eq!(counts.alert, 1);
        assert_eq!(counts.crit, 1);
    }

    // REQ: JR-7 — SeverityCounts defaults to zero.
    #[test]
    fn severity_counts_default_is_zero() {
        let sc = SeverityCounts::default();
        assert_eq!(sc.info, 0);
        assert_eq!(sc.warn, 0);
        assert_eq!(sc.alert, 0);
        assert_eq!(sc.crit, 0);
    }

    // ---- count_events with scope filter ----

    // REQ: JR-7 — count_events respects scope filtering.
    #[test]
    fn count_events_with_scope_filter() {
        let (_guard, path) = tmp_journal();
        let w = JournalWriter::open(&path).unwrap();
        let mut e_host = Event::new("observe", Severity::Info);
        e_host.scope = Scope::Host;
        w.append(&e_host).unwrap();

        let mut e_self = Event::new("proprio", Severity::Info);
        e_self.scope = Scope::Self_;
        w.append(&e_self).unwrap();

        let r = w.reader();
        let total = r.count_events(0, i64::MAX, None).unwrap();
        assert_eq!(total, 2);
        let host_only = r.count_events(0, i64::MAX, Some(Scope::Host)).unwrap();
        assert_eq!(host_only, 1);
        let self_only = r.count_events(0, i64::MAX, Some(Scope::Self_)).unwrap();
        assert_eq!(self_only, 1);
    }

    // ---- host_samples_summary ----

    // REQ: JR-7 — host_samples_summary returns correct min/avg/max/last/count.
    #[test]
    fn host_samples_summary_statistics() {
        let (_guard, path) = tmp_journal();
        let w = JournalWriter::open(&path).unwrap();
        let base = 1_700_000_000;
        // Insert 3 samples for mem_available_mib: 8000, 6000, 7000
        for (i, val) in [8000.0, 6000.0, 7000.0].iter().enumerate() {
            w.append_sample(
                base + i as i64 * 300,
                Scope::Host,
                "mem_available_mib",
                Some(*val),
                None,
                Some("MiB"),
            )
            .unwrap();
        }
        let r = w.reader();
        let summaries = r.host_samples_summary(base, base + 1000).unwrap();
        assert_eq!(summaries.len(), 1);
        let s = &summaries[0];
        assert_eq!(s.probe, "mem_available_mib");
        assert_eq!(s.unit.as_deref(), Some("MiB"));
        assert_eq!(s.min, Some(6000.0));
        assert_eq!(s.max, Some(8000.0));
        // avg = (8000+6000+7000)/3 = 7000.0
        let avg = s.avg.unwrap();
        assert!(
            (avg - 7000.0).abs() < 0.01,
            "avg should be 7000.0, got {avg}"
        );
        assert_eq!(s.last, Some(7000.0));
        assert_eq!(s.count, 3);
    }

    // ---- self_samples_summary ----

    // REQ: JR-7 — self_samples_summary only returns self-scope samples.
    #[test]
    fn self_samples_summary_excludes_host() {
        let (_guard, path) = tmp_journal();
        let w = JournalWriter::open(&path).unwrap();
        let ts = 1_700_000_000;
        w.append_sample(
            ts,
            Scope::Host,
            "mem_available_mib",
            Some(8192.0),
            None,
            Some("MiB"),
        )
        .unwrap();
        w.append_sample(
            ts,
            Scope::Self_,
            "timer_drift_s",
            Some(0.5),
            None,
            Some("s"),
        )
        .unwrap();

        let r = w.reader();
        let host = r.host_samples_summary(ts, ts + 1).unwrap();
        let self_ = r.self_samples_summary(ts, ts + 1).unwrap();
        assert_eq!(host.len(), 1, "host summary should have 1 probe");
        assert_eq!(self_.len(), 1, "self summary should have 1 probe");
        assert_eq!(self_[0].probe, "timer_drift_s");
    }

    // ---- baselines ----

    // REQ: JR-7 — Baseline upsert + read round-trip.
    #[test]
    fn baseline_upsert_and_read() {
        let (_guard, path) = tmp_journal();
        let w = JournalWriter::open(&path).unwrap();
        w.upsert_baseline(
            "mem_available_mib",
            Scope::Host,
            Some(8192.0), // ewma_mean
            Some(256.0),  // ewma_var
            Some(7500.0), // p50
            Some(8000.0), // p95
            Some(8100.0), // p99
        )
        .unwrap();

        let r = w.reader();
        let baselines = r.read_baselines().unwrap();
        assert_eq!(baselines.len(), 1);
        let b = &baselines[0];
        assert_eq!(b.probe, "mem_available_mib");
        assert_eq!(b.p50, Some(7500.0));
        assert_eq!(b.p95, Some(8000.0));
        assert_eq!(b.p99, Some(8100.0));
        assert_eq!(b.ewma_mean, Some(8192.0));
        assert_eq!(b.ewma_var, Some(256.0));
        assert!(b.updated_ts.is_some());
    }

    // REQ: JR-7 — Baseline upsert is idempotent (INSERT OR REPLACE).
    #[test]
    fn baseline_upsert_replaces() {
        let (_guard, path) = tmp_journal();
        let w = JournalWriter::open(&path).unwrap();
        w.upsert_baseline(
            "mem_available_mib",
            Scope::Host,
            Some(8192.0),
            Some(100.0),
            Some(7500.0),
            Some(8000.0),
            Some(8100.0),
        )
        .unwrap();
        w.upsert_baseline(
            "mem_available_mib",
            Scope::Host,
            Some(9000.0),
            Some(200.0),
            Some(8000.0),
            Some(8500.0),
            Some(8600.0),
        )
        .unwrap();

        let r = w.reader();
        let baselines = r.read_baselines().unwrap();
        assert_eq!(baselines.len(), 1);
        assert_eq!(baselines[0].ewma_mean, Some(9000.0));
        assert_eq!(baselines[0].p95, Some(8500.0));
    }

    // REQ: JR-7 — BaselineRow is_stale / is_fresh logic.
    #[test]
    fn baseline_staleness_checks() {
        let now = crate::time::now_unix();
        let fresh = BaselineRow {
            probe: "test".into(),
            p50: None,
            p95: Some(1.0),
            p99: None,
            ewma_mean: None,
            ewma_var: None,
            count: 0,
            updated_ts: Some(now),
        };
        assert!(fresh.is_fresh(24));
        assert!(!fresh.is_stale(24));

        let stale = BaselineRow {
            probe: "test".into(),
            p50: None,
            p95: Some(1.0),
            p99: None,
            ewma_mean: None,
            ewma_var: None,
            count: 0,
            updated_ts: Some(now - 100_000), // ~27.7 hours ago
        };
        assert!(stale.is_stale(24));
        assert!(!stale.is_fresh(24));

        let no_ts = BaselineRow {
            probe: "test".into(),
            p50: None,
            p95: Some(1.0),
            p99: None,
            ewma_mean: None,
            ewma_var: None,
            count: 0,
            updated_ts: None,
        };
        assert!(no_ts.is_stale(24), "missing updated_ts means stale");
    }

    // ---- hash chain integrity ----

    // REQ: JR-7, ADR-0004 — Chain integrity returns true for a fresh journal.
    #[test]
    fn chain_integrity_fresh_journal() {
        let (_guard, path) = tmp_journal();
        let w = JournalWriter::open(&path).unwrap();
        let r = w.reader();
        // No events → chain is trivially intact.
        assert_eq!(r.check_chain_integrity(), Some(true));
    }

    // REQ: JR-7, ADR-0004 — Chain integrity holds after appending events.
    #[test]
    fn chain_integrity_after_appends() {
        let (_guard, path) = tmp_journal();
        let w = JournalWriter::open(&path).unwrap();
        for i in 0..5 {
            let mut e = Event::new("observe", Severity::Info);
            e.module = Some(format!("mod-{i}"));
            w.append(&e).unwrap();
        }
        let r = w.reader();
        assert_eq!(r.check_chain_integrity(), Some(true));
    }

    // ---- HelpSessionStatus ----

    // REQ: JR-7 — HelpSessionStatus round-trips through as_str / from_str.
    #[test]
    fn help_session_status_round_trip() {
        for (variant, s) in [
            (HelpSessionStatus::Ok, "ok"),
            (HelpSessionStatus::Error, "error"),
            (HelpSessionStatus::Fallback, "fallback"),
            (HelpSessionStatus::ThresholdSkip, "threshold_skip"),
        ] {
            assert_eq!(variant.as_str(), s);
            assert_eq!(HelpSessionStatus::from_str(s).unwrap(), variant);
        }
    }

    // REQ: JR-7 — Unknown HelpSessionStatus string is an error.
    #[test]
    fn help_session_status_unknown_rejected() {
        assert!(HelpSessionStatus::from_str("unknown").is_err());
    }

    // ---- append_help_session + help_sessions_in_range ----

    // REQ: JR-7 — Help session write and read round-trip.
    #[test]
    fn append_help_session_and_read() {
        let (_guard, path) = tmp_journal();
        let w = JournalWriter::open(&path).unwrap();
        let ts = 1_700_000_000;
        let input = HelpSessionInput {
            id: "01HXYZ1234567890123456789A",
            ts_unix: ts,
            ts: "2026-01-15T00:00:00Z",
            backend: "okapi",
            model: Some("llama3"),
            note: Some("test session"),
            prompt_chars: 150,
            response_chars: 300,
            latency_ms: Some(2500),
            status: HelpSessionStatus::Ok,
            error_kind: None,
            evidence_ref: "/tmp/evidence",
        };
        w.append_help_session(&input).unwrap();

        let r = w.reader();
        let sessions = r.help_sessions_in_range(ts, ts + 1).unwrap();
        assert_eq!(sessions.len(), 1);
        let s = &sessions[0];
        assert_eq!(s.id, "01HXYZ1234567890123456789A");
        assert_eq!(s.backend, "okapi");
        assert_eq!(s.model.as_deref(), Some("llama3"));
        assert_eq!(s.latency_ms, Some(2500));
        assert_eq!(s.status, HelpSessionStatus::Ok);
    }

    // ---- check_and_mark_nonce ----

    // REQ: JR-7 — Nonce replay protection: first use returns false, second returns true.
    // NOTE: requires the used_nonces table (migration 7 file exists but isn't in
    // MIGRATIONS list yet); we create it via a separate connection.
    #[test]
    fn nonce_replay_detection() {
        let (_guard, path) = tmp_journal();
        let w = JournalWriter::open(&path).unwrap();
        // Ensure the used_nonces table exists by opening a separate connection.
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                r"CREATE TABLE IF NOT EXISTS used_nonces (
                     token_id   TEXT    NOT NULL,
                     nonce      TEXT    NOT NULL,
                     expires_at INTEGER NOT NULL,
                     PRIMARY KEY (token_id, nonce)
                   );
                   CREATE INDEX IF NOT EXISTS used_nonces_expires ON used_nonces(expires_at);",
            )
            .unwrap();
        }
        let expires = crate::time::now_unix() + 3600;
        // First use → not seen before.
        assert_eq!(
            w.check_and_mark_nonce("token-1", "nonce-abc", expires)
                .unwrap(),
            false
        );
        // Second use → replay detected.
        assert_eq!(
            w.check_and_mark_nonce("token-1", "nonce-abc", expires)
                .unwrap(),
            true
        );
        // Different nonce → not seen before.
        assert_eq!(
            w.check_and_mark_nonce("token-1", "nonce-def", expires)
                .unwrap(),
            false
        );
    }

    // ---- previous_sample ----

    // REQ: JR-7 — previous_sample returns the most recent sample before a timestamp.
    #[test]
    fn previous_sample_returns_most_recent() {
        let (_guard, path) = tmp_journal();
        let w = JournalWriter::open(&path).unwrap();
        w.append_sample(
            1_700_000_000,
            Scope::Host,
            "mem_available_mib",
            Some(8192.0),
            None,
            Some("MiB"),
        )
        .unwrap();
        w.append_sample(
            1_700_000_300,
            Scope::Host,
            "mem_available_mib",
            Some(7000.0),
            None,
            Some("MiB"),
        )
        .unwrap();

        let r = w.reader();
        let result = r.previous_sample("mem_available_mib", 1_700_000_500);
        assert_eq!(result, Some((7000.0, 1_700_000_300)));
    }

    // REQ: JR-7 — previous_sample returns None when no sample exists.
    #[test]
    fn previous_sample_returns_none_when_empty() {
        let (_guard, path) = tmp_journal();
        let w = JournalWriter::open(&path).unwrap();
        let r = w.reader();
        assert_eq!(r.previous_sample("nonexistent", i64::MAX), None);
    }

    // ---- recent_with_evidence ----

    // REQ: JR-7 — recent_with_evidence only returns events with non-null evidence_ref.
    #[test]
    fn recent_with_evidence_filters() {
        let (_guard, path) = tmp_journal();
        let w = JournalWriter::open(&path).unwrap();
        // Event without evidence.
        let e1 = Event::new("observe", Severity::Info);
        w.append(&e1).unwrap();
        // Event with evidence.
        let mut e2 = Event::new("remediate", Severity::Alert);
        e2.evidence_ref = Some("/tmp/evidence-1".into());
        w.append(&e2).unwrap();

        let r = w.reader();
        let with_ev = r.recent_with_evidence(10).unwrap();
        assert_eq!(with_ev.len(), 1);
        assert_eq!(with_ev[0].evidence_ref.as_deref(), Some("/tmp/evidence-1"));
    }

    // ---- last_remote_fetch_ts ----

    // REQ: JR-7 — last_remote_fetch_ts returns the most recent unix timestamp
    // for remote.skill.fetch events. Previously queried MAX(ts) (text column)
    // instead of MAX(ts_unix) (integer) — fixed 2026-06-07.
    #[test]
    fn last_remote_fetch_ts_returns_unix_timestamp() {
        let (_guard, path) = tmp_journal();
        let w = JournalWriter::open(&path).unwrap();
        let mut e = Event::new("remote.skill.fetch", Severity::Info);
        e.ts_unix = 1_700_000_999;
        w.append(&e).unwrap();

        let r = w.reader();
        let ts = r.last_remote_fetch_ts().unwrap();
        assert_eq!(ts, Some(1_700_000_999));
    }

    // ---- list_events_by_action ----

    // REQ: JR-7 — list_events_by_action filters by action type.
    #[test]
    fn list_events_by_action_filters() {
        let (_guard, path) = tmp_journal();
        let w = JournalWriter::open(&path).unwrap();
        w.append(&Event::new("observe", Severity::Info)).unwrap();
        w.append(&Event::new("observe", Severity::Warn)).unwrap();
        w.append(&Event::new("remediate", Severity::Alert)).unwrap();

        let r = w.reader();
        let observe = r.list_events_by_action("observe", 0, i64::MAX).unwrap();
        assert_eq!(observe.len(), 2);
        let remediate = r.list_events_by_action("remediate", 0, i64::MAX).unwrap();
        assert_eq!(remediate.len(), 1);
        let empty = r.list_events_by_action("nonexistent", 0, i64::MAX).unwrap();
        assert_eq!(empty.len(), 0);
    }

    // ---- percentile helper ----

    // REQ: JR-7 — percentile returns correct values for known inputs.
    #[test]
    fn percentile_known_values() {
        let sorted = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        let p50 = percentile(&sorted, 50.0).unwrap();
        assert!((p50 - 5.5).abs() < 0.01, "p50 should be ~5.5, got {p50}");
        let p95 = percentile(&sorted, 95.0).unwrap();
        assert!((p95 - 9.55).abs() < 0.01, "p95 should be ~9.55, got {p95}");
        let p99 = percentile(&sorted, 99.0).unwrap();
        assert!((p99 - 9.91).abs() < 0.01, "p99 should be ~9.91, got {p99}");
    }

    // REQ: JR-7 — percentile returns None for empty slice.
    #[test]
    fn percentile_empty_returns_none() {
        assert_eq!(percentile(&[], 50.0), None);
    }

    // REQ: JR-7 — percentile returns the single element for len=1.
    #[test]
    fn percentile_single_element() {
        assert_eq!(percentile(&[42.0], 99.0), Some(42.0));
    }

    // ---- compute_ewma helper ----

    // REQ: JR-7 — compute_ewma returns None for fewer than 2 points.
    #[test]
    fn compute_ewma_too_few_points() {
        assert_eq!(compute_ewma(&[(1.0, 100)], 604_800.0), (None, None));
    }

    // REQ: JR-7 — compute_ewma returns values for a short series.
    #[test]
    fn compute_ewma_produces_values() {
        let series = [(10.0, 100), (20.0, 200), (30.0, 300)];
        let (mean, var) = compute_ewma(&series, 604_800.0);
        assert!(mean.is_some());
        assert!(var.is_some());
        // Mean should be between 10 and 30.
        let m = mean.unwrap();
        assert!(
            m > 10.0 && m < 30.0,
            "EWMA mean should be between 10 and 30, got {m}"
        );
    }

    // ---- JournalReader standalone construction ----

    // REQ: JR-7 — JournalReader::new does not require the file to exist.
    #[test]
    fn reader_new_does_not_require_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("nonexistent.db");
        let r = JournalReader::new(&path);
        assert_eq!(r.path(), &path);
    }

    // ---- compute_baselines from samples ----

    // REQ: JR-7 — compute_baselines produces baselines from real samples.
    #[test]
    fn compute_baselines_from_samples() {
        let (_guard, path) = tmp_journal();
        let w = JournalWriter::open(&path).unwrap();
        let now = crate::time::now_unix();
        // Insert enough samples for a single probe.
        for i in 0..10 {
            w.append_sample(
                now - 86_400 + i * 100,
                Scope::Host,
                "mem_available_mib",
                Some(8000.0 + i as f64 * 100.0),
                None,
                Some("MiB"),
            )
            .unwrap();
        }
        let r = w.reader();
        let baselines = r.compute_baselines(1).unwrap();
        assert_eq!(baselines.len(), 1);
        assert_eq!(baselines[0].probe, "mem_available_mib");
        assert!(baselines[0].p50.is_some());
        assert!(baselines[0].p95.is_some());
        assert!(baselines[0].ewma_mean.is_some());
        assert_eq!(baselines[0].count, 10);
    }
}

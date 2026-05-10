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

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, Ordering};

use rusqlite::{Connection, OpenFlags, params};
use tracing::{debug, info};

use crate::error::{CoreError, Result};
use crate::event::{Event, Scope, Severity};

/// Write-capable journal handle. Cheap to construct; holds an
/// open SQLite connection in WAL mode.
pub struct JournalWriter {
    conn: Connection,
    path: PathBuf,
    /// Atomically updated on every write (append / append_sample).
    /// Records the unix timestamp of the most recent write.
    last_write_unix_s: AtomicI64,
}

/// Read-only journal handle. Constructs fresh connections as needed.
#[derive(Debug, Clone)]
pub struct JournalReader {
    path: PathBuf,
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

        Ok(Self {
            conn,
            path: path.to_path_buf(),
            last_write_unix_s: AtomicI64::new(crate::time::now_unix()),
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
        let severity = match event.severity {
            Severity::Info => "info",
            Severity::Warn => "warn",
            Severity::Alert => "alert",
            Severity::Crit => "crit",
        };
        let scope = match event.scope {
            Scope::Host => "host",
            Scope::Self_ => "self",
        };

        self.conn.execute(
            r"INSERT INTO events (
                id, ts_unix, ts, schema, scope, tier, module, run_id,
                severity, action, dry_run, summary, evidence_ref,
                duration_ms, payload
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
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
            ],
        )?;
        debug!(id = %event.id, action = %event.action, "event appended");
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
        let scope_s = match scope {
            Scope::Host => "host",
            Scope::Self_ => "self",
        };
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

    /// Append a `help_sessions` row produced by the Doctor.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Sqlite`] on DB errors.
    #[allow(clippy::too_many_arguments)]
    pub fn append_help_session_row(
        &self,
        id: &str,
        ts_unix: i64,
        ts: &str,
        backend: &str,
        model: Option<&str>,
        note: Option<&str>,
        prompt_chars: i64,
        response_chars: i64,
        latency_ms: Option<i64>,
        status: &str,
        error_kind: Option<&str>,
        evidence_ref: &str,
    ) -> Result<()> {
        self.conn.execute(
            r"INSERT INTO help_sessions (
                id, ts_unix, ts, backend, model, note,
                prompt_chars, response_chars, latency_ms,
                status, error_kind, evidence_ref
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                id,
                ts_unix,
                ts,
                backend,
                model,
                note,
                prompt_chars,
                response_chars,
                latency_ms,
                status,
                error_kind,
                evidence_ref
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

    /// Unix timestamp of the most recent write (append / append_sample / append_help_session_row).
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
                let scope_s = match s {
                    Scope::Host => "host",
                    Scope::Self_ => "self",
                };
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
                    severity: match severity.as_str() {
                        "info" => Severity::Info,
                        "warn" => Severity::Warn,
                        "alert" => Severity::Alert,
                        "crit" => Severity::Crit,
                        _ => Severity::Info,
                    },
                    scope: if scope == "self" {
                        Scope::Self_
                    } else {
                        Scope::Host
                    },
                    tier: r.get(4)?,
                    module: r.get(5)?,
                    action: r.get(6)?,
                    summary: r.get(7)?,
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
    use crate::event::Severity;

    fn tmp_path() -> (tempfile::TempDir, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("journal.db");
        (tmp, p)
    }

    #[test]
    fn open_runs_migrations() {
        let (_g, p) = tmp_path();
        let w = JournalWriter::open(&p).unwrap();
        // Tables exist.
        let n: i64 = w
            .conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type='table' AND name IN ('events','samples','baselines','confirmations')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 4);
    }

    #[test]
    fn reopen_is_idempotent() {
        let (_g, p) = tmp_path();
        {
            let _ = JournalWriter::open(&p).unwrap();
        }
        let _ = JournalWriter::open(&p).unwrap();
    }

    #[test]
    fn append_and_read_round_trip() {
        let (_g, p) = tmp_path();
        let w = JournalWriter::open(&p).unwrap();
        let mut e = Event::new("observe", Severity::Info);
        e.module = Some("test".into());
        e.summary = Some("hello".into());
        w.append(&e).unwrap();

        let r = w.reader();
        let rows = r.recent(5).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].summary.as_deref(), Some("hello"));
    }

    #[test]
    fn severity_counts_buckets_correctly() {
        let (_g, p) = tmp_path();
        let w = JournalWriter::open(&p).unwrap();
        for sev in [
            Severity::Info,
            Severity::Info,
            Severity::Warn,
            Severity::Crit,
        ] {
            w.append(&Event::new("x", sev)).unwrap();
        }
        let c = w.reader().severity_counts(0, i64::MAX).unwrap();
        assert_eq!(
            c,
            SeverityCounts {
                info: 2,
                warn: 1,
                alert: 0,
                crit: 1
            }
        );
    }

    #[test]
    fn samples_insert_or_replace_is_idempotent() {
        let (_g, p) = tmp_path();
        let w = JournalWriter::open(&p).unwrap();
        w.append_sample(100, Scope::Host, "cpu_temp_c", Some(42.0), None, Some("C"))
            .unwrap();
        w.append_sample(100, Scope::Host, "cpu_temp_c", Some(43.0), None, Some("C"))
            .unwrap();
        let conn = w.reader().open_ro().unwrap();
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM samples WHERE ts=100", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(n, 1);
    }
}

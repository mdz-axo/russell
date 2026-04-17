// SPDX-License-Identifier: MIT OR Apache-2.0
//! Forward-only migration runner.
//!
//! Migrations live as individual `NNNN_<slug>.sql` files under
//! `migrations/` and are embedded into the binary via
//! [`include_str!`]. Adding a migration means:
//!
//! 1. Create the next zero-padded file.
//! 2. Append an entry to [`MIGRATIONS`].
//! 3. Add a test that runs all migrations and snapshots
//!    `PRAGMA table_info(...)` per touched table
//!    ([`CONTRIBUTING.md` §7](../../../CONTRIBUTING.md)).
//!
//! Never edit a merged migration.

use rusqlite::{Connection, params};
use tracing::info;

use crate::error::{CoreError, Result};

/// A single migration step.
pub struct Migration {
    /// Monotonic version. Matches the filename prefix.
    pub version: u32,
    /// Short kebab-case slug, matches filename suffix.
    pub slug: &'static str,
    /// The SQL. Multiple statements allowed.
    pub sql: &'static str,
}

/// Canonical list of migrations. Order matters.
pub const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    slug: "init",
    sql: include_str!("migrations/0001_init.sql"),
}];

/// Apply any migrations newer than the DB's current version.
///
/// # Errors
///
/// Returns [`CoreError::Migration`] if any step fails;
/// [`CoreError::Sqlite`] on lower-level DB errors.
pub fn run(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r"CREATE TABLE IF NOT EXISTS schema_migrations (
              version INTEGER PRIMARY KEY,
              slug    TEXT NOT NULL,
              applied_ts INTEGER NOT NULL
          );",
    )?;

    let applied: u32 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    for m in MIGRATIONS {
        if m.version <= applied {
            continue;
        }
        info!(version = m.version, slug = %m.slug, "applying migration");
        apply_one(conn, m)?;
    }

    Ok(())
}

fn apply_one(conn: &Connection, m: &Migration) -> Result<()> {
    let tx_result: rusqlite::Result<()> = (|| {
        conn.execute_batch("BEGIN IMMEDIATE")?;
        conn.execute_batch(m.sql)?;
        conn.execute(
            "INSERT INTO schema_migrations (version, slug, applied_ts) VALUES (?1, ?2, ?3)",
            params![m.version, m.slug, crate::time::now_unix()],
        )?;
        conn.execute_batch("COMMIT")?;
        Ok(())
    })();
    match tx_result {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(CoreError::Migration {
                version: m.version,
                reason: e.to_string(),
            })
        }
    }
}

/// Version of the most-recently-applied migration in the DB
/// reachable through `conn`.
///
/// # Errors
///
/// Returns [`CoreError::Sqlite`] on DB errors.
pub fn current_version(conn: &Connection) -> Result<u32> {
    let v: u32 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh() -> Connection {
        Connection::open_in_memory().unwrap()
    }

    #[test]
    fn runs_once_then_noop() {
        let c = fresh();
        run(&c).unwrap();
        assert_eq!(current_version(&c).unwrap(), 1);
        // Second run must not re-apply.
        run(&c).unwrap();
        assert_eq!(current_version(&c).unwrap(), 1);
    }

    #[test]
    fn init_creates_all_core_tables() {
        let c = fresh();
        run(&c).unwrap();
        for t in ["samples", "events", "baselines", "confirmations"] {
            let n: i64 = c
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    params![t],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(n, 1, "table {t} missing");
        }
    }

    #[test]
    fn migrations_are_monotonic() {
        let mut last = 0u32;
        for m in MIGRATIONS {
            assert!(m.version > last, "non-monotonic at {}", m.version);
            last = m.version;
        }
    }
}

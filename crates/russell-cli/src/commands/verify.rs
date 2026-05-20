// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell verify-journal` — check event integrity chain (T6).
//!
//! Walks the hash chain from the first non-NULL hash event to the
//! latest, recomputing SHA-256 links and reporting any breaks.
//! This is a read-only diagnostic — it never modifies the journal.

use anyhow::{Context, Result};
use russell_core::hash_chain::{self, ChainVerdict};
use russell_core::journal::JournalWriter;
use russell_core::paths::Paths;

pub fn run(paths: &Paths) -> Result<()> {
    let journal = JournalWriter::open(&paths.journal())
        .with_context(|| format!("opening journal {}", paths.journal().display()))?;
    let reader = journal.reader();

    // Read all events with non-NULL hashes in chronological order.
    let conn = reader
        .open_ro_conn()
        .context("opening read-only connection")?;

    let mut stmt = conn
        .prepare(
            "SELECT prev_hash, payload, hash FROM events \
             WHERE hash IS NOT NULL \
             ORDER BY ts_unix ASC, id ASC",
        )
        .context("preparing query")?;

    let links: Vec<(String, String, String)> = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .context("executing query")?
        .filter_map(|r| r.ok())
        .collect();

    match hash_chain::verify_chain(&links) {
        ChainVerdict::Intact { count } => {
            println!("journal integrity: OK ({count} events verified)");
        }
        ChainVerdict::Broken {
            position,
            expected,
            found,
        } => {
            println!("journal integrity: BROKEN at position {position}");
            println!("  expected hash: {expected}");
            println!("  found hash:    {found}");
            println!("  → One or more events may have been tampered with.");
            std::process::exit(1);
        }
        ChainVerdict::Empty => {
            println!("journal integrity: no hash-chained events yet (pre-T6 journal)");
        }
    }

    Ok(())
}

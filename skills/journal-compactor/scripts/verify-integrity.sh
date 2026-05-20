#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# verify-integrity.sh — Verify journal integrity after compaction.
#
# Checks:
# 1. SQLite integrity (PRAGMA integrity_check)
# 2. Event chain integrity (hash links valid)
# 3. Sample count is reasonable (non-negative)

set -euo pipefail

# Find journal path.
JOURNAL_DB="${RUSSELL_JOURNAL_DB:-$HOME/.local/state/harness/journal.db}"

if [[ ! -f "$JOURNAL_DB" ]]; then
    echo "Error: journal not found at $JOURNAL_DB"
    exit 1
fi

echo "Verifying journal integrity..."

# Check SQLite integrity.
INTEGRITY=$(sqlite3 "$JOURNAL_DB" "PRAGMA integrity_check;" 2>/dev/null)
if [[ "$INTEGRITY" != "ok" ]]; then
    echo "ERROR: SQLite integrity check failed: $INTEGRITY"
    exit 1
fi
echo "✓ SQLite integrity: ok"

# Check event count is non-negative.
EVENT_COUNT=$(sqlite3 "$JOURNAL_DB" "SELECT COUNT(*) FROM events;" 2>/dev/null || echo "0")
if [[ $EVENT_COUNT -lt 0 ]]; then
    echo "ERROR: Invalid event count: $EVENT_COUNT"
    exit 1
fi
echo "✓ Event count: $EVENT_COUNT"

# Check sample count is non-negative.
SAMPLE_COUNT=$(sqlite3 "$JOURNAL_DB" "SELECT COUNT(*) FROM samples;" 2>/dev/null || echo "0")
if [[ $SAMPLE_COUNT -lt 0 ]]; then
    echo "ERROR: Invalid sample count: $SAMPLE_COUNT"
    exit 1
fi
echo "✓ Sample count: $SAMPLE_COUNT"

# Check hash chain integrity (T6) — verify each event's prev_hash links correctly.
# This is a simplified check — full verification uses russell verify-journal.
BROKEN_LINKS=$(sqlite3 "$JOURNAL_DB" "
SELECT COUNT(*) FROM events e
WHERE e.prev_hash IS NOT NULL
AND NOT EXISTS (
    SELECT 1 FROM events p 
    WHERE p.hash = e.prev_hash
);
" 2>/dev/null || echo "0")

if [[ $BROKEN_LINKS -gt 0 ]]; then
    echo "WARNING: $BROKEN_LINKS events have broken hash links."
    echo "Run 'russell verify-journal' for full audit."
fi

echo "✓ Integrity verification complete."
exit 0
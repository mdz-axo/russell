#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# vacuum-journal.sh — Compact SQLite journal via VACUUM command.
#
# VACUUM rebuilds the database file, reclaiming unused space.
# This is safe but may take time for large journals.
#
# Pre-condition: Journal file exists
# Post-condition: Journal file is compacted
# Rollback: Not possible (none_needed) — VACUUM is idempotent

set -euo pipefail

# Find journal path.
JOURNAL_DB="${RUSSELL_JOURNAL_DB:-$HOME/.local/state/harness/journal.db}"

if [[ ! -f "$JOURNAL_DB" ]]; then
    echo "Error: journal not found at $JOURNAL_DB"
    exit 1
fi

# Get size before.
SIZE_BEFORE=$(stat -c%s "$JOURNAL_DB" 2>/dev/null || echo "0")

echo "VACUUMing journal: $JOURNAL_DB"
echo "Size before: $SIZE_BEFORE bytes"

# Run VACUUM.
sqlite3 "$JOURNAL_DB" "VACUUM;" 2>&1

# Get size after.
SIZE_AFTER=$(stat -c%s "$JOURNAL_DB" 2>/dev/null || echo "0")

echo "Size after: $SIZE_AFTER bytes"

# Calculate savings.
if [[ $SIZE_BEFORE -gt 0 ]]; then
    SAVINGS=$((SIZE_BEFORE - SIZE_AFTER))
    PERCENT=$((SAVINGS * 100 / SIZE_BEFORE))
    echo "Space reclaimed: $SAVINGS bytes ($PERCENT%)"
fi

echo "VACUUM complete"
exit 0
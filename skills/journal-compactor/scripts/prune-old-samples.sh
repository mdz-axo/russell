#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# prune-old-samples.sh — Delete samples older than retention threshold.
#
# WARNING: This operation causes permanent DATA LOSS.
# Samples older than the retention threshold are permanently deleted.
# Rollback is NOT possible — this is why require_human_for is set.
#
# Usage: prune-old-samples.sh [--retention-days N]
# Default retention: 365 days

set -euo pipefail

# Parse arguments.
RETENTION_DAYS=365
while [[ $# -gt 0 ]]; do
    case "$1" in
        --retention-days)
            RETENTION_DAYS="$2"
            shift 2
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Find journal path.
JOURNAL_DB="${RUSSELL_JOURNAL_DB:-$HOME/.local/state/harness/journal.db}"

if [[ ! -f "$JOURNAL_DB" ]]; then
    echo "Error: journal not found at $JOURNAL_DB"
    exit 1
fi

# Count samples to be deleted.
TO_DELETE=$(sqlite3 "$JOURNAL_DB" "
SELECT COUNT(*) FROM samples 
WHERE scope = 'host' 
AND ts < (strftime('%s', 'now') - ($RETENTION_DAYS * 86400));
" 2>/dev/null || echo "0")

if [[ $TO_DELETE -eq 0 ]]; then
    echo "No samples older than $RETENTION_DAYS days found."
    exit 0
fi

echo "WARNING: About to delete $TO_DELETE samples older than $RETENTION_DAYS days."
echo "This operation is PERMANENT and CANNOT be undone."
echo ""

# Delete old samples.
sqlite3 "$JOURNAL_DB" "
DELETE FROM samples 
WHERE scope = 'host' 
AND ts < (strftime('%s', 'now') - ($RETENTION_DAYS * 86400));
" 2>&1

echo "Deleted $TO_DELETE samples."

# Run VACUUM to reclaim space.
echo "Running VACUUM to reclaim space..."
sqlite3 "$JOURNAL_DB" "VACUUM;" 2>&1

echo "Prune complete."
exit 0
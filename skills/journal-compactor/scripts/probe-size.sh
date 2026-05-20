#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# probe-size.sh — Estimate journal size and sample age distribution.
#
# Outputs JSON with:
# - total_size_bytes: Total journal file size
# - sample_count: Total number of samples
# - oldest_sample_days: Age of oldest sample in days
# - newest_sample_days: Age of newest sample in days
# - samples_over_365_days: Count of samples older than 365 days

set -euo pipefail

# Find journal path — use RUSSELL_ROOT if set, otherwise XDG default.
JOURNAL_DB="${RUSSELL_JOURNAL_DB:-$HOME/.local/state/harness/journal.db}"

if [[ ! -f "$JOURNAL_DB" ]]; then
    echo '{"error": "journal not found", "path": "'"$JOURNAL_DB"'"}'
    exit 0
fi

# Get file size.
TOTAL_SIZE=$(stat -c%s "$JOURNAL_DB" 2>/dev/null || echo "0")

# Query sample statistics.
read -r SAMPLE_COUNT OLDEST_DAYS NEWEST_DAYS OVER_365 <<< $(sqlite3 "$JOURNAL_DB" "
SELECT 
    COUNT(*) as sample_count,
    CAST((strftime('%s', 'now') - MIN(ts)) / 86400.0 AS INTEGER) as oldest_days,
    CAST((strftime('%s', 'now') - MAX(ts)) / 86400.0 AS INTEGER) as newest_days,
    SUM(CASE WHEN (strftime('%s', 'now') - ts) > (365 * 86400) THEN 1 ELSE 0 END) as over_365
FROM samples
WHERE scope = 'host';
" 2>/dev/null | tr '|' ' ' || echo "0 0 0 0")

# Output JSON.
cat <<EOF
{
    "total_size_bytes": $TOTAL_SIZE,
    "sample_count": ${SAMPLE_COUNT:-0},
    "oldest_sample_days": ${OLDEST_DAYS:-0},
    "newest_sample_days": ${NEWEST_DAYS:-0},
    "samples_over_365_days": ${OVER_365:-0},
    "journal_path": "$JOURNAL_DB"
}
EOF

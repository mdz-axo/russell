#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# workspace: write-file intervention
# Writes content from stdin to a file. IDRS-compliant.
#
# I — Idempotent: if content matches, no-op.
# D — Dry-run: RUSSELL_DRY_RUN=1 or --dry-run shows what would happen.
# R — Rollback: backup original to RUSSELL_BACKUP_DIR before write.
# S — Structured log: JSON event to stdout.
#
# Usage: write-file.sh <path> [--dry-run]
# Content is read from stdin.

set -euo pipefail

FILE_PATH="${1:-}"
DRY_RUN="${RUSSELL_DRY_RUN:-0}"
BACKUP_DIR="${RUSSELL_BACKUP_DIR:-$HOME/.local/share/harness/backups}"

# Parse --dry-run flag
shift || true
for arg in "$@"; do
    if [[ "$arg" == "--dry-run" ]]; then
        DRY_RUN=1
    fi
done

if [[ -z "$FILE_PATH" ]]; then
    echo '{"action":"write-file","error":"missing path argument"}' >&2
    exit 1
fi

# Security: resolve and check path stays under HOME or /tmp
FILE_PATH=$(realpath -m "$FILE_PATH" 2>/dev/null || echo "$FILE_PATH")
HOME_DIR=$(realpath -m "$HOME" 2>/dev/null || echo "$HOME")

if [[ ! "$FILE_PATH" == "$HOME_DIR"* ]] && [[ ! "$FILE_PATH" == /tmp/* ]]; then
    echo '{"action":"write-file","error":"path must be under $HOME or /tmp"}' >&2
    exit 1
fi

# Read content from stdin
CONTENT=$(cat)

# Idempotency check: if file exists and content matches, no-op
if [[ -f "$FILE_PATH" ]]; then
    EXISTING=$(cat "$FILE_PATH" 2>/dev/null || echo "")
    if [[ "$CONTENT" == "$EXISTING" ]]; then
        echo '{"action":"write-file","path":"'"$FILE_PATH"'","status":"no_change","dry_run":false}'
        exit 0
    fi
fi

# Dry-run: report but don't act
if [[ "$DRY_RUN" == "1" ]]; then
    BYTES=$(echo -n "$CONTENT" | wc -c)
    echo '{"action":"write-file","path":"'"$FILE_PATH"'","status":"would_write","dry_run":true,"bytes":'$BYTES'}'
    exit 0
fi

# Backup original if it exists
BACKUP_PATH=""
if [[ -f "$FILE_PATH" ]]; then
    mkdir -p "$BACKUP_DIR"
    TIMESTAMP=$(date +%Y%m%dT%H%M%S)
    BASENAME=$(basename "$FILE_PATH")
    BACKUP_PATH="${BACKUP_DIR}/${TIMESTAMP}-${BASENAME}.bak"
    cp -p "$FILE_PATH" "$BACKUP_PATH"
fi

# Write content (create parent dirs if needed)
PARENT_DIR=$(dirname "$FILE_PATH")
mkdir -p "$PARENT_DIR"

echo -n "$CONTENT" > "$FILE_PATH"

# Set permissions: readable by owner and group
chmod 644 "$FILE_PATH" 2>/dev/null || true

BYTES=$(stat -c%s "$FILE_PATH" 2>/dev/null || echo "?")
TIMESTAMP=$(date -Iseconds)

# Structured log
echo '{"action":"write-file","path":"'"$FILE_PATH"'","status":"written","dry_run":false,"bytes_written":'$BYTES',"backup":"'"$BACKUP_PATH"'","timestamp":"'"$TIMESTAMP"'"}'

exit 0

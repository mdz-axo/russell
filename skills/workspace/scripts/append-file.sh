#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# workspace: append-file intervention
# Appends content from stdin to a file. NOT idempotent (appending twice
# doubles content). IDRS-compliant.
#
# D — Dry-run: RUSSELL_DRY_RUN=1 or --dry-run.
# R — Rollback: backup original before append.
# S — Structured log: JSON event to stdout.
#
# Usage: append-file.sh <path> [--dry-run]
# Content is read from stdin.

set -euo pipefail

FILE_PATH="${1:-}"
DRY_RUN="${RUSSELL_DRY_RUN:-0}"
BACKUP_DIR="${RUSSELL_BACKUP_DIR:-$HOME/.local/share/harness/backups}"

shift || true
for arg in "$@"; do
    if [[ "$arg" == "--dry-run" ]]; then
        DRY_RUN=1
    fi
done

if [[ -z "$FILE_PATH" ]]; then
    echo '{"action":"append-file","error":"missing path argument"}' >&2
    exit 1
fi

FILE_PATH=$(realpath -m "$FILE_PATH" 2>/dev/null || echo "$FILE_PATH")
HOME_DIR=$(realpath -m "$HOME" 2>/dev/null || echo "$HOME")

if [[ ! "$FILE_PATH" == "$HOME_DIR"* ]] && [[ ! "$FILE_PATH" == /tmp/* ]]; then
    echo '{"action":"append-file","error":"path must be under $HOME or /tmp"}' >&2
    exit 1
fi

CONTENT=$(cat)

if [[ "$DRY_RUN" == "1" ]]; then
    BYTES=$(echo -n "$CONTENT" | wc -c)
    echo '{"action":"append-file","path":"'"$FILE_PATH"'","status":"would_append","dry_run":true,"bytes":'$BYTES'}'
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

# Append content (create file if it doesn't exist)
PARENT_DIR=$(dirname "$FILE_PATH")
mkdir -p "$PARENT_DIR"

echo -n "$CONTENT" >> "$FILE_PATH"

BYTES=$(echo -n "$CONTENT" | wc -c)
TIMESTAMP=$(date -Iseconds)

echo '{"action":"append-file","path":"'"$FILE_PATH"'","status":"appended","dry_run":false,"bytes_appended":'$BYTES',"backup":"'"$BACKUP_PATH"'","timestamp":"'"$TIMESTAMP"'"}'

exit 0

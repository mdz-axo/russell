#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# workspace: move-path intervention
# Moves or renames a file/directory. NOT idempotent. IDRS-compliant.
#
# D — Dry-run: RUSSELL_DRY_RUN=1 or --dry-run.
# R — Rollback: move back if destination didn't exist before.
# S — Structured log: JSON event to stdout.
#
# Usage: move-path.sh <source> <destination> [--dry-run]

set -euo pipefail

SOURCE_PATH="${1:-}"
DEST_PATH="${2:-}"
DRY_RUN="${RUSSELL_DRY_RUN:-0}"
BACKUP_DIR="${RUSSELL_BACKUP_DIR:-$HOME/.local/share/harness/backups}"

# Parse dry-run from remaining args
shift 2 2>/dev/null || true
for arg in "$@"; do
    if [[ "$arg" == "--dry-run" ]]; then
        DRY_RUN=1
    fi
done

if [[ -z "$SOURCE_PATH" ]] || [[ -z "$DEST_PATH" ]]; then
    echo '{"action":"move-path","error":"missing source or destination argument"}' >&2
    exit 1
fi

SOURCE_PATH=$(realpath -m "$SOURCE_PATH" 2>/dev/null || echo "$SOURCE_PATH")
DEST_PATH=$(realpath -m "$DEST_PATH" 2>/dev/null || echo "$DEST_PATH")
HOME_DIR=$(realpath -m "$HOME" 2>/dev/null || echo "$HOME")

if [[ ! "$SOURCE_PATH" == "$HOME_DIR"* ]] && [[ ! "$SOURCE_PATH" == /tmp/* ]]; then
    echo '{"action":"move-path","error":"source must be under $HOME or /tmp"}' >&2
    exit 1
fi

if [[ ! "$DEST_PATH" == "$HOME_DIR"* ]] && [[ ! "$DEST_PATH" == /tmp/* ]]; then
    echo '{"action":"move-path","error":"destination must be under $HOME or /tmp"}' >&2
    exit 1
fi

if [[ ! -e "$SOURCE_PATH" ]]; then
    echo '{"action":"move-path","error":"source not found","path":"'"$SOURCE_PATH"'"}' >&2
    exit 1
fi

if [[ "$DRY_RUN" == "1" ]]; then
    echo '{"action":"move-path","source":"'"$SOURCE_PATH"'","destination":"'"$DEST_PATH"'","status":"would_move","dry_run":true}'
    exit 0
fi

# Backup destination if it already exists (prevents overwrite)
BACKUP_PATH=""
if [[ -e "$DEST_PATH" ]]; then
    mkdir -p "$BACKUP_DIR"
    TIMESTAMP=$(date +%Y%m%dT%H%M%S)
    BASENAME=$(basename "$DEST_PATH")
    BACKUP_PATH="${BACKUP_DIR}/${TIMESTAMP}-${BASENAME}.bak"
    mv "$DEST_PATH" "$BACKUP_PATH"
fi

# Create destination parent directory if needed
DEST_PARENT=$(dirname "$DEST_PATH")
mkdir -p "$DEST_PARENT"

# Move the file/directory
mv "$SOURCE_PATH" "$DEST_PATH"

TIMESTAMP=$(date -Iseconds)
echo '{"action":"move-path","source":"'"$SOURCE_PATH"'","destination":"'"$DEST_PATH"'","status":"moved","dry_run":false,"destination_backup":"'"$BACKUP_PATH"'","timestamp":"'"$TIMESTAMP"'"}'

exit 0

#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# workspace: delete-path intervention
# Moves a file or directory to backup (safe delete). IDRS-compliant.
# Medium risk — requires explicit human confirmation.
#
# I — Idempotent: deleting non-existent path is a no-op.
# D — Dry-run: RUSSELL_DRY_RUN=1 or --dry-run.
# R — Rollback: original moved to backup, can be restored.
# S — Structured log: JSON event to stdout.
#
# Usage: delete-path.sh <path> [--dry-run]

set -euo pipefail

TARGET_PATH="${1:-}"
DRY_RUN="${RUSSELL_DRY_RUN:-0}"
BACKUP_DIR="${RUSSELL_BACKUP_DIR:-$HOME/.local/share/harness/backups}"

shift || true
for arg in "$@"; do
    if [[ "$arg" == "--dry-run" ]]; then
        DRY_RUN=1
    fi
done

if [[ -z "$TARGET_PATH" ]]; then
    echo '{"action":"delete-path","error":"missing path argument"}' >&2
    exit 1
fi

TARGET_PATH=$(realpath -m "$TARGET_PATH" 2>/dev/null || echo "$TARGET_PATH")
HOME_DIR=$(realpath -m "$HOME" 2>/dev/null || echo "$HOME")

if [[ ! "$TARGET_PATH" == "$HOME_DIR"* ]] && [[ ! "$TARGET_PATH" == /tmp/* ]]; then
    echo '{"action":"delete-path","error":"path must be under $HOME or /tmp"}' >&2
    exit 1
fi

# Safety: refuse to delete the home directory itself
if [[ "$TARGET_PATH" == "$HOME_DIR" || "$TARGET_PATH" == "$HOME_DIR/" ]]; then
    echo '{"action":"delete-path","error":"refused: will not delete home directory"}' >&2
    exit 1
fi

# Safety: refuse to delete Russell's journal
if [[ "$TARGET_PATH" == *"/journal.db"* ]]; then
    echo '{"action":"delete-path","error":"refused: will not delete journal database"}' >&2
    exit 1
fi

# Idempotency: if already gone, no-op
if [[ ! -e "$TARGET_PATH" ]]; then
    echo '{"action":"delete-path","path":"'"$TARGET_PATH"'","status":"already_gone","dry_run":false}'
    exit 0
fi

if [[ "$DRY_RUN" == "1" ]]; then
    KIND=$([[ -d "$TARGET_PATH" ]] && echo "directory" || echo "file")
    echo '{"action":"delete-path","path":"'"$TARGET_PATH"'","kind":"'"$KIND"'","status":"would_delete","dry_run":true}'
    exit 0
fi

# Move to backup instead of actually deleting
mkdir -p "$BACKUP_DIR"
TIMESTAMP=$(date +%Y%m%dT%H%M%S)
BASENAME=$(basename "$TARGET_PATH")
BACKUP_PATH="${BACKUP_DIR}/${TIMESTAMP}-${BASENAME}.bak"

# Handle collision in backup dir
if [[ -e "$BACKUP_PATH" ]]; then
    BACKUP_PATH="${BACKUP_DIR}/${TIMESTAMP}-${BASENAME}-$$.bak"
fi

mv "$TARGET_PATH" "$BACKUP_PATH"

TIMESTAMP_ISO=$(date -Iseconds)
echo '{"action":"delete-path","path":"'"$TARGET_PATH"'","status":"moved_to_backup","dry_run":false,"backup":"'"$BACKUP_PATH"'","timestamp":"'"$TIMESTAMP_ISO"'"}'

exit 0

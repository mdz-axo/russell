#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# workspace: delete-if-created rollback intervention
# Deletes a directory that was just created (rollback for create-directory). IDRS-compliant.
#
# I — Idempotent: if directory doesn't exist, no-op.
# D — Dry-run: RUSSELL_DRY_RUN=1 or --dry-run shows what would happen.
# R — Rollback: this IS the rollback — removes the created directory.
# S — Structured log: JSON event to stdout.
#
# Usage: delete-if-created.sh <dir_path> [--dry-run]

set -euo pipefail

DIR_PATH="${1:-}"
DRY_RUN="${RUSSELL_DRY_RUN:-0}"
shift || true

for arg in "$@"; do
    if [[ "$arg" == "--dry-run" ]]; then
        DRY_RUN=1
    fi
done

if [[ -z "$DIR_PATH" ]]; then
    echo '{"action":"delete-if-created","error":"missing dir_path argument"}' >&2
    exit 1
fi

# Security: resolve and check path stays under HOME or /tmp
DIR_PATH=$(realpath -m "$DIR_PATH" 2>/dev/null || echo "$DIR_PATH")
HOME_DIR=$(realpath -m "$HOME" 2>/dev/null || echo "$HOME")

if [[ ! "$DIR_PATH" == "$HOME_DIR"* ]] && [[ ! "$DIR_PATH" == /tmp/* ]]; then
    echo '{"action":"delete-if-created","error":"path must be under $HOME or /tmp"}' >&2
    exit 1
fi

# Idempotency check: if directory doesn't exist, no-op
if [[ ! -d "$DIR_PATH" ]]; then
    echo '{"action":"delete-if-created","path":"'"$DIR_PATH"'","status":"no_change","dry_run":false}'
    exit 0
fi

# Safety: refuse to delete non-empty directories
ITEM_COUNT=$(find "$DIR_PATH" -mindepth 1 -maxdepth 1 2>/dev/null | wc -l)
if [[ "$ITEM_COUNT" -gt 0 ]]; then
    echo '{"action":"delete-if-created","error":"directory not empty, refusing to delete","path":"'"$DIR_PATH"'"}' >&2
    exit 1
fi

# Dry-run: report but don't act
if [[ "$DRY_RUN" == "1" ]]; then
    echo '{"action":"delete-if-created","path":"'"$DIR_PATH"'","status":"would_delete","dry_run":true}'
    exit 0
fi

# Delete the empty directory
rmdir "$DIR_PATH"

TIMESTAMP=$(date -Iseconds)
echo '{"action":"delete-if-created","path":"'"$DIR_PATH"'","status":"deleted","dry_run":false,"timestamp":"'"$TIMESTAMP"'"}'

exit 0

#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# workspace: restore-backup rollback intervention
# Restores a file from its .bak backup. IDRS-compliant.
#
# I — Idempotent: if file already matches backup, no-op.
# D — Dry-run: RUSSELL_DRY_RUN=1 or --dry-run shows what would happen.
# R — Rollback: this IS the rollback — restores original from backup.
# S — Structured log: JSON event to stdout.
#
# Usage: restore-backup.sh <backup_path> <original_path> [--dry-run]

set -euo pipefail

BACKUP_PATH="${1:-}"
ORIGINAL_PATH="${2:-}"
DRY_RUN="${RUSSELL_DRY_RUN:-0}"
shift 2 2>/dev/null || true

for arg in "$@"; do
    if [[ "$arg" == "--dry-run" ]]; then
        DRY_RUN=1
    fi
done

if [[ -z "$BACKUP_PATH" ]] || [[ -z "$ORIGINAL_PATH" ]]; then
    echo '{"action":"restore-backup","error":"missing backup_path or original_path argument"}' >&2
    exit 1
fi

if [[ ! -f "$BACKUP_PATH" ]]; then
    echo '{"action":"restore-backup","error":"backup file not found","backup":"'"$BACKUP_PATH"'"}' >&2
    exit 1
fi

# Security: resolve and check path stays under HOME or /tmp
BACKUP_PATH=$(realpath -m "$BACKUP_PATH" 2>/dev/null || echo "$BACKUP_PATH")
ORIGINAL_PATH=$(realpath -m "$ORIGINAL_PATH" 2>/dev/null || echo "$ORIGINAL_PATH")
HOME_DIR=$(realpath -m "$HOME" 2>/dev/null || echo "$HOME")

if [[ ! "$BACKUP_PATH" == "$HOME_DIR"* ]] && [[ ! "$BACKUP_PATH" == /tmp/* ]]; then
    echo '{"action":"restore-backup","error":"backup path must be under $HOME or /tmp"}' >&2
    exit 1
fi
if [[ ! "$ORIGINAL_PATH" == "$HOME_DIR"* ]] && [[ ! "$ORIGINAL_PATH" == /tmp/* ]]; then
    echo '{"action":"restore-backup","error":"original path must be under $HOME or /tmp"}' >&2
    exit 1
fi

# Idempotency check: if original matches backup, no-op
if [[ -f "$ORIGINAL_PATH" ]]; then
    if cmp -s "$BACKUP_PATH" "$ORIGINAL_PATH"; then
        echo '{"action":"restore-backup","path":"'"$ORIGINAL_PATH"'","status":"no_change","dry_run":false}'
        exit 0
    fi
fi

# Dry-run: report but don't act
if [[ "$DRY_RUN" == "1" ]]; then
    echo '{"action":"restore-backup","path":"'"$ORIGINAL_PATH"'","backup":"'"$BACKUP_PATH"'","status":"would_restore","dry_run":true}'
    exit 0
fi

# Restore
cp -p "$BACKUP_PATH" "$ORIGINAL_PATH"

TIMESTAMP=$(date -Iseconds)
echo '{"action":"restore-backup","path":"'"$ORIGINAL_PATH"'","backup":"'"$BACKUP_PATH"'","status":"restored","dry_run":false,"timestamp":"'"$TIMESTAMP"'"}'

exit 0

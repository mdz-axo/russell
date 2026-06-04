#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# workspace: patch-file intervention
# Applies a unified diff from stdin to a file. IDRS-compliant.
# Medium risk — requires explicit human confirmation.
#
# I — Idempotent: if patch already applied, no-op (patch --fuzz=0).
# D — Dry-run: RUSSELL_DRY_RUN=1 or --dry-run.
# R — Rollback: backup original before patch.
# S — Structured log: JSON event to stdout.
#
# Usage: patch-file.sh <path> [--dry-run]
# Diff content is read from stdin.

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
    echo '{"action":"patch-file","error":"missing path argument"}' >&2
    exit 1
fi

FILE_PATH=$(realpath -m "$FILE_PATH" 2>/dev/null || echo "$FILE_PATH")
HOME_DIR=$(realpath -m "$HOME" 2>/dev/null || echo "$HOME")

if [[ ! "$FILE_PATH" == "$HOME_DIR"* ]] && [[ ! "$FILE_PATH" == /tmp/* ]]; then
    echo '{"action":"patch-file","error":"path must be under $HOME or /tmp"}' >&2
    exit 1
fi

if [[ ! -f "$FILE_PATH" ]]; then
    echo '{"action":"patch-file","error":"file not found","path":"'"$FILE_PATH"'"}' >&2
    exit 1
fi

DIFF_CONTENT=$(cat)

if [[ -z "$DIFF_CONTENT" ]]; then
    echo '{"action":"patch-file","error":"empty diff on stdin"}' >&2
    exit 1
fi

# Dry-run: test if patch applies cleanly
DRY_RESULT=$(echo "$DIFF_CONTENT" | patch --dry-run --fuzz=0 -p1 -d "$(dirname "$FILE_PATH")" 2>&1) || true

if [[ "$DRY_RUN" == "1" ]]; then
    echo '{"action":"patch-file","path":"'"$FILE_PATH"'","status":"would_patch","dry_run":true,"test_result":"'"$(echo "$DRY_RESULT" | head -3 | tr '\n' ' ')"'"}'
    exit 0
fi

# Check if patch already applied (reverse dry-run succeeds = already applied)
if echo "$DIFF_CONTENT" | patch --dry-run --reverse --fuzz=0 -p1 -d "$(dirname "$FILE_PATH")" &>/dev/null; then
    echo '{"action":"patch-file","path":"'"$FILE_PATH"'","status":"already_applied","dry_run":false}'
    exit 0
fi

# Backup original
mkdir -p "$BACKUP_DIR"
TIMESTAMP=$(date +%Y%m%dT%H%M%S)
BASENAME=$(basename "$FILE_PATH")
BACKUP_PATH="${BACKUP_DIR}/${TIMESTAMP}-${BASENAME}.bak"
cp -p "$FILE_PATH" "$BACKUP_PATH"

# Apply the patch
echo "$DIFF_CONTENT" | patch --fuzz=0 -p1 -d "$(dirname "$FILE_PATH")" 2>&1 || {
    # Patch failed — report and exit with error
    echo '{"action":"patch-file","path":"'"$FILE_PATH"'","status":"patch_failed","dry_run":false,"backup":"'"$BACKUP_PATH"'"}' >&2
    exit 1
}

TIMESTAMP=$(date -Iseconds)
echo '{"action":"patch-file","path":"'"$FILE_PATH"'","status":"patched","dry_run":false,"backup":"'"$BACKUP_PATH"'","timestamp":"'"$TIMESTAMP"'"}'

exit 0

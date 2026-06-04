#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# workspace: move-back rollback intervention
# Moves a file back to its original location (rollback for move-path). IDRS-compliant.
#
# I — Idempotent: if file already at original location, no-op.
# D — Dry-run: RUSSELL_DRY_RUN=1 or --dry-run shows what would happen.
# R — Rollback: this IS the rollback — moves file back to original location.
# S — Structured log: JSON event to stdout.
#
# Usage: move-back.sh <current_path> <original_path> [--dry-run]

set -euo pipefail

CURRENT_PATH="${1:-}"
ORIGINAL_PATH="${2:-}"
DRY_RUN="${RUSSELL_DRY_RUN:-0}"
shift 2 2>/dev/null || true

for arg in "$@"; do
    if [[ "$arg" == "--dry-run" ]]; then
        DRY_RUN=1
    fi
done

if [[ -z "$CURRENT_PATH" ]] || [[ -z "$ORIGINAL_PATH" ]]; then
    echo '{"action":"move-back","error":"missing current_path or original_path argument"}' >&2
    exit 1
fi

# Security: resolve and check path stays under HOME or /tmp
CURRENT_PATH=$(realpath -m "$CURRENT_PATH" 2>/dev/null || echo "$CURRENT_PATH")
ORIGINAL_PATH=$(realpath -m "$ORIGINAL_PATH" 2>/dev/null || echo "$ORIGINAL_PATH")
HOME_DIR=$(realpath -m "$HOME" 2>/dev/null || echo "$HOME")

if [[ ! "$CURRENT_PATH" == "$HOME_DIR"* ]] && [[ ! "$CURRENT_PATH" == /tmp/* ]]; then
    echo '{"action":"move-back","error":"current path must be under $HOME or /tmp"}' >&2
    exit 1
fi
if [[ ! "$ORIGINAL_PATH" == "$HOME_DIR"* ]] && [[ ! "$ORIGINAL_PATH" == /tmp/* ]]; then
    echo '{"action":"move-back","error":"original path must be under $HOME or /tmp"}' >&2
    exit 1
fi

# Idempotency check: if file already at original location, no-op
if [[ ! -e "$CURRENT_PATH" ]] && [[ -e "$ORIGINAL_PATH" ]]; then
    echo '{"action":"move-back","path":"'"$ORIGINAL_PATH"'","status":"no_change","dry_run":false}'
    exit 0
fi

if [[ ! -f "$CURRENT_PATH" ]]; then
    echo '{"action":"move-back","error":"current file not found","path":"'"$CURRENT_PATH"'"}' >&2
    exit 1
fi

# Dry-run: report but don't act
if [[ "$DRY_RUN" == "1" ]]; then
    echo '{"action":"move-back","from":"'"$CURRENT_PATH"'","to":"'"$ORIGINAL_PATH"'","status":"would_move","dry_run":true}'
    exit 0
fi

# Create parent dir if needed
PARENT_DIR=$(dirname "$ORIGINAL_PATH")
mkdir -p "$PARENT_DIR"

# Move back
mv "$CURRENT_PATH" "$ORIGINAL_PATH"

TIMESTAMP=$(date -Iseconds)
echo '{"action":"move-back","from":"'"$CURRENT_PATH"'","to":"'"$ORIGINAL_PATH"'","status":"moved","dry_run":false,"timestamp":"'"$TIMESTAMP"'"}'

exit 0

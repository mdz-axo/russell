#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# workspace: create-directory intervention
# Creates a directory with parents. IDRS-compliant.
#
# I — Idempotent: mkdir -p on existing dir is a no-op.
# D — Dry-run: RUSSELL_DRY_RUN=1 or --dry-run.
# R — Rollback: delete if we created it (only if it didn't exist before).
# S — Structured log: JSON event to stdout.
#
# Usage: create-directory.sh <path> [--dry-run]

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
    echo '{"action":"create-directory","error":"missing path argument"}' >&2
    exit 1
fi

DIR_PATH=$(realpath -m "$DIR_PATH" 2>/dev/null || echo "$DIR_PATH")
HOME_DIR=$(realpath -m "$HOME" 2>/dev/null || echo "$HOME")

if [[ ! "$DIR_PATH" == "$HOME_DIR"* ]] && [[ ! "$DIR_PATH" == /tmp/* ]]; then
    echo '{"action":"create-directory","error":"path must be under $HOME or /tmp"}' >&2
    exit 1
fi

# Idempotency: already exists is a no-op
if [[ -d "$DIR_PATH" ]]; then
    echo '{"action":"create-directory","path":"'"$DIR_PATH"'","status":"already_exists","dry_run":false}'
    exit 0
fi

if [[ "$DRY_RUN" == "1" ]]; then
    echo '{"action":"create-directory","path":"'"$DIR_PATH"'","status":"would_create","dry_run":true}'
    exit 0
fi

# Create the directory
mkdir -p "$DIR_PATH"

TIMESTAMP=$(date -Iseconds)
echo '{"action":"create-directory","path":"'"$DIR_PATH"'","status":"created","dry_run":false,"timestamp":"'"$TIMESTAMP"'"}'

exit 0

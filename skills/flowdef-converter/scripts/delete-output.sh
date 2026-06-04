#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# flowdef-converter: delete-output rollback intervention
# Deletes the converted output files produced by the convert intervention. IDRS-compliant.
#
# I — Idempotent: if output directory doesn't exist, no-op.
# D — Dry-run: RUSSELL_DRY_RUN=1 or --dry-run shows what would happen.
# R — Rollback: this IS the rollback — removes converted output.
# S — Structured log: JSON event to stdout.
#
# Usage: delete-output.sh <output_dir> [--dry-run]

set -euo pipefail

OUTPUT_DIR="${1:-}"
DRY_RUN="${RUSSELL_DRY_RUN:-0}"
shift || true

for arg in "$@"; do
    if [[ "$arg" == "--dry-run" ]]; then
        DRY_RUN=1
    fi
done

if [[ -z "$OUTPUT_DIR" ]]; then
    echo '{"action":"delete-output","error":"missing output_dir argument"}' >&2
    exit 1
fi

# Security: resolve and check path stays under HOME or /tmp
OUTPUT_DIR=$(realpath -m "$OUTPUT_DIR" 2>/dev/null || echo "$OUTPUT_DIR")
HOME_DIR=$(realpath -m "$HOME" 2>/dev/null || echo "$HOME")

if [[ ! "$OUTPUT_DIR" == "$HOME_DIR"* ]] && [[ ! "$OUTPUT_DIR" == /tmp/* ]]; then
    echo '{"action":"delete-output","error":"path must be under $HOME or /tmp"}' >&2
    exit 1
fi

# Idempotency check: if output directory doesn't exist, no-op
if [[ ! -d "$OUTPUT_DIR" ]]; then
    echo '{"action":"delete-output","path":"'"$OUTPUT_DIR"'","status":"no_change","dry_run":false}'
    exit 0
fi

# Dry-run: report but don't act
if [[ "$DRY_RUN" == "1" ]]; then
    echo '{"action":"delete-output","path":"'"$OUTPUT_DIR"'","status":"would_delete","dry_run":true}'
    exit 0
fi

# Delete the output directory
rm -rf "$OUTPUT_DIR"

TIMESTAMP=$(date -Iseconds)
echo '{"action":"delete-output","path":"'"$OUTPUT_DIR"'","status":"deleted","dry_run":false,"timestamp":"'"$TIMESTAMP"'"}'

exit 0

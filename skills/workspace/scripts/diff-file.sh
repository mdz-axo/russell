#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# workspace: diff-file probe
# Shows diff between a file and its backup. Read-only.

set -euo pipefail

FILE_PATH="${1:-}"
BACKUP_DIR="${RUSSELL_BACKUP_DIR:-$HOME/.local/share/harness/backups}"

if [[ -z "$FILE_PATH" ]]; then
    echo "Usage: diff-file.sh <path>" >&2
    exit 1
fi

FILE_PATH=$(realpath -m "$FILE_PATH" 2>/dev/null || echo "$FILE_PATH")

if [[ ! -f "$FILE_PATH" ]]; then
    echo "not_found: ${FILE_PATH}"
    exit 1
fi

# Find the most recent backup
BASENAME=$(basename "$FILE_PATH")
LATEST_BACKUP=$(ls -t "${BACKUP_DIR}/"*-"${BASENAME}.bak" 2>/dev/null | head -1 || true)

if [[ -z "$LATEST_BACKUP" ]]; then
    echo "no_backup_found"
    exit 0
fi

echo "backup: ${LATEST_BACKUP}"
echo "---"
diff -u "$LATEST_BACKUP" "$FILE_PATH" 2>/dev/null || true

exit 0

#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# workspace: read-file probe
# Reads a file's content (first 200 lines). Read-only, no mutations.

set -euo pipefail

FILE_PATH="${1:-}"

if [[ -z "$FILE_PATH" ]]; then
    echo "Usage: read-file.sh <path>" >&2
    exit 1
fi

# Security: resolve and check path stays within allowed boundaries
FILE_PATH=$(realpath -m "$FILE_PATH" 2>/dev/null || echo "$FILE_PATH")
HOME_DIR=$(realpath -m "$HOME" 2>/dev/null || echo "$HOME")

if [[ ! "$FILE_PATH" == "$HOME_DIR"* ]] && [[ ! "$FILE_PATH" == /tmp/* ]]; then
    echo "Error: path must be under $HOME or /tmp" >&2
    exit 1
fi

if [[ ! -f "$FILE_PATH" ]]; then
    echo "not_found: ${FILE_PATH}"
    exit 1
fi

if [[ ! -r "$FILE_PATH" ]]; then
    echo "not_readable: ${FILE_PATH}"
    exit 1
fi

# Output with line numbers, head -200 to avoid flooding
echo "path: ${FILE_PATH}"
echo "size: $(stat -c%s "$FILE_PATH" 2>/dev/null || echo "?")"
echo "---"
head -200 "$FILE_PATH"
exit 0

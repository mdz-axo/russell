#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# workspace: stat-file probe
# Reports file metadata (exists, size, permissions, mod time). Read-only.

set -euo pipefail

FILE_PATH="${1:-}"

if [[ -z "$FILE_PATH" ]]; then
    echo "Usage: stat-file.sh <path>" >&2
    exit 1
fi

FILE_PATH=$(realpath -m "$FILE_PATH" 2>/dev/null || echo "$FILE_PATH")
HOME_DIR=$(realpath -m "$HOME" 2>/dev/null || echo "$HOME")

if [[ ! "$FILE_PATH" == "$HOME_DIR"* ]] && [[ ! "$FILE_PATH" == /tmp/* ]]; then
    echo "Error: path must be under $HOME or /tmp" >&2
    exit 1
fi

if [[ ! -e "$FILE_PATH" ]]; then
    echo "not_found"
    exit 0
fi

# Collect metadata
if [[ -d "$FILE_PATH" ]]; then
    kind="directory"
    size=$(du -sh "$FILE_PATH" 2>/dev/null | cut -f1 || echo "?")
elif [[ -f "$FILE_PATH" ]]; then
    kind="file"
    size=$(stat -c%s "$FILE_PATH" 2>/dev/null || echo "?")
else
    kind="other"
    size="?"
fi

perms=$(stat -c%a "$FILE_PATH" 2>/dev/null || echo "?")
mtime=$(stat -c%Y "$FILE_PATH" 2>/dev/null || echo "?")
mtime_iso=$(date -d "@${mtime}" -Iseconds 2>/dev/null || echo "?")

echo "path: ${FILE_PATH}"
echo "kind: ${kind}"
echo "size: ${size}"
echo "perms: ${perms}"
echo "mtime: ${mtime_iso}"

exit 0

#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# workspace: list-dir probe
# Lists directory contents. Read-only.

set -euo pipefail

DIR_PATH="${1:-}"

if [[ -z "$DIR_PATH" ]]; then
    echo "Usage: list-dir.sh <path>" >&2
    exit 1
fi

DIR_PATH=$(realpath -m "$DIR_PATH" 2>/dev/null || echo "$DIR_PATH")
HOME_DIR=$(realpath -m "$HOME" 2>/dev/null || echo "$HOME")

if [[ ! "$DIR_PATH" == "$HOME_DIR"* ]] && [[ ! "$DIR_PATH" == /tmp/* ]]; then
    echo "Error: path must be under $HOME or /tmp" >&2
    exit 1
fi

if [[ ! -d "$DIR_PATH" ]]; then
    echo "not_found: ${DIR_PATH}"
    exit 1
fi

echo "path: ${DIR_PATH}"
echo "---"

# List with type indicators: / for dirs, * for executables
ls -1F "$DIR_PATH" 2>/dev/null || echo "(empty or unreadable)"

exit 0

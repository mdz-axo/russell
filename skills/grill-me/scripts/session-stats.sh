#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# grill-me: session-stats probe
# Reports on any saved grilling session records in memory/grill-sessions/.
# Sessions are optional — they only exist if the operator asked Jack to save
# a gap analysis to disk.

set -euo pipefail

MEMORY_DIR="${RUSSELL_MEMORY_DIR:-$HOME/.local/share/harness/memory/grill-sessions}"

if [[ ! -d "$MEMORY_DIR" ]]; then
    echo "no_sessions"
    echo "sessions_dir: ${MEMORY_DIR}"
    echo "total: 0"
    exit 0
fi

# Count session files
session_files=()
while IFS= read -r -d '' f; do
    session_files+=("$f")
done < <(find "$MEMORY_DIR" -name "*.md" -print0 2>/dev/null)

total=${#session_files[@]}

echo "sessions_dir: ${MEMORY_DIR}"
echo "total: ${total}"

if [[ $total -eq 0 ]]; then
    echo "(no saved sessions)"
    exit 0
fi

# Show the most recent sessions (last 5)
echo ""
echo "recent_sessions:"
count=0
for f in "$(ls -t "${MEMORY_DIR}"/*.md 2>/dev/null)"; do
    if [[ $count -ge 5 ]]; then
        break
    fi
    basename=$(basename "$f" .md)
    # Extract topic from filename pattern: YYYY-MM-DD-<topic>.md
    topic=$(echo "$basename" | sed 's/^[0-9]\{4\}-[0-9]\{2\}-[0-9]\{2\}-//' | tr '-' ' ')
    date_part=$(echo "$basename" | cut -d'-' -f1-3)
    # Count ratings in the file
    solid=$(grep -c "Solid\|🟢" "$f" 2>/dev/null || echo "0")
    partial=$(grep -c "Partial\|🟡" "$f" 2>/dev/null || echo "0")
    gap=$(grep -c "Gap\|🔴" "$f" 2>/dev/null || echo "0")
    echo "  ${date_part} | topic: ${topic} | solid:${solid} partial:${partial} gap:${gap}"
    ((count++)) || true
done

exit 0

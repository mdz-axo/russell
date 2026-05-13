#!/bin/bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# sysadmin: probe-coredumps
# Checks for coredump files and reports count with total size.
# Output: count and total size in bytes.
set -euo pipefail

COREDUMP_DIR="/var/lib/systemd/coredump"

if [ ! -d "$COREDUMP_DIR" ]; then
    echo "count=0"
    echo "total_bytes=0"
    echo "dir_missing"
    exit 0
fi

# Count lz4/zst compressed core files
CORE_FILES=$(find "$COREDUMP_DIR" -name 'core.*' -type f 2>/dev/null || true)
if [ -z "$CORE_FILES" ]; then
    echo "count=0"
    echo "total_bytes=0"
    echo "all_clear"
    exit 0
fi

COUNT=$(echo "$CORE_FILES" | wc -l)
TOTAL_BYTES=$(echo "$CORE_FILES" | xargs -r stat -c '%s' 2>/dev/null | awk '{s+=$1} END {print s}' || echo "0")

echo "count=$COUNT"
echo "total_bytes=$TOTAL_BYTES"
TOTAL_MB=$(echo "scale=1; $TOTAL_BYTES / 1048576" | bc 2>/dev/null || echo "0")
echo "total_mb=$TOTAL_MB"

# Show oldest and newest
OLDEST=$(echo "$CORE_FILES" | xargs -r stat -c '%Y %n' 2>/dev/null | sort -n | head -1 | awk '{print $2}' || echo "none")
NEWEST=$(echo "$CORE_FILES" | xargs -r stat -c '%Y %n' 2>/dev/null | sort -n | tail -1 | awk '{print $2}' || echo "none")
echo "oldest=$OLDEST"
echo "newest=$NEWEST"

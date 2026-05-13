#!/bin/bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# sysadmin: probe-zombies
# Counts zombie processes and lists their PID/name/PPID for diagnostics.
# Output: count and per-zombie detail.
set -euo pipefail

ZOMBIES=$(ps -eo pid,state,ppid,comm --no-headers 2>/dev/null | awk '$2 ~ /Z/ {print $1, $3, $4}' || true)

if [ -z "$ZOMBIES" ]; then
    echo "count=0"
    echo "all_clear"
    exit 0
fi

COUNT=$(echo "$ZOMBIES" | wc -l)
echo "count=$COUNT"

echo "$ZOMBIES" | while read -r pid ppid comm; do
    # Resolve parent command name
    if [ -n "$ppid" ] && [ -d "/proc/$ppid" ]; then
        PNAME=$(cat "/proc/$ppid/comm" 2>/dev/null || echo "unknown")
    else
        PNAME="(reparented to init)"
    fi
    echo "zombie pid=$pid ppid=$ppid parent=\"$PNAME\" name=\"$comm\""
done

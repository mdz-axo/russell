#!/bin/bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# sysadmin: reap-zombies
# Identifies zombie processes and reaps them by signalling their parents.
#
# Strategy:
#   1. Find all zombies (state Z)
#   2. Group by parent PID
#   3. For each parent: send SIGCHLD to trigger waitpid() reaping
#   4. If parent is unresponsive or already exited: zombies are reparented
#      to init (PID 1), init will reap them automatically — nothing to do.
#
# IDRS: idempotent — signalling a parent that already reaped is harmless.
# Risk: medium — signalling the wrong process could have side effects
#        (mitigated: we signal with SIGCHLD, not SIGKILL).
# Rollback: none_needed — zombies are already dead, just unreaped.
set -euo pipefail

REAPED=0
SKIPPED=0

# Find zombies and their parent PIDs
ZOMBIES=$(ps -eo pid,state,ppid,comm --no-headers 2>/dev/null | awk '$2 ~ /Z/ {print $1, $3}' || true)

if [ -z "$ZOMBIES" ]; then
    echo "no zombies found"
    echo "reaped=0"
    echo "skipped=0"
    exit 0
fi

# Collect unique parent PIDs
declare -A PARENTS
while read -r zpid ppid; do
    # Skip if parent is init — init manages its own children
    if [ "$ppid" = "1" ] || [ "$ppid" = "0" ]; then
        echo "zombie pid=$zpid ppid=$ppid — reparented to init, will auto-reap"
        SKIPPED=$((SKIPPED + 1))
        continue
    fi

    # Skip if parent no longer exists
    if [ ! -d "/proc/$ppid" ]; then
        echo "zombie pid=$zpid ppid=$ppid — parent gone, will auto-reap"
        SKIPPED=$((SKIPPED + 1))
        continue
    fi

    PARENTS["$ppid"]=$((PARENTS["$ppid"] + 1))
done <<< "$ZOMBIES"

# Signal each unique parent with SIGCHLD
for PPID in "${!PARENTS[@]}"; do
    ZOMBIE_COUNT=${PARENTS[$PPID]}
    PNAME=$(cat "/proc/$PPID/comm" 2>/dev/null || echo "unknown")

    echo "signalling parent pid=$PPID name=\"$PNAME\" zombies=$ZOMBIE_COUNT with SIGCHLD"

    if kill -SIGCHLD "$PPID" 2>/dev/null; then
        echo "  OK: sent SIGCHLD to $PPID ($PNAME)"
        REAPED=$((REAPED + ZOMBIE_COUNT))
    else
        echo "  FAIL: cannot signal $PPID ($PNAME) — zombie count=$ZOMBIE_COUNT"
        SKIPPED=$((SKIPPED + ZOMBIE_COUNT))
    fi
done

# Wait a moment for parents to reap
sleep 1

# Verify: count remaining zombies
REMAINING=$(ps -eo pid,state --no-headers 2>/dev/null | awk '$2 ~ /Z/' | wc -l || echo "0")
echo "reaped=$REAPED"
echo "skipped=$SKIPPED"
echo "remaining=$REMAINING"

if [ "$REMAINING" -gt 0 ]; then
    echo "WARN: $REMAINING zombies still present after signalling"
    exit 1
fi

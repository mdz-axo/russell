#!/bin/bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# sysadmin: remove-coredumps
# Removes all coredump files from systemd's coredump directory.
#
# IDRS: idempotent — removing already-removed files is a no-op.
# Risk: low — coredumps are forensic artifacts, not system state.
#        The operator should review any crash patterns before removing.
# Rollback: none_needed — cannot meaningfully restore deleted coredumps
#        (operator should use a backup if forensics are needed).
set -euo pipefail

COREDUMP_DIR="/var/lib/systemd/coredump"

if [ ! -d "$COREDUMP_DIR" ]; then
    echo "no coredump directory"
    echo "removed=0"
    echo "bytes_freed=0"
    exit 0
fi

# Count and measure before removal
CORE_FILES=$(find "$COREDUMP_DIR" -name 'core.*' -type f 2>/dev/null || true)

if [ -z "$CORE_FILES" ]; then
    echo "no coredumps found"
    echo "removed=0"
    echo "bytes_freed=0"
    exit 0
fi

COUNT=$(echo "$CORE_FILES" | wc -l)
BYTES=$(echo "$CORE_FILES" | xargs -r stat -c '%s' 2>/dev/null | awk '{s+=$1} END {print s}' || echo "0")
BYTES_MB=$(echo "scale=1; $BYTES / 1048576" | bc 2>/dev/null || echo "0")

echo "removing $COUNT coredump files ($BYTES_MB MB)..."

rm -f $CORE_FILES 2>/dev/null || {
    echo "ERROR: failed to remove some coredump files"
    exit 1
}

echo "OK: removed $COUNT coredumps"
echo "removed=$COUNT"
echo "bytes_freed=$BYTES"

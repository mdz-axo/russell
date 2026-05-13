#!/bin/bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# sysadmin: probe-systemd-failed
# Lists failed user-level systemd units with their descriptions.
# Output: count and unit list or "none".
set -euo pipefail

FAILED=$(systemctl --user list-units --failed --no-legend 2>/dev/null || true)

if [ -z "$FAILED" ]; then
    echo "count=0"
    echo "all_clear"
    exit 0
fi

COUNT=$(echo "$FAILED" | wc -l)
echo "count=$COUNT"
echo "$FAILED" | while read -r unit load active sub description; do
    echo "unit=$unit state=$active/$sub desc=\"$description\""
done

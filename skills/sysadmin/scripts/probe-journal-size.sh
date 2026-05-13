#!/bin/bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# sysadmin: probe-journal-size
# Reports journal disk usage for both user and system journals.
# Output: size in megabytes for each.
set -euo pipefail

# User journal
if command -v journalctl &>/dev/null; then
    USER_DISK=$(journalctl --user --disk-usage 2>/dev/null | awk '{print $NF}' || echo "0")
    echo "user_journal_bytes=$USER_DISK"
    USER_MB=$(echo "scale=1; $USER_DISK / 1048576" | bc 2>/dev/null || echo "0")
    echo "user_journal_mb=$USER_MB"
else
    echo "user_journal_bytes=0"
    echo "user_journal_mb=0"
fi

# System journal (requires sudo; may fail silently)
SYSTEM_DISK=$(journalctl --disk-usage 2>/dev/null | awk '{print $NF}' || echo "0")
echo "system_journal_bytes=$SYSTEM_DISK"
SYSTEM_MB=$(echo "scale=1; $SYSTEM_DISK / 1048576" | bc 2>/dev/null || echo "0")
echo "system_journal_mb=$SYSTEM_MB"

#!/bin/bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# sysadmin: probe-swap-detail
# Reports swap usage, pressure, and top swap-consuming processes.
# Output: total/used swap, pressure metrics, top consumers.
set -euo pipefail

# Swap totals
if [ -r /proc/meminfo ]; then
    SWAP_TOTAL=$(awk '/SwapTotal/ {print $2}' /proc/meminfo 2>/dev/null || echo "0")
    SWAP_FREE=$(awk '/SwapFree/ {print $2}' /proc/meminfo 2>/dev/null || echo "0")
    SWAP_USED=$((SWAP_TOTAL - SWAP_FREE))
    echo "swap_total_kb=$SWAP_TOTAL"
    echo "swap_used_kb=$SWAP_USED"
    echo "swap_free_kb=$SWAP_FREE"
else
    echo "swap_total_kb=0"
    echo "swap_used_kb=0"
    echo "swap_free_kb=0"
fi

# Swap pressure (PSI)
if [ -r /proc/pressure/memory ]; then
    PRESSURE=$(cat /proc/pressure/memory 2>/dev/null || true)
    echo "memory_pressure=$PRESSURE"
fi

# Top swap consumers (top 5 by swap usage)
TOPSWAP=$(grep VmSwap /proc/*/status 2>/dev/null | sort -k2 -nr | head -5 | while read -r line; do
    PID=$(echo "$line" | cut -d/ -f3)
    SWAP_KB=$(echo "$line" | awk '{print $2}')
    CMD=$(tr '\0' ' ' < "/proc/$PID/cmdline" 2>/dev/null | head -c 80 || echo "unknown")
    echo "pid=$PID swap_kb=$SWAP_KB cmd=\"$CMD\""
done || true)
echo "$TOPSWAP"

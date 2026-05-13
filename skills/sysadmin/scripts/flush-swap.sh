#!/bin/bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# sysadmin: flush-swap
# Flushes swap by turning it off and back on.
# Also drops page cache first to maximise available memory for swapoff.
#
# WARNING: swapoff requires enough free RAM to hold all swapped pages.
# If RAM is insufficient, swapoff will fail and the script exits safely.
#
# IDRS: idempotent — end state is always "swap enabled and empty".
# Risk: medium — swapoff can be slow (paging everything back to RAM).
# Rollback: none_needed — swap state is transient by nature.
set -euo pipefail

# Check if swap is even active
SWAP_TOTAL=$(awk '/SwapTotal/ {print $2}' /proc/meminfo 2>/dev/null || echo "0")
if [ "$SWAP_TOTAL" -eq 0 ]; then
    echo "no swap configured"
    exit 0
fi

SWAP_USED=$(awk '/SwapUsed/ {print $2}' /proc/meminfo 2>/dev/null |
    awk '{print $1}' || echo "0")

echo "swap_total_kb=$SWAP_TOTAL swap_used_kb=$SWAP_USED"

# Check available memory (MemAvailable)
AVAIL_KB=$(awk '/MemAvailable/ {print $2}' /proc/meminfo 2>/dev/null || echo "0")
echo "mem_available_kb=$AVAIL_KB"

# Drop caches to free page cache (gives swapoff more room)
echo "dropping page cache..."
echo 3 > /proc/sys/vm/drop_caches 2>/dev/null || true
sleep 1

# Re-check available after cache drop
AVAIL_KB=$(awk '/MemAvailable/ {print $2}' /proc/meminfo 2>/dev/null || echo "0")
echo "mem_available_kb_after_drop=$AVAIL_KB"

# Safety check: do we have enough free RAM?
if [ "$AVAIL_KB" -lt "$SWAP_USED" ]; then
    echo "WARN: available memory ($AVAIL_KB kB) < swap used ($SWAP_USED kB)"
    echo "swapoff may fail or be very slow"
fi

# Get list of swap devices
SWAP_DEVICES=$(awk 'NR>1 {print $1}' /proc/swaps 2>/dev/null || true)
if [ -z "$SWAP_DEVICES" ]; then
    echo "no active swap devices"
    exit 0
fi

echo "deactivating swap devices: $SWAP_DEVICES"

# Swap off all devices
for dev in $SWAP_DEVICES; do
    echo "swapoff $dev..."
    if swapoff "$dev" 2>&1; then
        echo "  OK: deactivated $dev"
    else
        echo "  FAILED: swapoff $dev — aborting"
        exit 1
    fi
done

echo "reactivating swap..."

# Swap back on — all devices listed in /etc/fstab with type swap
swapon -a 2>&1

# Verify
SWAP_USED_AFTER=$(awk '/SwapUsed/ {print $2}' /proc/meminfo 2>/dev/null |
    awk '{print $1}' || echo "0")
echo "OK: swap flushed, now using ${SWAP_USED_AFTER} kB"

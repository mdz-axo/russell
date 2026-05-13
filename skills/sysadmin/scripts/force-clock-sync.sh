#!/bin/bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# sysadmin: force-clock-sync
# Force immediate NTP clock synchronisation.
# Tries chronyc first (Ubuntu 25.10 default), falls back to timedatectl.
#
# IDRS: idempotent — running twice yields same clock state.
# Risk: medium — clock jumps can confuse applications.
# Rollback: none_needed — cannot meaningfully "un-sync" a clock.
set -euo pipefail

if command -v chronyc &>/dev/null; then
    echo "Using chronyc to force sync..."
    # -a: use all available sources
    # makestep: step the clock immediately (don't slew)
    if chronyc -a makestep 2>&1; then
        echo "OK: chronyc makestep succeeded"

        # Report new offset
        SYNC_INFO=$(chronyc -c tracking 2>/dev/null || true)
        if [ -n "$SYNC_INFO" ]; then
            IFS=',' read -r _ _ _ _ OFFSET _ <<< "$SYNC_INFO"
            echo "post_sync_offset=$OFFSET"
        fi
        exit 0
    else
        echo "WARN: chronyc makestep failed, trying timedatectl..."
    fi
fi

if command -v timedatectl &>/dev/null; then
    echo "Using timedatectl to cycle NTP..."
    # Cycle NTP off and on to force a fresh sync
    timedatectl set-ntp false 2>&1
    sleep 1
    timedatectl set-ntp true 2>&1
    echo "OK: timedatectl NTP cycle complete"

    # Report new status
    timedatectl show -p NTPSynchronized 2>/dev/null || true
    exit 0
fi

echo "ERROR: no NTP client available (chronyc or timedatectl required)"
exit 1

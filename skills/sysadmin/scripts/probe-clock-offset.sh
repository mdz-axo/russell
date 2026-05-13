#!/bin/bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# sysadmin: probe-clock-offset
# Checks NTP sync status via chronyc (Ubuntu 25.10 default) or timedatectl.
# Output: offset in seconds, stratum, and sync source.
set -euo pipefail

if command -v chronyc &>/dev/null; then
    TRACKING=$(chronyc -c tracking 2>/dev/null || true)
    if [ -n "$TRACKING" ]; then
        # chronyc -c tracking CSV: refid,stratum,ref_time,system_time,last_offset,rms_offset,...
        IFS=',' read -r REFID STRATUM _ SYS_TIME LAST_OFFSET RMS_OFFSET _ <<< "$TRACKING"
        echo "source=chrony"
        echo "stratum=$STRATUM"
        echo "offset_seconds=$LAST_OFFSET"
        echo "rms_offset_seconds=$RMS_OFFSET"
        echo "refid=$REFID"

        # Check if synced: offset magnitude
        OFFSET_ABS=$(echo "$LAST_OFFSET" | awk '{print ($1 < 0) ? -$1 : $1}' 2>/dev/null || echo "999")
        if [ "$(echo "$OFFSET_ABS > 5.0" | bc 2>/dev/null || echo 1)" -eq 1 ]; then
            echo "status=desynced"
        elif [ "$(echo "$OFFSET_ABS > 1.0" | bc 2>/dev/null || echo 1)" -eq 1 ]; then
            echo "status=drifting"
        else
            echo "status=synced"
        fi
        exit 0
    fi
fi

# Fallback to timedatectl
if command -v timedatectl &>/dev/null; then
    NTP=$(timedatectl show -p NTP -p NTPSynchronized 2>/dev/null || true)
    echo "source=timedatectl"
    echo "$NTP"
    if echo "$NTP" | grep -q "NTPSynchronized=yes"; then
        echo "status=synced"
    else
        echo "status=desynced"
    fi
    exit 0
fi

echo "source=none"
echo "status=unknown"

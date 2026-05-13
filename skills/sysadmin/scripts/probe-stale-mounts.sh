#!/bin/bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# sysadmin: probe-stale-mounts
# Identifies mounts that are stuck or unreachable.
# Output: count and list of stale/unreachable mount points.
set -euo pipefail

STALE_COUNT=0

# Check each mount by attempting stat on the mountpoint
# with a short timeout via timeout command
mount 2>/dev/null | while read -r line; do
    MP=$(echo "$line" | awk '{print $3}')
    FS=$(echo "$line" | awk '{print $5}')

    # Skip virtual filesystems
    case "$FS" in
        proc|sysfs|devtmpfs|devpts|tmpfs|cgroup|cgroup2|pstore|bpf|debugfs|tracefs|fusectl|configfs|securityfs|hugetlbfs|mqueue|autofs|binfmt_misc|rpc_pipefs|nfsd|overlay)
            continue ;;
    esac

    # Skip common root-like mountpoints
    if [ "$MP" = "/" ] || [ "$MP" = "/boot" ] || [ "$MP" = "/boot/efi" ]; then
        continue
    fi

    # Quick liveness check
    if ! timeout 2 stat "$MP" &>/dev/null; then
        echo "stale mountpoint=$MP fstype=$FS"
        STALE_COUNT=$((STALE_COUNT + 1))
    fi
done

if [ "$STALE_COUNT" -eq 0 ]; then
    echo "stale_count=0"
    echo "all_clear"
fi

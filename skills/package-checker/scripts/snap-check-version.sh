#!/usr/bin/env bash
# package-checker: snap-check-version probe
# Checks if a snap package is installed and reports its version.
# Read-only, no mutations.

set -euo pipefail

PACKAGE="${1:-}"

if [[ -z "$PACKAGE" ]; then
    echo "Usage: snap-check-version.sh <package-name>" >&2
    echo "Example: snap-check-version.sh node" >&2
    exit 1
fi

if ! command -v snap &>/dev/null; then
    echo "Error: snap is not installed" >&2
    exit 1
fi

# Check if package is installed
snap_info=$(snap list "$PACKAGE" 2>/dev/null || true)

if [[ -z "$snap_info" ]]; then
    echo "Not installed (snap): ${PACKAGE}"
    echo ""
    # Check if available in snap store
    available=$(snap find "$PACKAGE" 2>/dev/null | head -5 || true)
    if [[ -n "$available" ]]; then
        echo "Available in snap store:"
        echo "$available"
    fi
    exit 0
fi

# Parse snap info
version=$(echo "$snap_info" | awk 'NR==2 {print $2}')
rev=$(echo "$snap_info" | awk 'NR==2 {print $3}')
channel=$(echo "$snap_info" | awk 'NR==2 {print $4}')

echo "Installed (snap): ${PACKAGE}"
echo "  Version:  ${version}"
echo "  Revision: ${rev}"
echo "  Channel:  ${channel}"
echo ""

exit 0

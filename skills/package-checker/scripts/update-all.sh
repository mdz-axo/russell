#!/usr/bin/env bash
# package-checker: update-all intervention
# Upgrades all packages to their latest versions.

set -euo pipefail

# Check if running as root (should be, since needs_sudo: true)
if [[ $EUID -ne 0 ]]; then
    echo "Error: This script must run as root (via sudo)" >&2
    exit 1
fi

echo "System-wide package update"
echo "=========================="
echo ""

# Update package lists
echo "[1/3] Updating package lists..."
if ! apt-get update -qq; then
    echo "✗ Failed to update package lists" >&2
    exit 3
fi
echo "✓ Package lists updated"
echo ""

# Get list of upgradable packages
upgradable=$(apt list --upgradable 2>/dev/null | grep -v "^Listing" || true)

if [[ -z "$upgradable" ]]; then
    echo "✓ All packages are already up to date"
    exit 2
fi

count=$(echo "$upgradable" | wc -l)
echo "[2/3] Found $count package(s) to upgrade"
echo ""

# Perform upgrade
echo "[3/3] Upgrading packages..."
echo ""

if apt-get upgrade -y; then
    echo ""
    echo "✓ System update completed successfully"
    echo "  Upgraded: $count package(s)"
    exit 0
else
    echo ""
    echo "✗ System update failed" >&2
    exit 3
fi

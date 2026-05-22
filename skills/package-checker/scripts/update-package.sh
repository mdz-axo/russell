#!/usr/bin/env bash
# package-checker: update-package intervention
# Upgrades a single package to the latest version.

set -euo pipefail

PACKAGE="${1:-}"

if [[ -z "$PACKAGE" ]]; then
    echo "Usage: update-package.sh <package-name>" >&2
    exit 1
fi

# Check if running as root (should be, since needs_sudo: true)
if [[ $EUID -ne 0 ]]; then
    echo "Error: This script must run as root (via sudo)" >&2
    exit 1
fi

# Check if package is installed
if ! dpkg-query -W -f='${Status}' "$PACKAGE" 2>/dev/null | grep -q "install ok installed"; then
    echo "Error: Package '$PACKAGE' is not installed" >&2
    exit 1
fi

# Get current version
current_version=$(dpkg-query -W -f='${Version}' "$PACKAGE")
echo "Current version: $PACKAGE=$current_version"
echo ""

# Update package lists
echo "Updating package lists..."
apt-get update -qq

# Check if update is available
new_version=$(apt-cache policy "$PACKAGE" 2>/dev/null | grep "Candidate:" | awk '{print $2}')

if [[ -z "$new_version" ]] || [[ "$current_version" == "$new_version" ]]; then
    echo "✓ Package '$PACKAGE' is already up to date"
    exit 2
fi

echo "Available version: $PACKAGE=$new_version"
echo ""
echo "Upgrading $PACKAGE..."
echo ""

# Perform upgrade
if apt-get install --only-upgrade -y "$PACKAGE"; then
    echo ""
    echo "✓ Successfully upgraded $PACKAGE"
    echo "  From: $current_version"
    echo "  To:   $new_version"
    exit 0
else
    echo ""
    echo "✗ Failed to upgrade $PACKAGE" >&2
    exit 3
fi

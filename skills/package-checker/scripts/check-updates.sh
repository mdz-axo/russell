#!/usr/bin/env bash
# package-checker: check-updates probe
# Checks for available package updates (read-only, no apt state changes).

set -euo pipefail

echo "Checking for available updates..."
echo ""

# Note: We don't run apt-get update here because:
# 1. This is a probe (risk: none) that shouldn't modify system state
# 2. apt-get update requires root access
# 3. The cached package list is usually sufficient for checking
# Users should run update-package or update-all to refresh lists and upgrade.

# Get list of upgradable packages from cached state
upgradable=$(apt list --upgradable 2>/dev/null | grep -v "^Listing" || true)

if [[ -z "$upgradable" ]]; then
    echo "✓ All packages are up to date (based on cached state)"
    echo ""
    echo "Note: Run 'ACTION: package-checker/update-package <pkg>' to refresh lists and upgrade."
    exit 0
fi

count=$(echo "$upgradable" | wc -l)
echo "Found $count package(s) with available updates (cached state):"
echo ""
printf "  %-40s %-20s → %s\n" "PACKAGE" "INSTALLED" "AVAILABLE"
printf "  %-40s %-20s → %s\n" "-------" "---------" "---------"

echo "$upgradable" | while read -r line; do
    package=$(echo "$line" | cut -d'/' -f1)
    current=$(echo "$line" | awk '{print $2}')
    available=$(echo "$line" | awk '{print $3}')
    printf "  %-40s %-20s → %s\n" "$package" "$current" "$available"
done

echo ""
echo "To update a specific package (refreshes lists + upgrades):"
echo "  ACTION: package-checker/update-package <package-name>"
echo ""
echo "To update all packages:"
echo "  ACTION: package-checker/update-all"

echo ""
exit 0

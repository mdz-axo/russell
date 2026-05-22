#!/usr/bin/env bash
# package-checker: list-installed probe
# Lists count and sample of installed packages.

set -euo pipefail

echo "Installed packages (sample):"
echo ""

# Get first 20 installed packages
dpkg-query -W -f='${Package}=${Version}\n' 2>/dev/null | sort | head -20 | while read -r line; do
    package=$(echo "$line" | cut -d'=' -f1)
    version=$(echo "$line" | cut -d'=' -f2-)
    printf "  %-50s %s\n" "$package" "$version"
done

echo ""
total=$(dpkg-query -W 2>/dev/null | wc -l)
echo "Total: $total packages installed (showing first 20)"
echo ""
exit 0

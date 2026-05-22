#!/usr/bin/env bash
# package-checker: check-version probe
# Reports the exact version of a specific package.

set -euo pipefail

PACKAGE="${1:-}"

if [[ -z "$PACKAGE" ]]; then
    echo "Usage: check-version.sh <package-name>" >&2
    exit 1
fi

# Check if package is installed
if dpkg-query -W -f='${Status}' "$PACKAGE" 2>/dev/null | grep -q "install ok installed"; then
    VERSION=$(dpkg-query -W -f='${Version}' "$PACKAGE")
    echo "Installed: $PACKAGE=$VERSION"
    exit 0
else
    echo "Not installed: $PACKAGE"
    exit 0
fi

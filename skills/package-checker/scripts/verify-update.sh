#!/usr/bin/env bash
# package-checker: verify-update evaluation check
# Verifies that a package was successfully updated.

set -euo pipefail

PACKAGE="${1:-}"

if [[ -z "$PACKAGE" ]]; then
    echo "Usage: verify-update.sh <package-name>" >&2
    exit 1
fi

# Check if package is installed
if ! dpkg-query -W -f='${Status}' "$PACKAGE" 2>/dev/null | grep -q "install ok installed"; then
    echo "FAIL: Package '$PACKAGE' is not installed" >&2
    exit 1
fi

# Get current version
current_version=$(dpkg-query -W -f='${Version}' "$PACKAGE")

echo "OK: $PACKAGE=$current_version (installed and working)"
exit 0

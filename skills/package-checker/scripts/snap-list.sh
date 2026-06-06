#!/usr/bin/env bash
# package-checker: snap-list probe
# Lists installed snap packages.
# Read-only, no mutations.

set -euo pipefail

if ! command -v snap &>/dev/null; then
    echo "Error: snap is not installed" >&2
    exit 1
fi

echo "Installed snap packages:"
echo ""

snap list 2>/dev/null || echo "(none)"

echo ""
exit 0

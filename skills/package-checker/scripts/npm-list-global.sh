#!/usr/bin/env bash
# package-checker: npm-list-global probe
# Lists globally installed npm packages.
# Read-only, no mutations.

set -euo pipefail

if ! command -v npm &>/dev/null; then
    echo "Error: npm is not installed or not in PATH" >&2
    exit 1
fi

npm_ver=$(npm --version 2>/dev/null || echo "?")
node_ver=$(node --version 2>/dev/null || echo "?")
echo "npm: v${npm_ver} | node: ${node_ver}"
echo ""

echo "Globally installed npm packages:"
echo ""

npm list -g --depth=0 2>/dev/null | tail -n +2 || echo "(none)"

echo ""
exit 0

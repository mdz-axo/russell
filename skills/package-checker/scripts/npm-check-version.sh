#!/usr/bin/env bash
# package-checker: npm-check-version probe
# Checks if an npm package is installed globally and reports its version.
# Read-only, no mutations.

set -euo pipefail

PACKAGE="${1:-}"

if [[ -z "$PACKAGE" ]]; then
    echo "Usage: npm-check-version.sh <package-name>" >&2
    echo "Example: npm-check-version.sh cline" >&2
    exit 1
fi

# Validate package name — npm rejects special characters
if [[ "$PACKAGE" =~ [~\'\!\(\)\*] ]]; then
    echo "Error: Package name '$PACKAGE' contains invalid characters." >&2
    echo "npm package names cannot contain: ~ ' ! ( ) *" >&2
    echo "Did you mean '${PACKAGE%%[~\'\!\(\)\*]*}'?" >&2
    exit 1
fi

# Check if npm is available
if ! command -v npm &>/dev/null; then
    echo "Error: npm is not installed or not in PATH" >&2
    echo "Install it with: sudo apt install -y npm" >&2
    exit 1
fi

# Check npm and node versions
npm_ver=$(npm --version 2>/dev/null || echo "?")
node_ver=$(node --version 2>/dev/null || echo "?")
echo "npm: v${npm_ver} | node: ${node_ver}"
echo ""

# Check global packages
installed=$(npm list -g --depth=0 2>/dev/null | grep -E "^[├└]─+ ${PACKAGE}@" || true)

if [[ -z "$installed" ]]; then
    echo "Not installed (global): ${PACKAGE}"
    echo ""
    # Try to look up the package in the registry
    registry_info=$(npm view "$PACKAGE" version 2>/dev/null || true)
    if [[ -n "$registry_info" ]]; then
        echo "Registry: ${PACKAGE}@${registry_info} is available"
        echo "Install with: sudo npm install -g ${PACKAGE}"
    else
        echo "Registry: '${PACKAGE}' not found in npm registry"
        echo "Check the package name at https://www.npmjs.com/search?q=${PACKAGE}"
    fi
    exit 0
fi

# Parse installed version
version=$(echo "$installed" | sed -E 's/.*@([0-9][^ ]*).*/\1/')
echo "Installed (global): ${PACKAGE}@${version}"
echo ""

# Check if newer version available
latest=$(npm view "$PACKAGE" version 2>/dev/null || true)
if [[ -n "$latest" ]] && [[ "$version" != "$latest" ]]; then
    echo "Update available: ${version} → ${latest}"
    echo "Update with: sudo npm install -g ${PACKAGE}@latest"
elif [[ -n "$latest" ]]; then
    echo "✓ Up to date (${version})"
fi

exit 0

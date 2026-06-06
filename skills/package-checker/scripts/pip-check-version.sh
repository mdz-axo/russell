#!/usr/bin/env bash
# package-checker: pip-check-version probe
# Checks if a pip package is installed and reports its version.
# Read-only, no mutations.

set -euo pipefail

PACKAGE="${1:-}"

if [[ -z "$PACKAGE" ]]; then
    echo "Usage: pip-check-version.sh <package-name>" >&2
    echo "Example: pip-check-version.sh requests" >&2
    exit 1
fi

# Try pip3 first, fall back to pip
PIP_CMD=""
if command -v pip3 &>/dev/null; then
    PIP_CMD="pip3"
elif command -v pip &>/dev/null; then
    PIP_CMD="pip"
else
    echo "Error: pip/pip3 is not installed" >&2
    exit 1
fi

pip_ver=$($PIP_CMD --version 2>/dev/null || echo "?")
echo "Using: $pip_ver"
echo ""

# Check if package is installed
installed=$($PIP_CMD show "$PACKAGE" 2>/dev/null || true)

if [[ -z "$installed" ]]; then
    echo "Not installed (pip): ${PACKAGE}"
    echo ""
    # Check PyPI
    pypi_version=$($PIP_CMD index versions "$PACKAGE" 2>/dev/null | head -1 || true)
    if [[ -n "$pypi_version" ]]; then
        echo "Available on PyPI: $pypi_version"
        echo "Install with: $PIP_CMD install --user ${PACKAGE}"
    fi
    exit 0
fi

# Parse pip show output
version=$(echo "$installed" | grep "^Version:" | awk '{print $2}')
location=$(echo "$installed" | grep "^Location:" | awk '{print $2}')

echo "Installed (pip): ${PACKAGE}"
echo "  Version:   ${version}"
echo "  Location:  ${location}"
echo ""

# Check for newer version
latest=$($PIP_CMD index versions "$PACKAGE" 2>/dev/null | grep "Available versions:" | awk '{print $3}' | tr -d ',' | head -1 || true)
if [[ -n "$latest" ]] && [[ "$version" != "$latest" ]]; then
    echo "Update available: ${version} → ${latest}"
    echo "Update with: $PIP_CMD install --user --upgrade ${PACKAGE}"
elif [[ -z "$latest" ]]; then
    echo "✓ Installed (${version})"
else
    echo "✓ Up to date (${version})"
fi

exit 0

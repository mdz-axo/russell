#!/usr/bin/env bash
# package-checker: detect-managers probe
# Detects which package managers are available on the system.
# Read-only, no mutations.

set -euo pipefail

echo "Package manager detection"
echo "========================="
echo ""

# apt/dpkg
if command -v dpkg &>/dev/null && command -v apt &>/dev/null; then
    apt_ver=$(apt --version 2>/dev/null | head -1 || echo "unknown")
    echo "✓ apt/dpkg: $apt_ver"
else
    echo "✗ apt/dpkg: not found"
fi

# npm
if command -v npm &>/dev/null; then
    npm_ver=$(npm --version 2>/dev/null || echo "unknown")
    node_ver=$(node --version 2>/dev/null || echo "unknown")
    node_path=$(command -v node 2>/dev/null || echo "not found")
    npm_path=$(command -v npm 2>/dev/null || echo "not found")
    echo "✓ npm: v${npm_ver} (node ${node_ver} at ${node_path}, npm at ${npm_path})"
else
    echo "✗ npm: not found (node may exist without npm — install 'npm' via apt)"
fi

# snap
if command -v snap &>/dev/null; then
    snap_ver=$(snap version 2>/dev/null | head -1 || echo "unknown")
    echo "✓ snap: $snap_ver"
else
    echo "✗ snap: not found"
fi

# pip3
if command -v pip3 &>/dev/null; then
    pip_ver=$(pip3 --version 2>/dev/null || echo "unknown")
    echo "✓ pip3: $pip_ver"
elif command -v pip &>/dev/null; then
    pip_ver=$(pip --version 2>/dev/null || echo "unknown")
    echo "✓ pip: $pip_ver"
else
    echo "✗ pip/pip3: not found"
fi

# cargo
if command -v cargo &>/dev/null; then
    cargo_ver=$(cargo --version 2>/dev/null || echo "unknown")
    echo "✓ cargo: $cargo_ver"
else
    echo "✗ cargo: not found"
fi

echo ""
echo "Use manager-specific probes for details:"
echo "  npm-check-version <package>"
echo "  snap-check-version <package>"
echo "  pip-check-version <package>"
echo ""
exit 0

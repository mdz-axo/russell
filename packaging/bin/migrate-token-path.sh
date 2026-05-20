#!/bin/bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# Migrate Russell MCP token from kask/ to hkask/ path
# Requires hKask keystore coordination

set -euo pipefail

KASK_TOKEN_PATH="${HOME}/.local/state/kask/mcp-token.json"
HKASK_TOKEN_PATH="${HOME}/.local/state/hkask/mcp-token.json"

echo "Russell Token Path Migration: kask → hkask"
echo "============================================"

# Check if old token exists
if [ ! -f "${KASK_TOKEN_PATH}" ]; then
    echo "Old token path not found: ${KASK_TOKEN_PATH}"
    echo "Token may already be migrated or never configured."
    exit 0
fi

# Check if new token already exists
if [ -f "${HKASK_TOKEN_PATH}" ]; then
    echo "New token path already exists: ${HKASK_TOKEN_PATH}"
    echo "Migration complete."
    
    # Remove old token if new one exists
    echo "Removing old token file..."
    rm -f "${KASK_TOKEN_PATH}"
    exit 0
fi

# Create hkask directory
mkdir -p "$(dirname "${HKASK_TOKEN_PATH}")"

# Migrate token file
echo "Migrating token file..."
cp "${KASK_TOKEN_PATH}" "${HKASK_TOKEN_PATH}"
chmod 600 "${HKASK_TOKEN_PATH}"

# Verify migration
if [ -f "${HKASK_TOKEN_PATH}" ]; then
    echo "Token migrated successfully."
    echo "  Source: ${KASK_TOKEN_PATH}"
    echo "  Target: ${HKASK_TOKEN_PATH}"
    
    # Remove old token
    rm -f "${KASK_TOKEN_PATH}"
    echo "  Old token removed."
    echo ""
    echo "Next steps:"
    echo "  1. Verify token: cat ${HKASK_TOKEN_PATH} | python3 -m json.tool"
    echo "  2. Test MCP connection: russell mcp-tools"
    echo "  3. Register new path with hKask keystore (if applicable)"
else
    echo "ERROR: Migration failed. Target file not created."
    exit 1
fi
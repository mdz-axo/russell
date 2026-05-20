#!/bin/bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# Migrate Russell environment variables from KASK_* to HKASK_*
# Idempotent: safe to run multiple times
# Creates backup at .env.kask.bak

set -euo pipefail

ENV_FILE="${HOME}/.config/harness/russell.env"
BACKUP_FILE="${HOME}/.config/harness/russell.env.kask.bak"

echo "Russell Environment Migration: KASK_* → HKASK_*"
echo "================================================"

# Check if env file exists
if [ ! -f "${ENV_FILE}" ]; then
    echo "Environment file not found: ${ENV_FILE}"
    echo "No migration needed."
    exit 0
fi

# Check if already migrated
if grep -q "^HKASK_" "${ENV_FILE}" 2>/dev/null; then
    echo "Environment file already contains HKASK_* variables."
    echo "Migration appears complete."
    
    # Clean up old backup if it exists
    if [ -f "${BACKUP_FILE}" ]; then
        echo "Removing old backup: ${BACKUP_FILE}"
        rm -f "${BACKUP_FILE}"
    fi
    exit 0
fi

# Check if KASK_* variables exist
if ! grep -q "^KASK_" "${ENV_FILE}" 2>/dev/null; then
    echo "No KASK_* variables found in environment file."
    echo "Migration not needed."
    exit 0
fi

# Create backup
echo "Creating backup: ${BACKUP_FILE}"
cp "${ENV_FILE}" "${BACKUP_FILE}"
chmod 600 "${BACKUP_FILE}"

# Migrate environment variables
echo "Migrating environment variables..."

# Create temp file for atomic update
TEMP_FILE=$(mktemp)

# Perform migration
sed -e 's/^KASK_MCP_ENDPOINT=/HKASK_MCP_ENDPOINT=/g' \
    -e 's/^KASK_MCP_TOKEN=/HKASK_MCP_TOKEN=/g' \
    -e 's/^KASK_MCP_TOOL_TTL_SECS=/HKASK_MCP_TOOL_TTL_SECS=/g' \
    -e 's/^KASK_MCP_TIMEOUT_SECS=/HKASK_MCP_TIMEOUT_SECS=/g' \
    -e 's/# KASK_/# HKASK_/g' \
    -e 's/ KASK_ / HKASK_ /g' \
    "${ENV_FILE}" > "${TEMP_FILE}"

# Atomic replacement
mv "${TEMP_FILE}" "${ENV_FILE}"
chmod 600 "${ENV_FILE}"

echo "Migration complete!"
echo ""
echo "Variables migrated:"
echo "  KASK_MCP_ENDPOINT     → HKASK_MCP_ENDPOINT"
echo "  KASK_MCP_TOKEN        → HKASK_MCP_TOKEN"
echo "  KASK_MCP_TOOL_TTL_SECS → HKASK_MCP_TOOL_TTL_SECS"
echo "  KASK_MCP_TIMEOUT_SECS → HKASK_MCP_TIMEOUT_SECS"
echo ""
echo "Backup saved to: ${BACKUP_FILE}"
echo ""
echo "Next steps:"
echo "  1. Review changes: diff -u ${BACKUP_FILE} ${ENV_FILE}"
echo "  2. Test connection: russell mcp-tools"
echo "  3. Update token path: see packaging/bin/migrate-token-path.sh"

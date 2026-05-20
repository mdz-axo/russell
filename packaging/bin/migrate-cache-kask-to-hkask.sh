#!/bin/bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# Migrate Russell MCP cache from kask-tools.cache.json to hkask-tools.cache.json
# Idempotent: safe to run multiple times

set -euo pipefail

MEMORY_DIR="${HOME}/.local/share/harness/memory"

echo "Russell Cache Migration: kask-tools → hkask-tools"
echo "=================================================="

# Check if memory directory exists
if [ ! -d "${MEMORY_DIR}" ]; then
    echo "Memory directory not found: ${MEMORY_DIR}"
    echo "No migration needed."
    exit 0
fi

KASK_CACHE="${MEMORY_DIR}/kask-tools.cache.json"
HKASK_CACHE="${MEMORY_DIR}/hkask-tools.cache.json"

# Check if kask cache exists
if [ ! -f "${KASK_CACHE}" ]; then
    echo "No kask-tools.cache.json found. Migration complete or never ran."
    exit 0
fi

# Check if hkask cache already exists
if [ -f "${HKASK_CACHE}" ]; then
    echo "hkask-tools.cache.json already exists. Migration complete."
    echo "Removing stale kask cache..."
    rm -f "${KASK_CACHE}"
    exit 0
fi

# Migrate cache file
echo "Migrating cache file..."
cp "${KASK_CACHE}" "${HKASK_CACHE}"

# Verify migration
if [ -f "${HKASK_CACHE}" ]; then
    echo "Cache migrated successfully."
    echo "  Source: ${KASK_CACHE}"
    echo "  Target: ${HKASK_CACHE}"
    
    # Invalidate old cache (force refresh on next boot)
    rm -f "${KASK_CACHE}"
    echo "  Old cache invalidated (removed)."
    echo ""
    echo "Next boot: hKask tool registry will refresh from MCP endpoint."
else
    echo "ERROR: Migration failed. Target file not created."
    exit 1
fi

#!/bin/bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# Migrate Russell evidence from kask/ to hkask/ paths
# Idempotent: safe to run multiple times

set -euo pipefail

EVIDENCE_DIR="${HOME}/.local/share/harness/evidence"
JOURNAL="${HOME}/.local/share/harness/journal.db"

echo "Russell Evidence Migration: kask → hkask"
echo "=========================================="

# Check if evidence directory exists
if [ ! -d "${EVIDENCE_DIR}" ]; then
    echo "Evidence directory not found: ${EVIDENCE_DIR}"
    echo "No migration needed."
    exit 0
fi

# Check if kask directory exists
KASK_DIR="${EVIDENCE_DIR}/kask"
HKASK_DIR="${EVIDENCE_DIR}/hkask"

if [ ! -d "${KASK_DIR}" ]; then
    echo "No kask evidence directory found. Migration complete or never ran."
    exit 0
fi

# Count existing bundles
BUNDLE_COUNT=$(find "${KASK_DIR}" -mindepth 2 -maxdepth 2 -type d 2>/dev/null | wc -l)

if [ "${BUNDLE_COUNT}" -eq 0 ]; then
    echo "No kask evidence bundles found. Migration complete."
    exit 0
fi

echo "Found ${BUNDLE_COUNT} kask evidence bundle(s) to migrate."

# Create hkask directory if it doesn't exist
mkdir -p "${HKASK_DIR}"

# Migrate each tool's evidence
MIGRATED=0
for tool_dir in "${KASK_DIR}"/*/; do
    if [ -d "${tool_dir}" ]; then
        tool_name=$(basename "${tool_dir}")
        target_dir="${HKASK_DIR}/${tool_name}"
        
        # Create target directory
        mkdir -p "${target_dir}"
        
        # Move evidence bundles (preserve timestamps)
        for bundle in "${tool_dir}"/*/; do
            if [ -d "${bundle}" ]; then
                bundle_name=$(basename "${bundle}")
                mv "${bundle}" "${target_dir}/"
                MIGRATED=$((MIGRATED + 1))
            fi
        done
        
        echo "  Migrated tool: ${tool_name}"
    fi
done

# Remove empty kask directory structure
if [ -d "${KASK_DIR}" ]; then
    find "${KASK_DIR}" -type d -empty -delete 2>/dev/null || true
    rmdir "${KASK_DIR}" 2>/dev/null || true
fi

echo ""
echo "Migration complete: ${MIGRATED} bundle(s) moved from kask/ to hkask/"
echo ""
echo "Next steps:"
echo "  1. Verify evidence: ls -la ${HKASK_DIR}/*/"
echo "  2. Journal migration event: russell journal --note 'evidence migration kask→hkask'"

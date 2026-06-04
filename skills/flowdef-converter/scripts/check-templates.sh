#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# flowdef-converter: check-templates probe
# Verifies that all template_ref values in the manifest have corresponding
# .j2 template files in the hKask registry.

set -euo pipefail

MANIFEST_PATH="${1:-}"
HKASK_REGISTRY="${HKASK_REGISTRY_DIR:-$HOME/Clones/hKask/registry}"

if [[ -z "$MANIFEST_PATH" ]]; then
    echo "Usage: check-templates.sh <path-to-flowdef-manifest>" >&2
    exit 1
fi

if [[ ! -f "$MANIFEST_PATH" ]]; then
    echo "not_found: ${MANIFEST_PATH}" >&2
    exit 1
fi

SKILL_ID=$(grep -A3 "^manifest:" "$MANIFEST_PATH" | grep "id:" | head -1 | sed 's/.*id: *//' | tr -d ' ')

if [[ -z "$SKILL_ID" ]]; then
    echo "could not extract skill id from manifest" >&2
    exit 1
fi

echo "skill_id: ${SKILL_ID}"
echo "registry: ${HKASK_REGISTRY}"
echo "---"

# Extract all template_refs (skip nulls)
TEMPLATE_REFS=$(grep "template_ref:" "$MANIFEST_PATH" | sed 's/.*template_ref: *//' | grep -v "^null$" | sort -u)

if [[ -z "$TEMPLATE_REFS" ]]; then
    echo "no template references found"
    exit 0
fi

FOUND=0
MISSING=0

for ref in $TEMPLATE_REFS; do
    # Template refs are like "grill-me/grill-me-round"
    # Files are at HKASK_REGISTRY/templates/<dir>/<file>.j2
    TEMPLATE_DIR=$(echo "$ref" | cut -d'/' -f1)
    TEMPLATE_NAME=$(echo "$ref" | cut -d'/' -f2-)

    SEARCH_PATHS=(
        "${HKASK_REGISTRY}/templates/${TEMPLATE_DIR}/${TEMPLATE_NAME}.j2"
        "${HKASK_REGISTRY}/templates/${TEMPLATE_NAME}.j2"
        "${HKASK_REGISTRY}/templates/${ref}.j2"
    )

    found_path=""
    for sp in "${SEARCH_PATHS[@]}"; do
        if [[ -f "$sp" ]]; then
            found_path="$sp"
            break
        fi
    done

    if [[ -n "$found_path" ]]; then
        echo "  ✓ ${ref} → ${found_path}"
        ((FOUND++)) || true
    else
        echo "  ✗ ${ref} → NOT FOUND"
        ((MISSING++)) || true
    fi
done

echo ""
echo "found: ${FOUND}"
echo "missing: ${MISSING}"

if [[ $MISSING -gt 0 ]]; then
    exit 1
fi

exit 0

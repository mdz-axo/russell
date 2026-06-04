#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# flowdef-converter: list-convertible probe
# Lists all FlowDef manifests in the hKask registry that can be converted.

set -euo pipefail

HKASK_REGISTRY="${HKASK_REGISTRY_DIR:-$HOME/Clones/hKask/registry}"

if [[ ! -d "$HKASK_REGISTRY/manifests" ]]; then
    echo "hKask registry not found at: ${HKASK_REGISTRY}"
    exit 1
fi

echo "registry: ${HKASK_REGISTRY}/manifests"
echo "---"

COUNT=0
CONVERTIBLE=0

for manifest in "$HKASK_REGISTRY"/manifests/*.yaml; do
    if [[ ! -f "$manifest" ]]; then
        continue
    fi

    ((COUNT++)) || true
    BASENAME=$(basename "$manifest" .yaml)

    # Check if this is a FlowDef (has "steps:" with ordinals)
    HAS_STEPS=$(grep -c '^  - ordinal:' "$manifest" 2>/dev/null || true)
    HAS_STEPS=${HAS_STEPS:-0}
    [[ "$HAS_STEPS" -lt 1 ]] && HAS_STEPS=0
    FUNC_ROLE=$(grep 'functional_role:' "$manifest" 2>/dev/null | head -1 | sed 's/.*functional_role: *//' | tr -d ' ' || true)
    DESCRIPTION=$(grep 'description:' "$manifest" 2>/dev/null | head -1 | sed 's/.*description: *//' | head -c 60 || true)

    if [[ "$HAS_STEPS" -gt 0 ]]; then
        KIND="flowdef"
        STATUS="convertible"
        ((CONVERTIBLE++)) || true
    else
        KIND="other"
        STATUS="skip"
    fi

    printf "  %-35s %-10s %-12s %s\n" "$BASENAME" "$KIND" "$STATUS" "$DESCRIPTION"
done

echo ""
echo "total: ${COUNT} manifests, ${CONVERTIBLE} convertible (FlowDef with steps)"

exit 0

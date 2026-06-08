#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# flowdef-converter: analyze probe
# Reads a FlowDef manifest and reports its structure, steps, and templates.

set -euo pipefail

MANIFEST_PATH="${1:-}"
FLOWDEF_REGISTRY="${FLOWDEF_REGISTRY_DIR:-}"

if [[ -z "$MANIFEST_PATH" ]]; then
    echo "Usage: analyze.sh <path-to-flowdef-manifest>" >&2
    exit 1
fi

if [[ ! -f "$MANIFEST_PATH" ]]; then
    echo "not_found: ${MANIFEST_PATH}" >&2
    exit 1
fi

echo "manifest: ${MANIFEST_PATH}"
echo "---"

# Extract manifest metadata (use || true to prevent set -e failures when grep finds nothing)
SKILL_ID=$(grep -A3 "^manifest:" "$MANIFEST_PATH" 2>/dev/null | grep "id:" | head -1 | sed 's/.*id: *//' | tr -d ' ' || true)
SKILL_NAME=$(grep -A3 "^manifest:" "$MANIFEST_PATH" 2>/dev/null | grep "name:" | head -1 | sed 's/.*name: *//' | tr -d ' ' || true)
SKILL_VERSION=$(grep -A5 "^manifest:" "$MANIFEST_PATH" 2>/dev/null | grep "version:" | head -1 | sed 's/.*version: *//' | tr -d ' ' || true)
FUNC_ROLE=$(grep -A5 "^manifest:" "$MANIFEST_PATH" 2>/dev/null | grep "functional_role:" | head -1 | sed 's/.*functional_role: *//' | tr -d ' ' || true)

echo "id: ${SKILL_ID:-unknown}"
echo "name: ${SKILL_NAME:-unknown}"
echo "version: ${SKILL_VERSION:-unknown}"
echo "functional_role: ${FUNC_ROLE:-unknown}"
echo ""

# Extract inputs
echo "inputs:"
grep -A100 "^inputs:" "$MANIFEST_PATH" | grep -E "^  - name:" | sed 's/.*name: */  /' || echo "  (none)"
echo ""

# Extract levels (question taxonomy, if present)
echo "levels:"
grep -A100 "^levels:" "$MANIFEST_PATH" | grep -E "^  - number:" | sed 's/.*number: */  level /' || echo "  (none)"
echo ""

# Extract steps — parse the steps: section only
STEPS_START=$(grep -n "^steps:" "$MANIFEST_PATH" 2>/dev/null | head -1 | cut -d: -f1 || echo "0")
if [[ -n "$STEPS_START" ]] && [[ "$STEPS_START" -gt 0 ]]; then
    echo "steps:"
    STEPS_SECTION=$(tail -n +"$((STEPS_START + 1))" "$MANIFEST_PATH" | sed '/^[a-z]/Q' 2>/dev/null || true)
    ORDINAL=0
    while IFS= read -r line; do
        if echo "$line" | grep -q "ordinal:"; then
            ORDINAL=$(echo "$line" | sed 's/.*ordinal: *//')
        fi
        if echo "$line" | grep -q "^    action:"; then
            ACTION=$(echo "$line" | sed 's/.*action: *//')
            TREF=$(echo "$STEPS_SECTION" | grep -A15 "ordinal: ${ORDINAL}" | grep "template_ref:" | head -1 | sed 's/.*template_ref: *//' || echo "null")
            MTIER=$(echo "$STEPS_SECTION" | grep -A15 "ordinal: ${ORDINAL}" | grep "model_tier:" | head -1 | sed 's/.*model_tier: *//' || echo "null")

            # Classify the step
            if [[ "$ACTION" == "feedback" ]]; then
                CLASSIFICATION="feedback → DROP"
            elif [[ "$TREF" != "null" && -n "$TREF" ]]; then
                CLASSIFICATION="cognitive → KNOWLEDGE.md"
            elif [[ "$ACTION" == "validate" ]]; then
                CLASSIFICATION="deterministic → probe script"
            elif [[ "$TREF" == "null" || -z "$TREF" ]] && [[ "$MTIER" == "null" || -z "$MTIER" ]]; then
                CLASSIFICATION="deterministic → probe/intervention"
            else
                CLASSIFICATION="cognitive → KNOWLEDGE.md"
            fi

            echo "  step ${ORDINAL}: action=${ACTION} template=${TREF} model=${MTIER} → ${CLASSIFICATION}"
        fi
    done <<< "$STEPS_SECTION"
else
    echo "steps: (no steps section found)"
fi

echo ""

# Extract escalation rules
echo "escalation:"
if grep -q "^escalation:" "$MANIFEST_PATH"; then
    grep -A10 "^escalation:" "$MANIFEST_PATH" | head -6 | sed 's/^/  /'
else
    echo "  (none)"
fi

echo ""
echo "template_refs:"
grep "template_ref:" "$MANIFEST_PATH" 2>/dev/null | sed 's/.*template_ref: */  /' | sort -u | grep -v "^null$" || echo "  (none)"

exit 0

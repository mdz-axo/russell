#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# grill-me: health probe
# Verifies that the grill-me skill files are intact and readable.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SKILL_DIR="$(dirname "$SCRIPT_DIR")"

errors=0

# Check manifest
if [[ ! -f "${SKILL_DIR}/manifest.yaml" ]]; then
    echo "missing manifest.yaml"
    ((errors++)) || true
fi

# Check KNOWLEDGE.md
if [[ ! -f "${SKILL_DIR}/KNOWLEDGE.md" ]]; then
    echo "missing KNOWLEDGE.md"
    ((errors++)) || true
fi

# Check scripts directory
if [[ ! -d "${SKILL_DIR}/scripts" ]]; then
    echo "missing scripts directory"
    ((errors++)) || true
fi

# Check this script is executable (self-test)
if [[ ! -x "${BASH_SOURCE[0]}" ]]; then
    echo "health.sh not executable"
    ((errors++)) || true
fi

# Check check-llm.sh exists
if [[ ! -f "${SKILL_DIR}/scripts/check-llm.sh" ]]; then
    echo "missing scripts/check-llm.sh"
    ((errors++)) || true
fi

# Check session-stats.sh exists
if [[ ! -f "${SKILL_DIR}/scripts/session-stats.sh" ]]; then
    echo "missing scripts/session-stats.sh"
    ((errors++)) || true
fi

# Check manifest has required fields
if [[ -f "${SKILL_DIR}/manifest.yaml" ]]; then
    for field in "id:" "version:" "symptoms:" "probes:"; do
        if ! grep -q "$field" "${SKILL_DIR}/manifest.yaml" 2>/dev/null; then
            echo "manifest missing field: ${field%:}"
            ((errors++)) || true
        fi
    done
fi

if [[ $errors -eq 0 ]]; then
    echo "ok"
else
    echo "errors: ${errors}"
    exit 1
fi

exit 0

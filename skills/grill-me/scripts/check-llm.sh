#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# grill-me: check-llm probe
# Verifies that the local LLM backend (Okapi) is reachable and has at least
# one model loaded. Grilling requires LLM inference for deep evaluation.
# Uses the same endpoint pattern as okapi-watcher.

set -euo pipefail

OKAPI_HOST="${OKAPI_HOST:-http://127.0.0.1:11435}"

response=$(curl -s --max-time 5 "${OKAPI_HOST}/api/tags" 2>/dev/null) || true

if [[ -z "$response" ]]; then
    echo "unreachable"
    exit 1
fi

# Count loaded models
model_count=$(echo "$response" | grep -o '"name"' | wc -l 2>/dev/null || echo "0")

if [[ "$model_count" -eq 0 ]]; then
    echo "no_models"
    exit 1
fi

echo "ok ${model_count}"
exit 0

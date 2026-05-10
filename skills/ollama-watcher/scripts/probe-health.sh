#!/bin/bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# ollama-watcher: probe-ollama-health
# Checks whether Ollama is reachable at the configured endpoint.
# Default: http://127.0.0.1:11434
# Prints "ok <model_count>" on success, "unreachable" on failure.

OLLAMA_HOST="${OLLAMA_HOST:-http://127.0.0.1:11434}"

response=$(curl -s --max-time 5 "${OLLAMA_HOST}/api/tags" 2>/dev/null)
if [ $? -ne 0 ] || [ -z "$response" ]; then
    echo "unreachable"
    exit 1
fi

# Count loaded models
model_count=$(echo "$response" | grep -o '"name"' | wc -l)
echo "ok ${model_count}"
exit 0

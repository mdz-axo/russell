#!/bin/bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# ollama-watcher: probe-ollama-models
# Lists loaded models with their sizes.
# Prints one line per model: "<name> <size_gb>"

OLLAMA_HOST="${OLLAMA_HOST:-http://127.0.0.1:11434}"

response=$(curl -s --max-time 5 "${OLLAMA_HOST}/api/tags" 2>/dev/null)
if [ $? -ne 0 ] || [ -z "$response" ]; then
    echo "unreachable"
    exit 1
fi

# Extract model names and sizes using grep and sed (no jq dependency)
echo "$response" | grep -E '"(name|size)"' | paste - - | \
    sed -E 's/.*"name": *"([^"]+)".*"size": *([0-9]+).*/\1 \2/' | \
    while read model size_bytes; do
        size_gb=$(echo "scale=1; $size_bytes / 1073741824" | bc 2>/dev/null || echo "?")
        echo "$model $size_gb"
    done
exit 0

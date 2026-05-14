#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# Probe: Check kask service availability (surface MCP + qdrant)

set -euo pipefail

# Check kask-surface MCP endpoint
KASK_MCP_ENDPOINT="${KASK_MCP_ENDPOINT:-http://127.0.0.1:9500/mcp}"
KASK_STATUS="unreachable"
KASK_LATENCY_MS="null"

if command -v curl &>/dev/null; then
    START_MS=$(date +%s%3N)
    if curl -sf --max-time 5 -X POST \
        -H "Content-Type: application/json" \
        -H "Authorization: Bearer ${KASK_MCP_TOKEN:-}" \
        -d '{"jsonrpc":"2.0","id":1,"method":"ping","params":{}}' \
        "$KASK_MCP_ENDPOINT" >/dev/null 2>&1; then
        KASK_STATUS="reachable"
        END_MS=$(date +%s%3N)
        KASK_LATENCY_MS=$((END_MS - START_MS))
    fi
fi

# Check kask-qdrant container
QDRANT_STATUS="not_found"
if command -v podman &>/dev/null; then
    if podman ps --format '{{.Names}}' 2>/dev/null | grep -qi kask; then
        QDRANT_STATUS="running"
    elif podman ps -a --format '{{.Names}}' 2>/dev/null | grep -qi kask; then
        QDRANT_STATUS="stopped"
    fi
fi

# Check kask-surface process
SURFACE_PID="null"
if pgrep -f "kask-surface|kask_surface" >/dev/null 2>&1; then
    SURFACE_PID=$(pgrep -f "kask-surface|kask_surface" | head -1)
fi

# Output as JSON for easy parsing
cat <<EOF
{
  "kask_mcp_endpoint": "$KASK_MCP_ENDPOINT",
  "kask_mcp_status": "$KASK_STATUS",
  "kask_mcp_latency_ms": $KASK_LATENCY_MS,
  "kask_qdrant_container": "$QDRANT_STATUS",
  "kask_surface_pid": $SURFACE_PID
}
EOF

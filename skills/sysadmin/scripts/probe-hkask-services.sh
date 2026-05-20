#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# Probe: Check hkask service availability (surface MCP + qdrant)

set -euo pipefail

# Check hkask-surface MCP endpoint
HKASK_MCP_ENDPOINT="${HKASK_MCP_ENDPOINT:-http://127.0.0.1:9500/mcp}"
HKASK_STATUS="unreachable"
HKASK_LATENCY_MS="null"

if command -v curl &>/dev/null; then
    START_MS=$(date +%s%3N)
    if curl -sf --max-time 5 -X POST \
        -H "Content-Type: application/json" \
        -H "Authorization: Bearer ${HKASK_MCP_TOKEN:-}" \
        -d '{"jsonrpc":"2.0","id":1,"method":"ping","params":{}}' \
        "$HKASK_MCP_ENDPOINT" >/dev/null 2>&1; then
        HKASK_STATUS="reachable"
        END_MS=$(date +%s%3N)
        HKASK_LATENCY_MS=$((END_MS - START_MS))
    fi
fi

# Check hkask-qdrant container
QDRANT_STATUS="not_found"
if command -v podman &>/dev/null; then
    if podman ps --format '{{.Names}}' 2>/dev/null | grep -qi hkask; then
        QDRANT_STATUS="running"
    elif podman ps -a --format '{{.Names}}' 2>/dev/null | grep -qi hkask; then
        QDRANT_STATUS="stopped"
    fi
fi

# Check hkask-surface process
SURFACE_PID="null"
if pgrep -f "hkask-surface|hkask_surface" >/dev/null 2>&1; then
    SURFACE_PID=$(pgrep -f "hkask-surface|hkask_surface" | head -1)
fi

# Output as JSON for easy parsing
cat <<EOF
{
  "hkask_mcp_endpoint": "$HKASK_MCP_ENDPOINT",
  "hkask_mcp_status": "$HKASK_STATUS",
  "hkask_mcp_latency_ms": $HKASK_LATENCY_MS,
  "hkask_qdrant_container": "$QDRANT_STATUS",
  "hkask_surface_pid": $SURFACE_PID
}
EOF

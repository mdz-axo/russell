#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# Probe: Check remote service availability (MCP endpoints, containers)

set -euo pipefail

# Check remote MCP endpoint
REMOTE_MCP_ENDPOINT="${REMOTE_MCP_ENDPOINT:-http://127.0.0.1:9500/mcp}"
MCP_STATUS="unreachable"
MCP_LATENCY_MS="null"

if command -v curl &>/dev/null; then
    START_MS=$(date +%s%3N)
    if curl -sf --max-time 5 -X POST \
        -H "Content-Type: application/json" \
        -H "Authorization: Bearer ${REMOTE_MCP_TOKEN:-}" \
        -d '{"jsonrpc":"2.0","id":1,"method":"ping","params":{}}' \
        "$REMOTE_MCP_ENDPOINT" >/dev/null 2>&1; then
        MCP_STATUS="reachable"
        END_MS=$(date +%s%3N)
        MCP_LATENCY_MS=$((END_MS - START_MS))
    fi
fi

# Check remote container services
CONTAINER_STATUS="not_found"
if command -v podman &>/dev/null; then
    # Check for any relevant service containers
    CONTAINER_COUNT=$(podman ps --format '{{.Names}}' 2>/dev/null | wc -l || echo "0")
    if [[ "$CONTAINER_COUNT" -gt 0 ]]; then
        CONTAINER_STATUS="running"
    fi
fi

# Check for surface process
SURFACE_PID="null"
if pgrep -f "remote_surface|remote-surface" >/dev/null 2>&1; then
    SURFACE_PID=$(pgrep -f "remote_surface|remote-surface" | head -1)
fi

# Output as JSON for easy parsing
cat <<EOF
{
  "remote_mcp_endpoint": "$REMOTE_MCP_ENDPOINT",
  "remote_mcp_status": "$MCP_STATUS",
  "remote_mcp_latency_ms": $MCP_LATENCY_MS,
  "remote_containers": "$CONTAINER_STATUS",
  "remote_surface_pid": $SURFACE_PID
}
EOF

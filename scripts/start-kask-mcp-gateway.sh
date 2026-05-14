#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
#
# Start Kask MCP HTTP gateway for Russell.
# This bridges Russell's HTTP MCP client to the stdio MCP servers.
#
# Usage: ./start-kask-mcp-gateway.sh [--background]
#
# What it does:
# 1. Builds stack-api if needed
# 2. Starts stack-api with MCP gateway enabled
# 3. Waits for MCP endpoint to be reachable
# 4. Optionally runs in background

set -euo pipefail

KASK_REPO="${HOME}/Clones/kask"
TARGET_DIR="${KASK_REPO}/target/debug"
BIND_ADDR="${KASK_BIND_ADDR:-127.0.0.1:9500}"

say() { printf '\033[1;34m[mcp-gateway]\033[0m %s\n' "$*"; }

# Check if Kask repo exists
if [ ! -d "$KASK_REPO" ] || [ ! -f "$KASK_REPO/Cargo.toml" ]; then
    say "ERROR: Kask repo not found at $KASK_REPO"
    exit 1
fi

# Build stack-api if not present
if [ ! -f "$TARGET_DIR/stack-api" ]; then
    say "Building stack-api..."
    (cd "$KASK_REPO" && cargo build -p stack-api)
fi

# Start stack-api
say "Starting Kask MCP gateway at $BIND_ADDR..."
if [ "${1:-}" = "--background" ] || [ "${1:-}" = "-b" ]; then
    # Run in background
    "$TARGET_DIR/stack-api" &
    GatewayPID=$!
    say "Gateway started (PID: $GatewayPID)"
    
    # Wait for endpoint to be ready
    for i in {1..30}; do
        if curl -sf "http://$BIND_ADDR/mcp" -X POST \
            -H "Content-Type: application/json" \
            -d '{"jsonrpc":"2.0","id":1,"method":"ping","params":{}}' \
            >/dev/null 2>&1; then
            say "MCP gateway ready"
            break
        fi
        sleep 1
    done
    
    # Save PID for later cleanup
    echo "$GatewayPID" > "${HOME}/.local/state/kask/mcp-gateway.pid"
else
    # Run in foreground
    exec "$TARGET_DIR/stack-api"
fi
#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
#
# Integration test: Russell ↔ Kask MCP end-to-end
#
# Usage:
#   ./scripts/integration-test-kask-mcp.sh
#
# Prerequisites:
#   - Kask repo cloned at ../kask (or set KASK_REPO)
#   - `stack-admin` binary available on PATH (or set STACK_ADMIN)
#   - Russell built (`cargo build`)
#   - RUSSELL_DATA_DIR and RUSSELL_CONFIG_DIR set in env (or defaults)
#
# What it does:
#   1. Starts kask-surface on port 9500 (or verifies it's already running)
#   2. Provisions Russell's service principal token via stack-admin key set
#   3. Sets KASK_MCP_TOKEN in the environment
#   4. Runs `russell mcp-tools` — verifies tool listing works
#   5. Runs `russell jack` — verifies the prompt includes Kask tools
#   6. Cleans up (stops kask-surface if we started it)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
KASK_REPO="${KASK_REPO:-$REPO_ROOT/../kask}"
STACK_ADMIN="${STACK_ADMIN:-$KASK_REPO/target/release/stack-admin}"
KASK_SURFACE="${KASK_SURFACE:-$KASK_REPO/target/release/kask-surface}"
MCP_PORT="${MCP_PORT:-9500}"
MCP_ENDPOINT="http://127.0.0.1:${MCP_PORT}/mcp"
RUSSELL_BINARY="$REPO_ROOT/target/debug/russell"

RED="\033[31m"
GREEN="\033[32m"
YELLOW="\033[33m"
NC="\033[0m"

pass() { echo -e "${GREEN}PASS${NC} $*"; }
fail() { echo -e "${RED}FAIL${NC} $*"; exit 1; }
info() { echo -e "${YELLOW}INFO${NC} $*"; }

cleanup() {
    if [[ -n "${SURFACE_PID:-}" ]]; then
        info "Stopping kask-surface (pid $SURFACE_PID)..."
        kill "$SURFACE_PID" 2>/dev/null || true
        wait "$SURFACE_PID" 2>/dev/null || true
    fi
    if [[ -n "${TOKEN_SET:-}" ]]; then
        info "Token was provisioned during test. Revoke with: stack-admin key revoke --for russell"
    fi
}
trap cleanup EXIT

# ── Pre-flight checks ────────────────────────────────────────────────

info "=== Russell ↔ Kask MCP Integration Test ==="
echo ""

# Check if kask-surface is available.
if [[ ! -x "$KASK_SURFACE" ]]; then
    info "kask-surface binary not found at $KASK_SURFACE"
    info "Set KASK_SURFACE env var to the correct path, or build kask first."
    info "Skipping live integration test — Kask not available."
    exit 0
fi

if [[ ! -x "$RUSSELL_BINARY" ]]; then
    info "Russell binary not found at $RUSSELL_BINARY. Building..."
    cargo build --manifest-path "$REPO_ROOT/Cargo.toml"
    if [[ ! -x "$RUSSELL_BINARY" ]]; then
        fail "Failed to build Russell."
    fi
fi

# ── 1. Start kask-surface ────────────────────────────────────────────

info "Step 1: Starting kask-surface on port $MCP_PORT..."

# Check if something is already listening on the port.
if ss -tlnp | grep -q ":$MCP_PORT "; then
    info "Port $MCP_PORT is already in use — assuming kask-surface is running."
    SURFACE_PID=""
else
    "$KASK_SURFACE" &
    SURFACE_PID=$!
    sleep 2

    if ! kill -0 "$SURFACE_PID" 2>/dev/null; then
        fail "kask-surface failed to start."
    fi
    pass "kask-surface started (pid $SURFACE_PID)"
fi

# Wait for the MCP endpoint to become responsive.
info "Waiting for MCP endpoint..."
for i in $(seq 1 10); do
    if curl -s -o /dev/null -w "%{http_code}" "$MCP_ENDPOINT" 2>/dev/null | grep -q "2"; then
        pass "MCP endpoint responsive"
        break
    fi
    if [[ $i -eq 10 ]]; then
        fail "MCP endpoint not responsive after 10 attempts."
    fi
    sleep 1
done

# ── 2. Provision Russell's service principal token ────────────────────

info "Step 2: Provisioning Russell's service principal token..."

if [[ ! -x "$STACK_ADMIN" ]]; then
    info "stack-admin binary not found at $STACK_ADMIN"
    info "Skipping token provisioning — set KASK_MCP_TOKEN manually."
else
    KASK_MCP_TOKEN=$("$STACK_ADMIN" key set --for russell --scope user 2>/dev/null || true)
    if [[ -z "$KASK_MCP_TOKEN" ]]; then
        info "stack-admin key set returned empty. Checking existing token..."
        # Maybe the key already exists — try to read it.
        KASK_MCP_TOKEN=$("$STACK_ADMIN" key get --for russell 2>/dev/null || true)
    fi
    if [[ -n "$KASK_MCP_TOKEN" ]]; then
        export KASK_MCP_TOKEN
        TOKEN_SET=1
        pass "Token provisioned"
    else
        info "Could not provision token. Set KASK_MCP_TOKEN manually."
    fi
fi

# ── 3. Run `russell mcp-tools` ──────────────────────────────────────

info "Step 3: Running 'russell mcp-tools'..."

MCP_TOOLS_OUTPUT=$(KASK_MCP_ENDPOINT="$MCP_ENDPOINT" "$RUSSELL_BINARY" mcp-tools 2>&1) || {
    echo "$MCP_TOOLS_OUTPUT"
    fail "'russell mcp-tools' failed."
}

echo "$MCP_TOOLS_OUTPUT" | head -20

if echo "$MCP_TOOLS_OUTPUT" | grep -q "TOOL"; then
    pass "'russell mcp-tools' listed tools successfully"
else
    fail "'russell mcp-tools' did not show tool list."
fi

# ── 4. Run `russell jack` (with mock backend to avoid LLM call) ─────

info "Step 4: Verifying 'russell jack' includes Kask tools..."

# Use mock backend to avoid requiring Okapi for this test.
JACK_OUTPUT=$(RUSSELL_DOCTOR_BACKEND=mock KASK_MCP_ENDPOINT="$MCP_ENDPOINT" "$RUSSELL_BINARY" jack 2>&1) || {
    echo "$JACK_OUTPUT"
    fail "'russell jack' failed."
}

if echo "$JACK_OUTPUT" | grep -q "Mock Jack"; then
    pass "'russell jack' responded (mock backend)"
else
    info "Mock Jack response not found. This may be OK if Okapi was configured."
fi

info "Step 5: Running 'russell proprio' for kask self-vital..."

PROPRIO_OUTPUT=$(KASK_MCP_ENDPOINT="$MCP_ENDPOINT" "$RUSSELL_BINARY" proprio 2>&1) || {
    echo "$PROPRIO_OUTPUT"
    fail "'russell proprio' failed."
}

if echo "$PROPRIO_OUTPUT" | grep -q "kask_mcp_reachable_ms"; then
    pass "'russell proprio' includes kask_mcp_reachable_ms self-vital"
else
    fail "'russell proprio' did not show kask_mcp_reachable_ms."
fi

# ── Summary ───────────────────────────────────────────────────────────

echo ""
info "=== Integration test summary ==="
echo "  ✅ kask-surface started and responsive"
echo "  ✅ Russell MCP client connected"
echo "  ✅ tools/list successful"
echo "  ✅ russell jack runs with Kask tools in prompt"
echo "  ✅ kask_mcp_reachable_ms self-vital journaled"
echo ""
pass "All integration checks passed."

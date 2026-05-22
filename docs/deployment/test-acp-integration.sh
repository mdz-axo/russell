#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# Russell ACP Integration Test Script

set -euo pipefail

MACAROON_KEY="${RUSSELL_ACP_MACAROON_KEY}"
ACP_SERVER="$HOME/.cargo/bin/russell-acp-server"

echo "=== Russell ACP Integration Tests ==="
echo ""

# Test 1: Capabilities endpoint
echo "[1/4] Testing acp/capabilities..."
RESPONSE=$(echo '{"jsonrpc":"2.0","id":1,"method":"acp/capabilities","params":{}}' | \
  RUSSELL_ACP_MACAROON_KEY="$MACAROON_KEY" timeout 5 "$ACP_SERVER" 2>&1 | grep '^{' || true)

if [ -n "$RESPONSE" ] && echo "$RESPONSE" | grep -q '"skills"'; then
    SKILL_COUNT=$(echo "$RESPONSE" | grep -o '"id"' | wc -l)
    echo "  ✓ Capabilities returned $SKILL_COUNT items"
else
    echo "  ✗ Capabilities test failed"
    exit 1
fi

# Test 2: Skill info
echo "[2/4] Testing acp/skill/info..."
INFO_RESPONSE=$(echo '{"jsonrpc":"2.0","id":2,"method":"acp/skill/info","params":{"skill_id":"web-search"}}' | \
  RUSSELL_ACP_MACAROON_KEY="$MACAROON_KEY" timeout 5 "$ACP_SERVER" 2>&1 | grep '^{' || true)

if echo "$INFO_RESPONSE" | grep -q 'web-search'; then
    echo "  ✓ Skill info returned"
else
    echo "  ✗ Skill info failed"
    echo "  Response: $INFO_RESPONSE"
    exit 1
fi

# Test 3: Probe execution (journal-viewer)
echo "[3/4] Testing acp/probe/run (journal-viewer)..."
PROBE_RESPONSE=$(echo '{"jsonrpc":"2.0","id":3,"method":"acp/probe/run","params":{"skill_id":"journal-viewer","probe_id":"show-host-samples","args":{}}}' | \
  RUSSELL_ACP_MACAROON_KEY="$MACAROON_KEY" timeout 10 "$ACP_SERVER" 2>&1 | grep '^{' || true)

if echo "$PROBE_RESPONSE" | grep -q '"result"'; then
    echo "  ✓ Probe execution completed"
else
    echo "  ✗ Probe execution failed"
    echo "  Response: $PROBE_RESPONSE"
    exit 1
fi

# Test 4: Private skill rejection
echo "[4/4] Testing private skill rejection..."
PRIVATE_RESPONSE=$(echo '{"jsonrpc":"2.0","id":4,"method":"acp/skill/info","params":{"skill_id":"okapi-watcher"}}' | \
  RUSSELL_ACP_MACAROON_KEY="$MACAROON_KEY" timeout 5 "$ACP_SERVER" 2>&1 | grep '^{' || true)

if echo "$PRIVATE_RESPONSE" | grep -q '"error"'; then
    echo "  ✓ Private skill correctly rejected"
else
    echo "  ✗ Private skill should be rejected"
    exit 1
fi

echo ""
echo "=== All Tests Passed ==="
echo ""
echo "Russell ACP server is ready for hKask integration."

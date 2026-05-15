#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
#
# Rotate Russell's Kask MCP token.
#
# This script should be run weekly via systemd timer or cron.
# It rotates the service principal token and updates the token file
# that Russell's FileTokenProvider reads.
#
# Usage: ./rotate-russell-token.sh [--dry-run]
#
# Requirements:
# - stack-admin binary in PATH
# - Access to Kask's keystore
# - Russell service principal already created

set -euo pipefail

DRY_RUN=0
TOKEN_FILE="${HOME}/.local/state/kask/mcp-token.json"
PRINCIPAL="russell"
TTL="168h"  # 7 days

# Parse arguments
for arg in "$@"; do
    case "$arg" in
        --dry-run)
            DRY_RUN=1
            ;;
        -h|--help)
            echo "Usage: $0 [--dry-run]"
            echo ""
            echo "Rotate Russell's Kask MCP token."
            echo ""
            echo "Options:"
            echo "  --dry-run    Show what would be done without rotating"
            echo "  -h, --help   Show this help message"
            exit 0
            ;;
        *)
            echo "Unknown option: $arg" >&2
            exit 1
            ;;
    esac
done

# Check prerequisites
if ! command -v stack-admin &>/dev/null; then
    echo "ERROR: stack-admin not found in PATH" >&2
    echo "Ensure Kask binaries are installed and on PATH" >&2
    exit 1
fi

# Ensure token directory exists
mkdir -p "$(dirname "$TOKEN_FILE")"

if [ "$DRY_RUN" -eq 1 ]; then
    echo "[dry-run] Would rotate token for principal: $PRINCIPAL"
    echo "[dry-run] TTL: $TTL"
    echo "[dry-run] Token file: $TOKEN_FILE"
    echo ""
    
    # Show current token info if file exists
    if [ -f "$TOKEN_FILE" ]; then
        echo "Current token info:"
        cat "$TOKEN_FILE" | python3 -m json.tool 2>/dev/null || cat "$TOKEN_FILE"
    else
        echo "Token file does not exist yet"
    fi
    exit 0
fi

# Check if principal exists
if ! stack-admin key get --for "$PRINCIPAL" &>/dev/null; then
    echo "ERROR: Principal '$PRINCIPAL' not found" >&2
    echo "Create it first with:" >&2
    echo "  stack-admin key create --for $PRINCIPAL --type service \\" >&2
    echo "    --display 'Russell (Host Curator)' --ttl $TTL" >&2
    exit 1
fi

# Rotate the token
echo "Rotating token for principal: $PRINCIPAL"
stack-admin key rotate --for "$PRINCIPAL" --format json > "$TOKEN_FILE"

# Set secure permissions
chmod 600 "$TOKEN_FILE"

# Verify the new token
if [ ! -f "$TOKEN_FILE" ] || [ ! -s "$TOKEN_FILE" ]; then
    echo "ERROR: Token file is empty or missing after rotation" >&2
    exit 1
fi

# Extract and display token info
EXPIRES=$(python3 -c "import json,sys; d=json.load(open('$TOKEN_FILE')); print(d.get('expires_at', 'unknown'))" 2>/dev/null || echo "unknown")

echo "✓ Token rotated successfully"
echo "  Principal: $PRINCIPAL"
echo "  Expires:   $EXPIRES"
echo "  Token file: $TOKEN_FILE"
echo ""
echo "Russell will automatically pick up the new token on next request."

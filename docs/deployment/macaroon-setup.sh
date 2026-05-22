#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# Macaroon configuration script

set -euo pipefail

CONFIG_DIR="${HOME}/.config/hkask"
MACAROON_KEY_FILE="${CONFIG_DIR}/macaroon-root.key"
MACAROON_CONFIG="${CONFIG_DIR}/macaroon.yaml"

echo "=== Russell Macaroon Configuration ==="
echo ""

# Create config directory
mkdir -p "${CONFIG_DIR}"

# Generate root key if not exists
if [ ! -f "${MACAROON_KEY_FILE}" ]; then
    echo "[1/3] Generating macaroon root key..."
    openssl rand -hex 32 > "${MACAROON_KEY_FILE}"
    chmod 600 "${MACAROON_KEY_FILE}"
    echo "  ✓ Key generated: ${MACAROON_KEY_FILE}"
else
    echo "[1/3] Using existing macaroon root key..."
    echo "  ✓ Key found: ${MACAROON_KEY_FILE}"
fi

# Read key
ROOT_KEY=$(cat "${MACAROON_KEY_FILE}")

# Create macaroon config
echo ""
echo "[2/3] Creating macaroon configuration..."
cat > "${MACAROON_CONFIG}" << YAML
# Russell ACP Macaroon Configuration
# Generated: $(date -Iseconds)

issuer:
  root_key: "${ROOT_KEY}"
  capabilities:
    - name: russell-acp
      attenuations:
        - skill: web-search
        - skill: journal-viewer
        - skill: journal-compactor
        - skill: package-checker
        - skill: scenario-tester
        - skill: ubuntu-jack
        - skill: pragmatic-cybernetics
        - skill: pragmatic-semantics
        - rate_limit: 100/minute
      before: 24h
YAML
chmod 600 "${MACAROON_CONFIG}"
echo "  ✓ Config created: ${MACAROON_CONFIG}"

# Set environment variable hint
echo ""
echo "[3/3] Environment configuration..."
echo ""
echo "Add to your shell profile (~/.bashrc or ~/.zshrc):"
echo ""
echo "  export RUSSELL_ACP_MACAROON_KEY=\"${ROOT_KEY}\""
echo ""

# Export for current session
export RUSSELL_ACP_MACAROON_KEY="${ROOT_KEY}"

echo "✓ Macaroon key exported for current session"
echo ""
echo "=== Configuration Complete ==="
echo ""
echo "To verify:"
echo "  echo \$RUSSELL_ACP_MACAROON_KEY"
echo ""

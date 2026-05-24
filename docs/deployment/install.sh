#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# Russell ACP Server installation script

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
HOME_DIR="${HOME}"

echo "=== Russell ACP Server Installation ==="
echo ""

# Check prerequisites
echo "[1/6] Checking prerequisites..."

if ! command -v cargo &> /dev/null; then
    echo "ERROR: cargo not found. Please install Rust."
    exit 1
fi

if ! systemctl --user --version &> /dev/null; then
    echo "ERROR: systemd --user not available."
    exit 1
fi

echo "  ✓ cargo: $(cargo --version)"
echo "  ✓ systemd: $(systemctl --user --version | head -1)"

# Build and install
echo ""
echo "[2/6] Building Russell binaries..."
cd "${SCRIPT_DIR}/../.."
cargo build --release -p russell-cli -p russell-acp-server

echo ""
echo "[3/6] Installing binaries..."
mkdir -p "${HOME_DIR}/.local/bin"
cp target/release/russell "${HOME_DIR}/.local/bin/"
chmod +x "${HOME_DIR}/.local/bin/russell"
echo "  ✓ Installed to ${HOME_DIR}/.local/bin/russell"
cp target/release/russell-acp-server "${HOME_DIR}/.local/bin/"
chmod +x "${HOME_DIR}/.local/bin/russell-acp-server"
echo "  ✓ Installed to ${HOME_DIR}/.local/bin/russell-acp-server"

# Install systemd units
echo ""
echo "[4/6] Installing systemd units..."
mkdir -p "${HOME_DIR}/.config/systemd/user"
cp "${SCRIPT_DIR}/../../packaging/systemd/russell-acp-server.service" "${HOME_DIR}/.config/systemd/user/"
cp "${SCRIPT_DIR}/../../packaging/systemd/russell-sentinel.service" "${HOME_DIR}/.config/systemd/user/"
cp "${SCRIPT_DIR}/../../packaging/systemd/russell-sentinel.timer" "${HOME_DIR}/.config/systemd/user/"
cp "${SCRIPT_DIR}/../../packaging/systemd/russell-digest.service" "${HOME_DIR}/.config/systemd/user/"
cp "${SCRIPT_DIR}/../../packaging/systemd/russell-digest.timer" "${HOME_DIR}/.config/systemd/user/"
cp "${SCRIPT_DIR}/../../packaging/systemd/russell-failure@.service" "${HOME_DIR}/.config/systemd/user/"
echo "  ✓ systemd units installed"

# Reload systemd
echo ""
echo "[5/6] Reloading systemd..."
systemctl --user daemon-reload
echo "  ✓ systemd reloaded"

# Check skills directory
echo ""
echo "[6/6] Checking skills directory..."
SKILLS_DIR="${HOME_DIR}/.local/share/harness/skills"
if [ -d "${SKILLS_DIR}" ]; then
    SKILL_COUNT=$(ls -1d "${SKILLS_DIR}"/*/ 2>/dev/null | wc -l)
    echo "  ✓ Skills directory exists (${SKILL_COUNT} skills)"
else
    echo "  ⚠ Skills directory not found. Creating..."
    mkdir -p "${SKILLS_DIR}"
fi

echo ""
echo "=== Installation Complete ==="
echo ""
echo "Next steps:"
echo "  1. Configure macaroon auth: ${SCRIPT_DIR}/macaroon-setup.sh"
echo "  2. Enable services:"
echo "     systemctl --user enable --now russell-sentinel.timer"
echo "     systemctl --user enable --now russell-acp-server.service"
echo "     systemctl --user enable --now russell-digest.timer"
echo "  3. Verify: systemctl --user status russell-acp-server.service"
echo ""

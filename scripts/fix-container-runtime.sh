#!/usr/bin/env bash
# =============================================================================
# Fix Container Runtime — Podman rootless with Docker CLI compatibility
# =============================================================================
# This script configures the workstation's container runtime properly:
#
#   - Podman 5.4.x is the engine (rootless, no daemon)
#   - Docker CLI talks to Podman via its socket (no docker-ce needed)
#   - Containers are managed as systemd user services via Quadlet
#   - Containers survive logout and auto-start on boot
#
# Run once. Idempotent — safe to re-run.
# Usage: bash ~/Clones/russell/scripts/fix-container-runtime.sh
# =============================================================================
set -euo pipefail

GRN='\033[0;32m'; CYN='\033[0;36m'; YEL='\033[1;33m'; BLD='\033[1m'; RST='\033[0m'
ok()   { echo -e "  ${GRN}✓${RST} $1"; }
info() { echo -e "  ${CYN}ℹ${RST} $1"; }
warn() { echo -e "  ${YEL}⚠${RST} $1"; }

echo -e "${BLD}"
echo "  ┌─────────────────────────────────────────────────────┐"
echo "  │  Container Runtime Setup — Podman + Docker CLI       │"
echo "  │  $(date +%Y-%m-%d)                                           │"
echo "  └─────────────────────────────────────────────────────┘"
echo -e "${RST}"

# ─── 1. Enable linger (user services survive logout) ────────────────────────
echo -e "\n${CYN}━━━ Step 1: Enable user linger${RST}"
if loginctl show-user "$USER" 2>/dev/null | grep -q "Linger=yes"; then
    ok "Linger already enabled"
else
    sudo loginctl enable-linger "$USER"
    ok "Linger enabled — user services will survive logout"
fi

# ─── 2. Enable Podman socket (Docker CLI compatibility) ─────────────────────
echo -e "\n${CYN}━━━ Step 2: Enable Podman user socket${RST}"
if systemctl --user is-active podman.socket &>/dev/null; then
    ok "podman.socket already active"
else
    systemctl --user enable --now podman.socket
    ok "podman.socket enabled and started"
fi
info "Socket at: /run/user/$(id -u)/podman/podman.sock"

# ─── 3. Point Docker CLI at Podman socket ───────────────────────────────────
echo -e "\n${CYN}━━━ Step 3: Configure Docker CLI context${RST}"
PODMAN_SOCK="unix:///run/user/$(id -u)/podman/podman.sock"
if docker context inspect podman &>/dev/null; then
    ok "Docker context 'podman' already exists"
else
    docker context create podman --docker "host=${PODMAN_SOCK}"
    ok "Created Docker context 'podman'"
fi
docker context use podman
ok "Docker CLI now routes to Podman"

# ─── 4. Stop existing kask-qdrant container (will be replaced by Quadlet) ───
echo -e "\n${CYN}━━━ Step 4: Migrate kask-qdrant to Quadlet${RST}"
QUADLET_DIR="${HOME}/.config/containers/systemd"
mkdir -p "$QUADLET_DIR"

if podman container exists kask-qdrant 2>/dev/null; then
    info "Stopping existing kask-qdrant container..."
    podman stop kask-qdrant 2>/dev/null || true
    podman rm kask-qdrant 2>/dev/null || true
    ok "Removed ephemeral kask-qdrant container"
else
    info "No ephemeral kask-qdrant container running"
fi

# ─── 5. Write Quadlet unit file ─────────────────────────────────────────────
QUADLET_FILE="${QUADLET_DIR}/kask-qdrant.container"
cat > "$QUADLET_FILE" <<'EOF'
# Kask Qdrant — vector database for the Kask knowledge system
# Managed by Podman Quadlet (systemd user service)
# Docs: https://docs.podman.io/en/latest/markdown/podman-systemd.unit.5.html

[Unit]
Description=Kask Qdrant vector database
After=network-online.target
Wants=network-online.target

[Container]
Image=docker.io/qdrant/qdrant:latest
ContainerName=kask-qdrant
PublishPort=6333:6333
PublishPort=6334:6334
Volume=/home/mdz-axolotl/Clones/kask/.data/qdrant:/qdrant/storage
Environment=RUN_MODE=production
AutoUpdate=registry

[Install]
WantedBy=default.target
EOF
ok "Wrote Quadlet file: ${QUADLET_FILE}"

# ─── 6. Reload systemd and start the service ────────────────────────────────
echo -e "\n${CYN}━━━ Step 5: Activate Quadlet service${RST}"
systemctl --user daemon-reload
# Quadlet-generated units cannot be `enable`d — systemd treats them as
# transient/generated. Auto-start is handled by the [Install] WantedBy=
# directive in the .container file, which the Quadlet generator wires up.
systemctl --user start kask-qdrant
ok "kask-qdrant.service started (auto-starts on boot via Quadlet)"

# ─── 7. Enable auto-update timer ────────────────────────────────────────────
echo -e "\n${CYN}━━━ Step 6: Enable container auto-updates${RST}"
if systemctl --user is-enabled podman-auto-update.timer &>/dev/null; then
    ok "podman-auto-update.timer already enabled"
else
    systemctl --user enable --now podman-auto-update.timer
    ok "Container images will auto-update on schedule"
fi

# ─── 8. Verify ──────────────────────────────────────────────────────────────
echo -e "\n${CYN}━━━ Verification${RST}"
sleep 2
if podman ps --filter name=kask-qdrant --format "{{.Status}}" | grep -qi "up"; then
    ok "kask-qdrant is running"
else
    warn "kask-qdrant may still be starting — check: systemctl --user status kask-qdrant"
fi

if docker info &>/dev/null; then
    ok "Docker CLI → Podman socket working"
else
    warn "Docker CLI not connecting — check: docker context ls"
fi

echo ""
echo -e "${BLD}  Done. Container runtime is configured.${RST}"
echo ""
echo "  Useful commands:"
echo "    systemctl --user status kask-qdrant    # check container service"
echo "    podman ps                              # list running containers"
echo "    docker ps                              # same thing, via Docker CLI"
echo "    podman logs kask-qdrant                # container logs"
echo "    systemctl --user restart kask-qdrant   # restart"
echo ""

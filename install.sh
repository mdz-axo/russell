#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
#
# russell/install.sh — one-command Russell setup.
#
# Usage:
#   ./install.sh              # Build and install (release)
#   ./install.sh --dev        # Build and install (debug, for development)
#   ./install.sh --check      # Dry-run: show what would happen
#   ./install.sh --uninstall  # Remove Russell's files and timers
#
# Installs:
#   ~/.local/bin/russell             — the binary
#   ~/.local/bin/russell-acp-server  — ACP server for hKask
#   ~/.local/bin/russell-api-server — HTTP REST API server
#   ~/.local/state/harness/          — journal, evidence, memory
#   ~/.local/share/harness/skills/   — skill manifests + scripts
#   ~/.local/share/harness/rules.d/  — default threshold rules
#   ~/.config/systemd/user/          — timers and services
#   ~/.config/harness/russell.env    — environment (created if missing)

set -euo pipefail

#######################
# Config
#######################

BIN_DIR="${HOME}/.local/bin"
STATE_DIR="${HOME}/.local/state/harness"
SHARE_DIR="${HOME}/.local/share/harness"
CONFIG_DIR="${HOME}/.config/harness"
SYSTEMD_USER_DIR="${HOME}/.config/systemd/user"

BINARY_NAME="russell"
REPO_ROOT="$(cd "$(dirname "$0")" && pwd)"

MODE="release"
ACTION="install"
NO_SYSTEMD=0

#######################
# Argument parsing
#######################

for arg in "$@"; do
    case "$arg" in
        --dev) MODE="debug" ;;
        --check) ACTION="check" ;;
        --uninstall) ACTION="uninstall" ;;
        -h|--help)
            echo "Usage: $0 [--dev] [--check] [--uninstall]"
            echo ""
            echo "  --dev        Build debug (faster compile, slower runs)"
            echo "  --check      Dry-run: show what would happen"
            echo "  --uninstall  Remove Russell's files and timers"
            exit 0
            ;;
        *)
            echo "Unknown option: $arg"
            echo "Usage: $0 [--dev] [--check] [--uninstall]"
            exit 1
            ;;
    esac
done

#######################
# Prerequisites
#######################

if [ "$ACTION" != "uninstall" ]; then
    if ! command -v cargo &>/dev/null; then
        echo "ERROR: cargo not found. Install Rust: https://rustup.rs"
        exit 1
    fi
fi

if ! command -v systemctl &>/dev/null; then
    if [ "$ACTION" = "uninstall" ]; then
        echo "WARNING: systemctl not found. Skipping systemd teardown."
    else
        echo "WARNING: systemctl not found. Systemd units will not be installed."
    fi
    NO_SYSTEMD=1
fi

#######################
# Uninstall
#######################

if [ "$ACTION" = "uninstall" ]; then
    if [ "$NO_SYSTEMD" = "0" ]; then
        echo "==> Stopping and disabling Russell timers and services…"
        for unit in russell-sentinel.timer russell-digest.timer \
                    russell-acp-server.service russell-api-server.service; do
            systemctl --user stop "$unit" 2>/dev/null || true
            systemctl --user disable "$unit" 2>/dev/null || true
        done

        echo "==> Removing systemd units…"
        rm -f "${SYSTEMD_USER_DIR}/russell-sentinel.service"
        rm -f "${SYSTEMD_USER_DIR}/russell-sentinel.timer"
        rm -f "${SYSTEMD_USER_DIR}/russell-digest.service"
        rm -f "${SYSTEMD_USER_DIR}/russell-digest.timer"
        rm -f "${SYSTEMD_USER_DIR}/russell-failure@.service"
        rm -f "${SYSTEMD_USER_DIR}/russell-acp-server.service"
        rm -f "${SYSTEMD_USER_DIR}/russell-api-server.service"
    else
        echo "==> Skipping systemd teardown (systemd not available)"
    fi

    echo "==> Removing binaries…"
    rm -f "${BIN_DIR}/${BINARY_NAME}"
    rm -f "${BIN_DIR}/russell-acp-server"
    rm -f "${BIN_DIR}/russell-api-server"
    rm -f "${HOME}/.cargo/bin/${BINARY_NAME}"
    rm -f "${HOME}/.cargo/bin/russell-acp-server"
    rm -f "${HOME}/.cargo/bin/russell-api-server"

    echo "==> Russell state and data preserved at:"
    echo "    ${STATE_DIR}"
    echo "    ${SHARE_DIR}"
    echo "    ${CONFIG_DIR}"
    echo ""
    echo "Remove them manually if you want a full reset:"
    echo "    rm -rf ${STATE_DIR} ${SHARE_DIR} ${CONFIG_DIR}"
    exit 0
fi

#######################
# Build
#######################

echo "==> Building Russell (${MODE})…"
cd "$REPO_ROOT"

if [ "$MODE" = "release" ]; then
    cargo build --release -p russell-cli -p russell-acp-server -p russell-api-server 2>&1
    BINARY_SRC="${REPO_ROOT}/target/release/${BINARY_NAME}"
    ACP_SERVER_SRC="${REPO_ROOT}/target/release/russell-acp-server"
    API_SERVER_SRC="${REPO_ROOT}/target/release/russell-api-server"
else
    cargo build -p russell-cli -p russell-acp-server -p russell-api-server 2>&1
    BINARY_SRC="${REPO_ROOT}/target/debug/${BINARY_NAME}"
    ACP_SERVER_SRC="${REPO_ROOT}/target/debug/russell-acp-server"
    API_SERVER_SRC="${REPO_ROOT}/target/debug/russell-api-server"
fi

if [ ! -f "$BINARY_SRC" ]; then
    echo "ERROR: binary not found at ${BINARY_SRC} — build may have failed"
    exit 1
fi

if [ ! -f "$ACP_SERVER_SRC" ]; then
    echo "WARNING: russell-acp-server binary not found at ${ACP_SERVER_SRC}"
fi

if [ ! -f "$API_SERVER_SRC" ]; then
    echo "WARNING: russell-api-server binary not found at ${API_SERVER_SRC}"
fi

#######################
# Check mode
#######################

if [ "$ACTION" = "check" ]; then
    echo ""
    echo "Would install to:"
    echo "  binary:    ${BIN_DIR}/${BINARY_NAME}"
    echo "  acp:       ${BIN_DIR}/russell-acp-server"
    echo "  api:       ${BIN_DIR}/russell-api-server"
    echo "  state:     ${STATE_DIR}"
    echo "  data:      ${SHARE_DIR}"
    echo "  config:    ${CONFIG_DIR}"
    echo "  units:     ${SYSTEMD_USER_DIR}"
    echo ""
    if [ "$NO_SYSTEMD" = "0" ]; then
        echo "Would enable timers:"
        echo "  russell-sentinel.timer"
        echo "  russell-digest.timer"
        echo "  russell-acp-server.service"
        echo "  russell-api-server.service"
    else
        echo "Systemd units: SKIPPED (systemd not available)"
    fi
    exit 0
fi

#######################
# Install
#######################

echo "==> Creating directories…"
mkdir -p "$BIN_DIR"
mkdir -p "${STATE_DIR}/runs"
mkdir -p "${SHARE_DIR}/skills"
mkdir -p "${SHARE_DIR}/rules.d"
mkdir -p "$CONFIG_DIR"

echo "==> Installing binaries…"
cp "$BINARY_SRC" "${BIN_DIR}/${BINARY_NAME}"
chmod +x "${BIN_DIR}/${BINARY_NAME}"

if [ -f "$ACP_SERVER_SRC" ]; then
    cp "$ACP_SERVER_SRC" "${BIN_DIR}/russell-acp-server"
    chmod +x "${BIN_DIR}/russell-acp-server"
    echo "  → russell-acp-server installed"
else
    echo "  → Warning: russell-acp-server not built, skipping"
fi

if [ -f "$API_SERVER_SRC" ]; then
    cp "$API_SERVER_SRC" "${BIN_DIR}/russell-api-server"
    chmod +x "${BIN_DIR}/russell-api-server"
    echo "  → russell-api-server installed"
else
    echo "  → Warning: russell-api-server not built, skipping"
fi

CARGO_BIN="${HOME}/.cargo/bin/${BINARY_NAME}"
if [ -f "$CARGO_BIN" ] && [ "$CARGO_BIN" != "${BIN_DIR}/${BINARY_NAME}" ]; then
    echo "  → Removing stale ${CARGO_BIN} (canonical is ${BIN_DIR}/${BINARY_NAME})"
    rm -f "$CARGO_BIN"
fi

ACP_CARGO_BIN="${HOME}/.cargo/bin/russell-acp-server"
if [ -f "$ACP_CARGO_BIN" ]; then
    echo "  → Removing stale ${ACP_CARGO_BIN} (canonical is ${BIN_DIR}/russell-acp-server)"
    rm -f "$ACP_CARGO_BIN"
fi

API_CARGO_BIN="${HOME}/.cargo/bin/russell-api-server"
if [ -f "$API_CARGO_BIN" ]; then
    echo "  → Removing stale ${API_CARGO_BIN} (canonical is ${BIN_DIR}/russell-api-server)"
    rm -f "$API_CARGO_BIN"
fi

if [ "$NO_SYSTEMD" = "0" ]; then
    echo "==> Installing systemd units…"
    mkdir -p "$SYSTEMD_USER_DIR"
    cp "${REPO_ROOT}/packaging/systemd/russell-sentinel.service" "$SYSTEMD_USER_DIR"
    cp "${REPO_ROOT}/packaging/systemd/russell-sentinel.timer" "$SYSTEMD_USER_DIR"
    cp "${REPO_ROOT}/packaging/systemd/russell-digest.service" "$SYSTEMD_USER_DIR"
    cp "${REPO_ROOT}/packaging/systemd/russell-digest.timer" "$SYSTEMD_USER_DIR"
    cp "${REPO_ROOT}/packaging/systemd/russell-failure@.service" "$SYSTEMD_USER_DIR"
    cp "${REPO_ROOT}/packaging/systemd/russell-acp-server.service" "$SYSTEMD_USER_DIR"
    cp "${REPO_ROOT}/packaging/systemd/russell-api-server.service" "$SYSTEMD_USER_DIR"
    echo "  → 7 systemd units installed"
fi

echo "==> Installing default rules…"
if [ ! -f "${SHARE_DIR}/rules.d/README.md" ]; then
    cat > "${SHARE_DIR}/rules.d/README.md" <<'RULES_README'
# Russell Rules

Default thresholds ship in the Russell binary (RuleSet::with_defaults()).
Place TOML override files here to customise or silence thresholds.

Format matches the internal Rule struct:
```toml
[probe_name]
description = "what this probe measures"
warn_above = 80.0   # threshold for warn severity
alert_above = 90.0  # threshold for alert severity
crit_above = 95.0   # threshold for critical severity
# warn_below / alert_below / crit_below also supported
```

Files are loaded alphabetically. Later files override earlier files.
RULES_README
    echo "  → rules.d/README.md created as guidance"
fi

if [ -f "${REPO_ROOT}/rules.d/docs.toml" ]; then
    cp "${REPO_ROOT}/rules.d/docs.toml" "${SHARE_DIR}/rules.d/"
    echo "  → rules.d/docs.toml installed"
fi

echo "==> Installing default skills…"
if [ -d "${REPO_ROOT}/skills" ]; then
    for skill_dir in "${REPO_ROOT}/skills"/*/; do
        skill_name=$(basename "$skill_dir")
        [[ "$skill_name" =~ ^[_\.] ]] && continue
        if [ ! -d "${SHARE_DIR}/skills/${skill_name}" ]; then
            cp -r "$skill_dir" "${SHARE_DIR}/skills/"
            echo "  → ${skill_name} skill installed"
        else
            echo "  → ${skill_name} skill already present (not overwritten)"
        fi
    done
fi

echo "==> Setting up environment…"
if [ ! -f "${CONFIG_DIR}/russell.env" ]; then
    cat > "${CONFIG_DIR}/russell.env" <<'ENV_EOF'
# Russell environment configuration.
# See docs/operations/REUSE_MANIFEST.md for available vars.

# Okapi endpoint for LLM inference
# OLLAMA_HOST=127.0.0.1:11435

# Override the LLM model for `russell jack`
# HARVESTER_MODEL=deepseek-v4-pro
ENV_EOF
    echo "  → ${CONFIG_DIR}/russell.env created (edit to configure)"
fi

if [ "$NO_SYSTEMD" = "0" ]; then
    echo "==> Reloading systemd and enabling timers…"
    systemctl --user daemon-reload

    systemctl --user enable russell-sentinel.timer
    systemctl --user enable russell-digest.timer
    systemctl --user enable russell-acp-server.service
    systemctl --user enable russell-api-server.service

    systemctl --user start russell-sentinel.timer
    systemctl --user start russell-digest.timer 2>/dev/null || true
    systemctl --user start russell-acp-server.service 2>/dev/null || true
    systemctl --user start russell-api-server.service 2>/dev/null || true
fi

echo ""
echo "====================="
echo " Russell installed."
echo "====================="
echo ""
echo "  Binary:    ${BIN_DIR}/${BINARY_NAME}"
echo "  Journal:   ${STATE_DIR}/journal.db"
echo "  Skills:    ${SHARE_DIR}/skills/"
echo "  Rules:     ${SHARE_DIR}/rules.d/"
echo "  Env:       ${CONFIG_DIR}/russell.env"
echo ""
echo "  Try:  russell status"
echo "        russell jack"
echo ""
if [ "$NO_SYSTEMD" = "0" ]; then
    echo "  Timers:"
    echo "    russell-sentinel.timer  — every 5 min"
    echo "    russell-digest.timer    — weekly"
    echo ""
    echo "  Check:  systemctl --user list-timers russell-*"
    echo "          journalctl --user -u russell-sentinel -f"
else
    echo "  Note: systemd not detected. Timers not installed."
    echo "  You can run the sentinel manually: russell sentinel-once"
fi

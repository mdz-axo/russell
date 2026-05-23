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
# Uninstall
#######################

if [ "$ACTION" = "uninstall" ]; then
    echo "==> Stopping and disabling Russell timers…"
    systemctl --user stop russell-sentinel.timer 2>/dev/null || true
    systemctl --user stop russell-okapi.timer 2>/dev/null || true
    systemctl --user stop russell-digest.timer 2>/dev/null || true
    systemctl --user disable russell-sentinel.timer 2>/dev/null || true
    systemctl --user disable russell-okapi.timer 2>/dev/null || true
    systemctl --user disable russell-digest.timer 2>/dev/null || true

    echo "==> Removing systemd units…"
    rm -f "${SYSTEMD_USER_DIR}/russell-sentinel.service"
    rm -f "${SYSTEMD_USER_DIR}/russell-sentinel.timer"
    rm -f "${SYSTEMD_USER_DIR}/russell-okapi.service"
    rm -f "${SYSTEMD_USER_DIR}/russell-okapi.timer"
    rm -f "${SYSTEMD_USER_DIR}/russell-digest.service"
    rm -f "${SYSTEMD_USER_DIR}/russell-digest.timer"
    rm -f "${SYSTEMD_USER_DIR}/russell-failure@.service"

    echo "==> Removing binaries…"
    rm -f "${BIN_DIR}/${BINARY_NAME}"
    rm -f "${BIN_DIR}/russell-acp-server"
    rm -f "${HOME}/.cargo/bin/${BINARY_NAME}"
    rm -f "${HOME}/.cargo/bin/russell-acp-server"

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
    cargo build --release 2>&1
    BINARY_SRC="${REPO_ROOT}/target/release/${BINARY_NAME}"
else
    cargo build 2>&1
    BINARY_SRC="${REPO_ROOT}/target/debug/${BINARY_NAME}"
fi

if [ ! -f "$BINARY_SRC" ]; then
    echo "ERROR: binary not found at ${BINARY_SRC} — build may have failed"
    exit 1
fi

#######################
# Check mode
#######################

if [ "$ACTION" = "check" ]; then
    echo ""
    echo "Would install to:"
    echo "  binary:    ${BIN_DIR}/${BINARY_NAME}"
    echo "  state:     ${STATE_DIR}"
    echo "  data:      ${SHARE_DIR}"
    echo "  config:    ${CONFIG_DIR}"
    echo "  units:     ${SYSTEMD_USER_DIR}"
    echo ""
    echo "Would enable timers:"
    echo "  russell-sentinel.timer"
    echo "  russell-okapi.timer"
    echo "  russell-digest.timer"
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
mkdir -p "$SYSTEMD_USER_DIR"

echo "==> Installing binaries…"
cp "$BINARY_SRC" "${BIN_DIR}/${BINARY_NAME}"
chmod +x "${BIN_DIR}/${BINARY_NAME}"

# Also install russell-acp-server for hKask integration
ACP_SERVER_SRC="${REPO_ROOT}/target/release/russell-acp-server"
if [ -f "$ACP_SERVER_SRC" ]; then
    cp "$ACP_SERVER_SRC" "${BIN_DIR}/russell-acp-server"
    chmod +x "${BIN_DIR}/russell-acp-server"
    echo "  → russell-acp-server installed"
else
    echo "  → Warning: russell-acp-server not built, skipping"
fi

# Remove any stale cargo-installed copy to prevent PATH shadowing.
# The canonical install location is ~/.local/bin (this script).
CARGO_BIN="${HOME}/.cargo/bin/${BINARY_NAME}"
if [ -f "$CARGO_BIN" ] && [ "$CARGO_BIN" != "${BIN_DIR}/${BINARY_NAME}" ]; then
    echo "  → Removing stale ${CARGO_BIN} (canonical is ${BIN_DIR}/${BINARY_NAME})"
    rm -f "$CARGO_BIN"
fi

# Also remove stale ACP server from cargo bin
ACP_CARGO_BIN="${HOME}/.cargo/bin/russell-acp-server"
if [ -f "$ACP_CARGO_BIN" ]; then
    echo "  → Removing stale ${ACP_CARGO_BIN} (canonical is ${BIN_DIR}/russell-acp-server)"
    rm -f "$ACP_CARGO_BIN"
fi

echo "==> Installing systemd units…"
cp "${REPO_ROOT}/packaging/systemd/russell-sentinel.service" "$SYSTEMD_USER_DIR"
cp "${REPO_ROOT}/packaging/systemd/russell-sentinel.timer" "$SYSTEMD_USER_DIR"
cp "${REPO_ROOT}/packaging/systemd/russell-okapi.service" "$SYSTEMD_USER_DIR"
cp "${REPO_ROOT}/packaging/systemd/russell-okapi.timer" "$SYSTEMD_USER_DIR"
cp "${REPO_ROOT}/packaging/systemd/russell-digest.service" "$SYSTEMD_USER_DIR"
cp "${REPO_ROOT}/packaging/systemd/russell-digest.timer" "$SYSTEMD_USER_DIR"
cp "${REPO_ROOT}/packaging/systemd/russell-failure@.service" "$SYSTEMD_USER_DIR"
# ACP server unit for hKask integration
if [ -f "${REPO_ROOT}/docs/deployment/russell-acp-server.service" ]; then
    cp "${REPO_ROOT}/docs/deployment/russell-acp-server.service" "$SYSTEMD_USER_DIR"
    echo "  → russell-acp-server.service installed"
fi

echo "==> Installing default rules…"
# Ship default rules only if the target doesn't already have rules
# (the operator may have customised them).
if [ ! -f "${SHARE_DIR}/rules.d/memory.toml" ]; then
    # Rules are compiled into the binary via RuleSet::defaults().
    # We create empty marker files so the operator knows where to
    # place overrides. The defaults come from the binary.
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

echo "==> Installing default skills…"
if [ -d "${REPO_ROOT}/skills" ]; then
    for skill_dir in "${REPO_ROOT}/skills"/*/; do
        skill_name=$(basename "$skill_dir")
        # Skip hidden/underscore-prefixed dirs
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

# Okapi model for auto-load (used by russell-okapi.service)
# RUSSELL_OKAPI_DEFAULT_MODEL=

# Override the LLM backend endpoint
# OLLAMA_HOST=127.0.0.1:11435

# Override the LLM model for `russell jack` / `russell chat`
# HARVESTER_MODEL=deepseek-v4-pro
ENV_EOF
    echo "  → ${CONFIG_DIR}/russell.env created (edit to configure)"
fi

echo "==> Reloading systemd and enabling timers…"
systemctl --user daemon-reload

# Enable the timers (they start on next boot or immediately if activated).
systemctl --user enable russell-sentinel.timer
systemctl --user enable russell-okapi.timer
systemctl --user enable russell-digest.timer

# Start them now.
systemctl --user start russell-sentinel.timer
systemctl --user start russell-okapi.timer 2>/dev/null || {
    echo "  → Note: russell-okapi.timer failed to start — is Okapi running?"
}
systemctl --user start russell-digest.timer 2>/dev/null || true

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
echo "        russell chat"
echo ""
echo "  Timers:"
echo "    russell-sentinel.timer  — every 5 min"
echo "    russell-okapi.timer     — every 5 min (offset)"
echo "    russell-digest.timer    — weekly"
echo ""
echo "  Check:  systemctl --user list-timers russell-*"
echo "          journalctl --user -u russell-sentinel -f"
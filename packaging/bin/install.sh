#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
#
# packaging/bin/install.sh — install Russell for the current user.
#
# Usage: ./packaging/bin/install.sh [--no-start] [--release]
#
# What it does (all idempotent):
#   1. `cargo build --release` (or debug with no --release) of the
#      `russell` binary.
#   2. Installs `target/.../russell` → `~/.local/bin/russell`.
#   3. Installs systemd unit files → `~/.config/systemd/user/`.
#   4. Creates `~/.config/harness/` and copies `.env.example` into
#      `russell.env` if no config exists yet (editable by you).
#   5. Creates `~/.local/state/harness/` with the expected layout.
#   6. `systemctl --user daemon-reload`.
#   7. Enables and (unless `--no-start`) starts the Sentinel +
#      digest timers.
#
# Nothing here requires root. Russell runs under your user.

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/../.." && pwd)"

NO_START=0
PROFILE=debug
for arg in "$@"; do
  case "$arg" in
    --no-start) NO_START=1 ;;
    --release) PROFILE=release ;;
    -h|--help)
      sed -n '3,20p' "$0"
      exit 0 ;;
    *) echo "unknown flag: $arg" >&2; exit 2 ;;
  esac
done

say() { printf '\033[1;34m[install]\033[0m %s\n' "$*"; }

say "Building russell ($PROFILE)…"
if [ "$PROFILE" = "release" ]; then
  (cd "$REPO" && cargo build --release -p russell-cli)
  BIN="$REPO/target/release/russell"
else
  (cd "$REPO" && cargo build -p russell-cli)
  BIN="$REPO/target/debug/russell"
fi

say "Installing binary → ~/.local/bin/russell"
mkdir -p "$HOME/.local/bin"
install -m 0755 "$BIN" "$HOME/.local/bin/russell"

say "Installing systemd user units → ~/.config/systemd/user/"
mkdir -p "$HOME/.config/systemd/user"
for u in "$REPO"/packaging/systemd/*.service "$REPO"/packaging/systemd/*.timer; do
  install -m 0644 "$u" "$HOME/.config/systemd/user/"
done

say "Ensuring config + state + data directories"
mkdir -p "$HOME/.config/harness" "$HOME/.local/state/harness/runs" \
         "$HOME/.local/state/harness/evidence/help" \
         "$HOME/.local/state/harness/digest" \
         "$HOME/.local/share/harness/skills"

say "Installing skills → ~/.local/share/harness/skills/"
cp -r "$REPO/skills/ollama-watcher" "$HOME/.local/share/harness/skills/"
cp -r "$REPO/skills/ubuntu-jack"    "$HOME/.local/share/harness/skills/"
cp -r "$REPO/skills/sysadmin"       "$HOME/.local/share/harness/skills/"
chmod +x "$HOME/.local/share/harness/skills/ollama-watcher/scripts/"*.sh 2>/dev/null || true
chmod +x "$HOME/.local/share/harness/skills/sysadmin/scripts/"*.sh 2>/dev/null || true

# Build and install arsenal-mcp-russell if Kask repo is available
KASK_REPO="${HOME}/Clones/kask"
if [ -d "$KASK_REPO" ] && [ -f "$KASK_REPO/Cargo.toml" ]; then
  say "Building arsenal-mcp-russell from Kask repo…"
  (cd "$KASK_REPO" && cargo build -p arsenal-mcp-russell)
  MCP_BIN="$KASK_REPO/target/debug/arsenal-mcp-russell"
  if [ -x "$MCP_BIN" ]; then
    say "Installing arsenal-mcp-russell → ~/.local/bin/"
    mkdir -p "$HOME/.local/bin"
    install -m 0755 "$MCP_BIN" "$HOME/.local/bin/arsenal-mcp-russell"
    
    # Update MCP registry to point to correct location
    say "Updating MCP registry…"
    mkdir -p "$HOME/.config/stack"
    cat > "$HOME/.config/stack/mcp-registry.json" <<'MCPREG'
{
  "version": "1.0",
  "servers": {
    "russell": {
      "transport": {
        "type": "stdio",
        "command": "/home/mdz-axolotl/.local/bin/arsenal-mcp-russell",
        "args": []
      },
      "enabled": true,
      "auto_start": true,
      "tags": ["infrastructure", "health", "host"],
      "description": "Russell host-health bridge"
    },
    "okapi-metrics": {
      "transport": {
        "type": "stdio",
        "command": "/home/mdz-axolotl/.local/bin/arsenal-mcp-okapi-metrics",
        "args": []
      },
      "enabled": true,
      "auto_start": true,
      "tags": ["okapi", "inference", "metrics"],
      "description": "Okapi inference metrics"
    }
  }
}
MCPREG
    
    # Build stack-api for MCP HTTP gateway
    say "Building Kask MCP HTTP gateway (stack-api)…"
    (cd "$KASK_REPO" && cargo build -p stack-api)
    if [ -f "$KASK_REPO/target/debug/stack-api" ]; then
      say "MCP gateway binary ready at $KASK_REPO/target/debug/stack-api"
      say "To start the MCP gateway, run:"
      say "  $REPO/scripts/start-kask-mcp-gateway.sh --background"
    fi
  else
    say "WARNING: arsenal-mcp-russell build failed, MCP tools unavailable"
  fi
else
  say "Kask repo not found at $KASK_REPO — skipping MCP server build"
  say "Note: Russell's MCP client requires Kask for advanced tool access"
fi
else
  say "Kask repo not found at $KASK_REPO — skipping MCP server build"
fi

if [ ! -f "$HOME/.config/harness/russell.env" ]; then
  # Prefer the repo .env if it's populated (convenience during dev).
  # Otherwise seed from the template.
  if [ -f "$REPO/.env.example" ]; then
    say "Seeding ~/.config/harness/russell.env from .env.example (template)"
    cp "$REPO/.env.example" "$HOME/.config/harness/russell.env"
    chmod 0600 "$HOME/.config/harness/russell.env"
  fi
fi

say "Reloading systemd user daemon"
systemctl --user daemon-reload

say "Enabling timers"
systemctl --user enable russell-sentinel.timer
systemctl --user enable russell-digest.timer

if [ "$NO_START" -eq 0 ]; then
  say "Starting timers"
  systemctl --user start russell-sentinel.timer
  systemctl --user start russell-digest.timer
else
  say "Skipping start (--no-start)"
fi

say "Running first Sentinel cycle to prove the wiring"
"$HOME/.local/bin/russell" sentinel-once

say "Status:"
"$HOME/.local/bin/russell" status

cat <<TAIL

Russell is installed.

  russell status                     # summary
  russell list --limit 20            # recent events
  russell digest --since-hours 168   # weekly digest
  russell jack --note "your worry"   # ask Jack

  systemctl --user list-timers 'russell-*'
  journalctl --user -u russell-sentinel.service --since "1 hour ago"

To uninstall:
  $REPO/packaging/bin/uninstall.sh
TAIL

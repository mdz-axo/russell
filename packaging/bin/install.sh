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
NO_SYSTEMD=0
for arg in "$@"; do
  case "$arg" in
    --no-start) NO_START=1 ;;
    --release) PROFILE=release ;;
    --no-systemd) NO_SYSTEMD=1 ;;
    -h|--help)
      sed -n '3,20p' "$0"
      exit 0 ;;
    *) echo "unknown flag: $arg" >&2; exit 2 ;;
  esac
done

say() { printf '\033[1;34m[install]\033[0m %s\n' "$*"; }

if [ "$NO_SYSTEMD" -eq 0 ] && ! command -v systemctl &>/dev/null; then
  say "WARNING: systemctl not found. Systemd units will not be installed."
  NO_SYSTEMD=1
fi

say "Building russell ($PROFILE)…"
if [ "$PROFILE" = "release" ]; then
  (cd "$REPO" && cargo build --release -p russell-cli -p russell-acp-server -p russell-api-server)
  BIN="$REPO/target/release/russell"
  ACP_BIN="$REPO/target/release/russell-acp-server"
  API_BIN="$REPO/target/release/russell-api-server"
else
  (cd "$REPO" && cargo build -p russell-cli -p russell-acp-server -p russell-api-server)
  BIN="$REPO/target/debug/russell"
  ACP_BIN="$REPO/target/debug/russell-acp-server"
  API_BIN="$REPO/target/debug/russell-api-server"
fi

say "Installing binary → ~/.local/bin/russell"
mkdir -p "$HOME/.local/bin"
install -m 0755 "$BIN" "$HOME/.local/bin/russell"

if [ -f "$ACP_BIN" ]; then
  say "Installing ACP server → ~/.local/bin/russell-acp-server"
  install -m 0755 "$ACP_BIN" "$HOME/.local/bin/russell-acp-server"
fi

if [ -f "$API_BIN" ]; then
  say "Installing API server → ~/.local/bin/russell-api-server"
  install -m 0755 "$API_BIN" "$HOME/.local/bin/russell-api-server"
fi

if [ "$NO_SYSTEMD" -eq 0 ]; then
  say "Installing systemd user units → ~/.config/systemd/user/"
  mkdir -p "$HOME/.config/systemd/user"
  for u in "$REPO"/packaging/systemd/*.service "$REPO"/packaging/systemd/*.timer; do
    install -m 0644 "$u" "$HOME/.config/systemd/user/"
  done
else
  say "Skipping systemd units (systemd not available or --no-systemd)"
fi

say "Ensuring config + state + data directories"
mkdir -p "$HOME/.config/harness" "$HOME/.local/state/harness/runs" \
         "$HOME/.local/state/harness/evidence/help" \
         "$HOME/.local/state/harness/digest" \
         "$HOME/.local/share/harness/skills" \
         "$HOME/.local/share/harness/rules.d"

say "Installing skills → ~/.local/share/harness/skills/"
for skill_dir in "$REPO"/skills/*/; do
  skill_name=$(basename "$skill_dir")
  [[ "$skill_name" =~ ^[_\.] ]] && continue
  if [ ! -d "$HOME/.local/share/harness/skills/$skill_name" ]; then
    cp -r "$skill_dir" "$HOME/.local/share/harness/skills/"
    say "  → $skill_name skill installed"
  else
    say "  → $skill_name skill already present (not overwritten)"
  fi
done
chmod +x "$HOME/.local/share/harness/skills/"*/scripts/*.sh 2>/dev/null || true

say "Installing default rules"
if [ -f "$REPO/rules.d/docs.toml" ]; then
    cp "$REPO/rules.d/docs.toml" "$HOME/.local/share/harness/rules.d/"
    say "  → rules.d/docs.toml installed"
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

if [ "$NO_SYSTEMD" -eq 0 ]; then
  say "Reloading systemd user daemon"
  systemctl --user daemon-reload

  say "Enabling timers and services"
  systemctl --user enable russell-sentinel.timer
  systemctl --user enable russell-digest.timer
  systemctl --user enable russell-acp-server.service
  systemctl --user enable russell-api-server.service

  if [ "$NO_START" -eq 0 ]; then
    say "Starting timers and services"
    systemctl --user start russell-sentinel.timer
    systemctl --user start russell-digest.timer
    systemctl --user start russell-acp-server.service 2>/dev/null || true
    systemctl --user start russell-api-server.service 2>/dev/null || true
  else
    say "Skipping start (--no-start)"
  fi
else
  say "Skipping systemd enable/start (systemd not available or --no-systemd)"
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

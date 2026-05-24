#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
#
# packaging/bin/uninstall.sh — remove Russell.
#
# Leaves your data (~/.local/state/harness/ and
# ~/.config/harness/) in place. Pass --purge to also delete them.

set -euo pipefail

PURGE=0
for arg in "$@"; do
  case "$arg" in
    --purge) PURGE=1 ;;
    -h|--help)
      sed -n '3,10p' "$0"
      exit 0 ;;
    *) echo "unknown flag: $arg" >&2; exit 2 ;;
  esac
done

say() { printf '\033[1;34m[uninstall]\033[0m %s\n' "$*"; }

say "Stopping timers and services"
systemctl --user stop russell-sentinel.timer  2>/dev/null || true
systemctl --user stop russell-digest.timer    2>/dev/null || true
systemctl --user stop russell-acp-server.service 2>/dev/null || true

say "Disabling timers and services"
systemctl --user disable russell-sentinel.timer 2>/dev/null || true
systemctl --user disable russell-digest.timer   2>/dev/null || true
systemctl --user disable russell-acp-server.service 2>/dev/null || true

say "Removing systemd user units"
for u in russell-sentinel.timer russell-sentinel.service \
         russell-digest.timer   russell-digest.service \
         russell-failure@.service \
         russell-acp-server.service; do
  rm -f "$HOME/.config/systemd/user/$u"
done
systemctl --user daemon-reload

say "Removing binaries"
rm -f "$HOME/.local/bin/russell"
rm -f "$HOME/.local/bin/russell-acp-server"
rm -f "$HOME/.cargo/bin/russell"
rm -f "$HOME/.cargo/bin/russell-acp-server"

if [ "$PURGE" -eq 1 ]; then
  say "Purging data + config (destructive)"
  rm -rf "$HOME/.local/state/harness"
  rm -rf "$HOME/.local/share/harness"
  rm -rf "$HOME/.config/harness"
else
  say "Data preserved at ~/.local/state/harness/ (pass --purge to remove)"
fi

say "Done."

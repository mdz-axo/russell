#!/usr/bin/env bash
# =============================================================================
# Fix Health Check Issues — 2026-05-07
# =============================================================================
# Addresses the 6 issues reported by system-health-check.sh:
#
#   1. ⚠ Swap 901MB in use         → clear swap (62G RAM free, safe)
#   2. ⚠ Docker daemon not reachable → see fix-container-runtime.sh (Podman setup)
#   3. ⚠ System Node packages       → leave as CLI fallback (intentional)
#   4. ✗ Broken symlinks            → remove dead corepack symlinks
#   5. ⚠ Journal 196M              → vacuum
#   6. ⚠ Duplicate /snap/bin       → remove from /etc/environment
#   +  Puppeteer cache 581MB       → remove (not used)
#
# Usage: bash ~/Clones/russell/scripts/fix-health-check-issues.sh
# Requires: sudo (will prompt)
# =============================================================================
set -euo pipefail

GRN='\033[0;32m'; CYN='\033[0;36m'; YEL='\033[1;33m'; BLD='\033[1m'; RST='\033[0m'
ok()   { echo -e "  ${GRN}✓${RST} $1"; }
info() { echo -e "  ${CYN}ℹ${RST} $1"; }
warn() { echo -e "  ${YEL}⚠${RST} $1"; }

echo -e "${BLD}"
echo "  ┌─────────────────────────────────────────────────────┐"
echo "  │  Health Check Fixes — $(date +%Y-%m-%d)                      │"
echo "  └─────────────────────────────────────────────────────┘"
echo -e "${RST}"

# ─── 1. Clear swap ──────────────────────────────────────────────────────────
echo -e "\n${CYN}━━━ 1. Clear swap${RST}"
SWAP_USED=$(free -m | awk '/Swap/{print $3}')
if [ "${SWAP_USED:-0}" -gt 100 ]; then
    info "Swap in use: ${SWAP_USED}MB — clearing (62G RAM free, safe)"
    sudo swapoff -a && sudo swapon -a
    ok "Swap cleared"
else
    ok "Swap usage already low (${SWAP_USED}MB)"
fi

# ─── 2. Container runtime ───────────────────────────────────────────────────
echo -e "\n${CYN}━━━ 2. Container runtime${RST}"
info "Handled separately — run: bash ~/Clones/russell/scripts/fix-container-runtime.sh"

# ─── 3. System Node packages ────────────────────────────────────────────────
echo -e "\n${CYN}━━━ 3. System Node packages${RST}"
info "Kept intentionally as CLI fallback (not removing)"

# ─── 4. Remove broken symlinks (dead corepack references) ───────────────────
echo -e "\n${CYN}━━━ 4. Remove broken symlinks in ~/.local/bin${RST}"
BROKEN=0
for link in ~/.local/bin/yarnpkg ~/.local/bin/pnpm ~/.local/bin/yarn ~/.local/bin/pnpx; do
    if [ -L "$link" ] && [ ! -e "$link" ]; then
        rm "$link"
        ok "Removed broken symlink: $(basename "$link")"
        BROKEN=$((BROKEN+1))
    fi
done
if [ "$BROKEN" -eq 0 ]; then
    ok "No broken symlinks found (already clean)"
fi

# ─── 5. Vacuum systemd journal ──────────────────────────────────────────────
echo -e "\n${CYN}━━━ 5. Vacuum systemd journal${RST}"
JOURNAL_SIZE=$(journalctl --disk-usage 2>/dev/null | grep -oP '[\d.]+[MG]' | head -1)
info "Current journal size: ${JOURNAL_SIZE}"
sudo journalctl --vacuum-size=100M
ok "Journal vacuumed to 100M max"

# ─── 6. Fix duplicate /snap/bin in PATH ─────────────────────────────────────
echo -e "\n${CYN}━━━ 6. Fix duplicate /snap/bin in PATH${RST}"
# /etc/environment has /snap/bin hardcoded AND /etc/profile.d/apps-bin-path.sh
# adds it conditionally. Remove from /etc/environment to eliminate the duplicate.
if grep -q ":/snap/bin" /etc/environment 2>/dev/null; then
    sudo sed -i 's|:/snap/bin||' /etc/environment
    ok "Removed /snap/bin from /etc/environment (profile.d still adds it)"
    info "Takes effect on next login"
else
    ok "/snap/bin not duplicated in /etc/environment"
fi

# ─── 7. Remove puppeteer cache (not used) ───────────────────────────────────
echo -e "\n${CYN}━━━ 7. Remove puppeteer cache${RST}"
if [ -d "${HOME}/.cache/puppeteer" ]; then
    SIZE=$(du -sh "${HOME}/.cache/puppeteer" | awk '{print $1}')
    rm -rf "${HOME}/.cache/puppeteer"
    ok "Removed ~/.cache/puppeteer (${SIZE} freed)"
else
    ok "No puppeteer cache found"
fi

# ─── Summary ────────────────────────────────────────────────────────────────
echo ""
echo -e "${BLD}  Done. Re-run system-health-check.sh to verify.${RST}"
echo ""
echo "  Remaining action:"
echo "    bash ~/Clones/russell/scripts/fix-container-runtime.sh"
echo ""

#!/usr/bin/env bash
# package-checker: npm-install intervention
# Installs an npm package globally. Satisfies IDRS:
#   I — Idempotent: npm install -g is idempotent
#   D — Dry-run: RUSSELL_DRY_RUN=1 shows what would happen
#   R — Rollback: npm-uninstall reverses this
#   S — Structured log: emits event record

set -euo pipefail

PACKAGE="${1:-}"

if [[ -z "$PACKAGE" ]]; then
    echo "Usage: npm-install.sh <package-name>" >&2
    echo "Example: npm-install.sh cline" >&2
    exit 1
fi

# Validate package name — npm rejects special characters
if [[ "$PACKAGE" =~ [~\'\!\(\)\*] ]]; then
    echo "Error: Package name '$PACKAGE' contains invalid characters." >&2
    echo "npm package names cannot contain: ~ ' ! ( ) *" >&2
    echo "Did you mean '${PACKAGE%%[~\'\!\(\)\*]*}'?" >&2
    exit 1
fi

# Dry-run support
DRY_RUN="${RUSSELL_DRY_RUN:-0}"

if [[ "$DRY_RUN" == "1" ]]; then
    echo "[DRY RUN] Would install: npm install -g ${PACKAGE}"
    echo "[DRY RUN] No changes made."
    exit 0
fi

# Check if running as root (needs_sudo: true)
if [[ $EUID -ne 0 ]]; then
    echo "Error: This script must run as root (via sudo)" >&2
    exit 1
fi

# Check npm available
if ! command -v npm &>/dev/null; then
    echo "Error: npm is not installed" >&2
    echo "Install it first with: apt install -y npm" >&2
    exit 1
fi

# Check if already installed
already_installed=$(npm list -g --depth=0 2>/dev/null | grep -E "^[├└]─+ ${PACKAGE}@" || true)
if [[ -n "$already_installed" ]]; then
    current_version=$(echo "$already_installed" | sed -E 's/.*@([0-9][^ ]*).*/\1/')
    echo "Already installed: ${PACKAGE}@${current_version}"
    echo "Reinstalling to ensure latest version..."
fi

# Pre-state capture (for rollback)
echo "Pre-install state:"
npm list -g --depth=0 2>/dev/null | grep -E "^${PACKAGE}@" || echo "  ${PACKAGE}: not previously installed"
echo ""

# Install
echo "Installing ${PACKAGE} globally..."
echo ""

if npm install -g "$PACKAGE" 2>&1; then
    installed_version=$(npm list -g --depth=0 2>/dev/null | grep -E "^[├└]─+ ${PACKAGE}@" | sed -E 's/.*@([0-9][^ ]*).*/\1/' || echo "?")
    echo ""
    echo "✓ Successfully installed: ${PACKAGE}@${installed_version}"
    echo ""
    # Structured log event
    echo "EVENT: npm-install package=${PACKAGE} version=${installed_version} timestamp=$(date -Iseconds)"
    exit 0
else
    echo ""
    echo "✗ Failed to install: ${PACKAGE}" >&2
    exit 3
fi

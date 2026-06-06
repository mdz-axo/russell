#!/usr/bin/env bash
# package-checker: check-version probe
# Reports the version of a specific package, searching across all available
# package managers. If a manager is specified as second arg, only that manager
# is checked.

set -euo pipefail

PACKAGE="${1:-}"
MANAGER="${2:-auto}"  # auto, apt, npm, snap, pip

if [[ -z "$PACKAGE" ]]; then
    echo "Usage: check-version.sh <package-name> [manager]" >&2
    echo "  manager: auto (default), apt, npm, snap, pip" >&2
    echo "Example: check-version.sh cline npm" >&2
    exit 1
fi

# Validate package name for npm
if [[ "$PACKAGE" =~ [~\'\!\(\)\*] ]]; then
    cleaned="${PACKAGE%%[~\'\!\(\)\*]*}"
    echo "Warning: '$PACKAGE' contains invalid characters for npm (~ ' ! ( ) *)" >&2
    echo "Using '${cleaned}' instead" >&2
    PACKAGE="$cleaned"
    echo ""
fi

found=0

# --- apt/dpkg ---
if [[ "$MANAGER" == "auto" || "$MANAGER" == "apt" ]]; then
    if command -v dpkg &>/dev/null; then
        if dpkg-query -W -f='${Status}' "$PACKAGE" 2>/dev/null | grep -q "install ok installed"; then
            VERSION=$(dpkg-query -W -f='${Version}' "$PACKAGE")
            echo "[apt] Installed: $PACKAGE=$VERSION"
            found=1
        elif [[ "$MANAGER" == "apt" ]]; then
            echo "[apt] Not installed: $PACKAGE"
            found=1
        fi
    fi
fi

# --- npm ---
if [[ "$MANAGER" == "auto" || "$MANAGER" == "npm" ]]; then
    if command -v npm &>/dev/null; then
        installed=$(npm list -g --depth=0 2>/dev/null | grep -E "^[├└]─+ ${PACKAGE}@" || true)
        if [[ -n "$installed" ]]; then
            version=$(echo "$installed" | sed -E 's/.*@([0-9][^ ]*).*/\1/')
            echo "[npm] Installed (global): ${PACKAGE}@${version}"
            found=1
        elif [[ "$MANAGER" == "npm" ]]; then
            echo "[npm] Not installed (global): $PACKAGE"
            found=1
        fi
    fi
fi

# --- snap ---
if [[ "$MANAGER" == "auto" || "$MANAGER" == "snap" ]]; then
    if command -v snap &>/dev/null; then
        snap_info=$(snap list "$PACKAGE" 2>/dev/null || true)
        if [[ -n "$snap_info" ]]; then
            version=$(echo "$snap_info" | awk 'NR==2 {print $2}')
            echo "[snap] Installed: ${PACKAGE} ${version}"
            found=1
        elif [[ "$MANAGER" == "snap" ]]; then
            echo "[snap] Not installed: $PACKAGE"
            found=1
        fi
    fi
fi

# --- pip ---
if [[ "$MANAGER" == "auto" || "$MANAGER" == "pip" ]]; then
    PIP_CMD=""
    if command -v pip3 &>/dev/null; then
        PIP_CMD="pip3"
    elif command -v pip &>/dev/null; then
        PIP_CMD="pip"
    fi
    if [[ -n "$PIP_CMD" ]]; then
        installed=$($PIP_CMD show "$PACKAGE" 2>/dev/null || true)
        if [[ -n "$installed" ]]; then
            version=$(echo "$installed" | grep "^Version:" | awk '{print $2}')
            echo "[pip] Installed: ${PACKAGE}==${version}"
            found=1
        elif [[ "$MANAGER" == "pip" ]]; then
            echo "[pip] Not installed: $PACKAGE"
            found=1
        fi
    fi
fi

if [[ "$found" -eq 0 ]]; then
    echo "Not installed: $PACKAGE (checked: apt, npm, snap, pip)"
fi

exit 0

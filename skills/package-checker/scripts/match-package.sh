#!/usr/bin/env bash
# package-checker: match-package probe
# Fuzzy-matches a pattern against installed packages across ALL available
# package managers (apt, npm, snap, pip).

set -euo pipefail

PATTERN="${1:-}"

if [[ -z "$PATTERN" ]]; then
    echo "Usage: match-package.sh <pattern>" >&2
    echo "Example: match-package.sh cline" >&2
    exit 1
fi

# Validate: strip obvious special characters and warn
if [[ "$PATTERN" =~ [~\'\!\(\)\*] ]]; then
    cleaned="${PATTERN%%[~\'\!\(\)\*]*}"
    echo "Warning: '$PATTERN' contains invalid package name characters (~ ' ! ( ) *)" >&2
    echo "Searching for '${cleaned}' instead..." >&2
    PATTERN="$cleaned"
    echo ""
fi

echo "Searching for packages matching '$PATTERN' across all package managers..."
echo ""

found_any=0

# --- apt/dpkg ---
if command -v apt &>/dev/null; then
    matches=$(apt list --installed 2>/dev/null | grep -i "$PATTERN" || true)
    if [[ -n "$matches" ]]; then
        found_any=1
        count=$(echo "$matches" | wc -l)
        echo "[apt] Found $count matching package(s):"
        echo "$matches" | while read -r line; do
            package=$(echo "$line" | cut -d'/' -f1)
            version=$(echo "$line" | awk '{print $2}')
            printf "  %-40s %s\n" "$package" "$version"
        done
        echo ""
    fi
fi

# --- npm (global) ---
if command -v npm &>/dev/null; then
    matches=$(npm list -g --depth=0 2>/dev/null | grep -i "$PATTERN" || true)
    if [[ -n "$matches" ]]; then
        found_any=1
        count=$(echo "$matches" | wc -l)
        echo "[npm] Found $count matching global package(s):"
        echo "$matches" | while read -r line; do
            # Strip tree characters
            clean=$(echo "$line" | sed 's/^[├└│ ──]*//')
            printf "  %s\n" "$clean"
        done
        echo ""
    fi
fi

# --- snap ---
if command -v snap &>/dev/null; then
    matches=$(snap list 2>/dev/null | grep -i "$PATTERN" || true)
    if [[ -n "$matches" ]]; then
        found_any=1
        count=$(echo "$matches" | wc -l)
        echo "[snap] Found $count matching package(s):"
        echo "$matches" | while read -r line; do
            printf "  %s\n" "$line"
        done
        echo ""
    fi
fi

# --- pip ---
PIP_CMD=""
if command -v pip3 &>/dev/null; then
    PIP_CMD="pip3"
elif command -v pip &>/dev/null; then
    PIP_CMD="pip"
fi
if [[ -n "$PIP_CMD" ]]; then
    matches=$($PIP_CMD list 2>/dev/null | grep -i "$PATTERN" || true)
    if [[ -n "$matches" ]]; then
        found_any=1
        count=$(echo "$matches" | wc -l)
        echo "[pip] Found $count matching package(s):"
        echo "$matches" | while read -r line; do
            printf "  %s\n" "$line"
        done
        echo ""
    fi
fi

if [[ "$found_any" -eq 0 ]]; then
    echo "No installed packages found matching '$PATTERN' across any package manager."
    echo ""
    echo "Tip: Try a shorter pattern, check spelling, or use a manager-specific probe:"
    echo "  npm-check-version <package>"
    echo "  snap-check-version <package>"
    echo "  pip-check-version <package>"
fi

echo ""
exit 0

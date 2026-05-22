#!/usr/bin/env bash
# package-checker: match-package probe
# Fuzzy-matches a pattern against installed Debian packages.

set -euo pipefail

PATTERN="${1:-}"

if [[ -z "$PATTERN" ]]; then
    echo "Usage: match-package.sh <pattern>" >&2
    echo "Example: match-package.sh ollama" >&2
    exit 1
fi

# Search installed packages (case-insensitive)
echo "Searching for packages matching '$PATTERN'..."
echo ""

matches=$(apt list --installed 2>/dev/null | grep -i "$PATTERN" || true)

if [[ -z "$matches" ]]; then
    echo "No installed packages found matching '$PATTERN'"
    echo ""
    echo "Tip: Try a shorter pattern or check spelling."
    exit 0
fi

echo "Found $(echo "$matches" | wc -l) matching package(s):"
echo "$matches" | while read -r line; do
    package=$(echo "$line" | cut -d'/' -f1)
    version=$(echo "$line" | awk '{print $2}')
    printf "  %-40s %s\n" "$package" "$version"
done

echo ""
exit 0

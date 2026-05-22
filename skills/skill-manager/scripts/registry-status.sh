#!/usr/bin/env bash
# skill-manager: registry-status probe
# Shows the status of the skill registry cache.

set -euo pipefail

REGISTRY_FILE="${RUSSELL_REGISTRY_FILE:-$HOME/.local/share/harness/registry/local-cache.yaml}"

echo "Registry status:"
echo ""

if [[ ! -f "$REGISTRY_FILE" ]]; then
    echo "  Registry file: $REGISTRY_FILE"
    echo "  Status: NOT FOUND"
    echo ""
    echo "  The registry will be created when you install your first skill."
    echo "  Run: russell skill install <skill-name>"
    exit 0
fi

echo "  Registry file: $REGISTRY_FILE"
echo "  Status: EXISTS"

# Count skills by status
installed=$(grep -c "status: installed" "$REGISTRY_FILE" 2>/dev/null || echo "0")
deprecated=$(grep -c "status: deprecated" "$REGISTRY_FILE" 2>/dev/null || echo "0")
retired=$(grep -c "status: retired" "$REGISTRY_FILE" 2>/dev/null || echo "0")
discovered=$(grep -c "status: discovered" "$REGISTRY_FILE" 2>/dev/null || echo "0")
evaluated=$(grep -c "status: evaluated" "$REGISTRY_FILE" 2>/dev/null || echo "0")

echo ""
echo "  Skills by status:"
echo "    installed:  $installed"
echo "    deprecated: $deprecated"
echo "    retired:    $retired"
echo "    discovered: $discovered"
echo "    evaluated:  $evaluated"

total=$((installed + deprecated + retired + discovered + evaluated))
echo ""
echo "  Total: $total skills in registry"

# File size
size=$(wc -c < "$REGISTRY_FILE")
echo "  File size: $size bytes"

# Last modified
modified=$(stat -c %y "$REGISTRY_FILE" 2>/dev/null || stat -f %Sm "$REGISTRY_FILE" 2>/dev/null || echo "unknown")
echo "  Last modified: $modified"

echo ""
exit 0

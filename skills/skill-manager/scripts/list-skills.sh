#!/usr/bin/env bash
# skill-manager: list-skills probe
# Lists all installed skills with their lifecycle status.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SKILL_DIR="$(dirname "$SCRIPT_DIR")"

# Get the skills directory from environment or default
SKILLS_DIR="${RUSSELL_SKILLS_DIR:-$HOME/.local/share/harness/skills}"
REGISTRY_FILE="${RUSSELL_REGISTRY_FILE:-$HOME/.local/share/harness/registry/local-cache.yaml}"

echo "Installed skills in $SKILLS_DIR:"
echo ""

if [[ ! -d "$SKILLS_DIR" ]]; then
    echo "  (no skills directory found)"
    exit 0
fi

# Count skills
count=0
for skill_dir in "$SKILLS_DIR"/*/; do
    if [[ -f "${skill_dir}manifest.yaml" ]]; then
        skill_name=$(basename "$skill_dir")
        
        # Try to get status from registry
        status="unknown"
        version="?"
        if [[ -f "$REGISTRY_FILE" ]]; then
            # Simple grep-based extraction (proper YAML parsing would require yq)
            status_line=$(grep -A5 "^  ${skill_name}:" "$REGISTRY_FILE" 2>/dev/null | grep "status:" | head -1 || true)
            if [[ -n "$status_line" ]]; then
                status=$(echo "$status_line" | sed 's/.*status: *//' | tr -d ' ')
            fi
            version_line=$(grep -A5 "^  ${skill_name}:" "$REGISTRY_FILE" 2>/dev/null | grep "version:" | head -1 || true)
            if [[ -n "$version_line" ]]; then
                version=$(echo "$version_line" | sed 's/.*version: *//')
            fi
        fi
        
        # Extract version from manifest if registry doesn't have it
        if [[ "$version" == "?" ]]; then
            version=$(grep "^version:" "${skill_dir}manifest.yaml" 2>/dev/null | sed 's/version: *//' || echo "?")
        fi
        
        printf "  %-30s v%-10s [%s]\n" "$skill_name" "$version" "$status"
        ((count++)) || true
    fi
done

if [[ $count -eq 0 ]]; then
    echo "  (no skills with valid manifests)"
else
    echo ""
    echo "Total: $count skills"
fi

exit 0

#!/usr/bin/env bash
# skill-manager: skill-check probe
# Audits all skills for issues.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SKILL_DIR="$(dirname "$SCRIPT_DIR")"

SKILLS_DIR="${RUSSELL_SKILLS_DIR:-$HOME/.local/share/harness/skills}"
REGISTRY_FILE="${RUSSELL_REGISTRY_FILE:-$HOME/.local/share/harness/registry/local-cache.yaml}"

issues=()

echo "Auditing skills..."
echo ""

# Check each skill in registry
if [[ -f "$REGISTRY_FILE" ]]; then
    current_skill=""
    while IFS= read -r line; do
        if [[ "$line" =~ ^[[:space:]]{2}[a-z][-a-z]*:$ ]]; then
            current_skill=$(echo "$line" | sed 's/^[[:space:]]*//' | tr -d ':')
        fi
        
        if [[ -n "$current_skill" ]]; then
            skill_dir="$SKILLS_DIR/$current_skill"
            
            # Check if manifest exists for installed skills
            if [[ "$line" == *"status: installed"* ]] || [[ "$line" == *"status: active"* ]]; then
                if [[ ! -f "$skill_dir/manifest.yaml" ]]; then
                    issues+=("$current_skill: marked as installed but manifest missing")
                fi
            fi
            
            # Check for recent failures
            if [[ "$line" == *"recent_probe_failures: "* ]]; then
                failures=$(echo "$line" | sed 's/.*recent_probe_failures: *//')
                if [[ "$failures" -gt 0 ]]; then
                    issues+=("$current_skill: $failures recent probe failures")
                fi
            fi
            
            if [[ "$line" == *"recent_intervention_failures: "* ]]; then
                failures=$(echo "$line" | sed 's/.*recent_intervention_failures: *//')
                if [[ "$failures" -gt 0 ]]; then
                    issues+=("$current_skill: $failures recent intervention failures")
                fi
            fi
        fi
    done < "$REGISTRY_FILE"
fi

# Check for skills on disk but not in registry
if [[ -d "$SKILLS_DIR" ]]; then
    for skill_dir in "$SKILLS_DIR"/*/; do
        if [[ -f "${skill_dir}manifest.yaml" ]]; then
            skill_name=$(basename "$skill_dir")
            if ! grep -q "^  ${skill_name}:" "$REGISTRY_FILE" 2>/dev/null; then
                issues+=("$skill_name: on disk but not in registry (run 'russell skill install $skill_name')")
            fi
        fi
    done
fi

if [[ ${#issues[@]} -eq 0 ]]; then
    echo "✓ All skills healthy"
else
    echo "Found ${#issues[@]} issue(s):"
    for issue in "${issues[@]}"; do
        echo "  ✗ $issue"
    done
fi

echo ""
exit 0

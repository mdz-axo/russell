#!/usr/bin/env bash
# skill-manager: skill-stats probe
# Shows telemetry statistics for all skills.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

REGISTRY_FILE="${RUSSELL_REGISTRY_FILE:-$HOME/.local/share/harness/registry/local-cache.yaml}"

if [[ ! -f "$REGISTRY_FILE" ]]; then
    echo "No registry file found at $REGISTRY_FILE"
    echo "Run some skills first to generate telemetry."
    exit 0
fi

echo "Skill telemetry from $REGISTRY_FILE:"
echo ""
printf "  %-30s %-10s %-8s %-8s %-10s %-10s\n" "SKILL" "STATUS" "PROBES" "FAILS" "INTERVS" "I-FAILS"
printf "  %-30s %-10s %-8s %-8s %-10s %-10s\n" "-----" "------" "------" "-----" "-------" "-------"

# Parse registry YAML (simple grep-based approach)
current_skill=""
while IFS= read -r line; do
    # Check for skill entry start
    if [[ "$line" =~ ^[[:space:]]{2}[a-z][-a-z]*:$ ]]; then
        current_skill=$(echo "$line" | sed 's/^[[:space:]]*//' | tr -d ':')
    fi
    
    # Extract fields for current skill
    if [[ -n "$current_skill" ]]; then
        case "$line" in
            *"status:"*)
                status=$(echo "$line" | sed 's/.*status: *//' | tr -d ' ')
                ;;
            *"probe_runs:"*)
                probe_runs=$(echo "$line" | sed 's/.*probe_runs: *//')
                ;;
            *"recent_probe_failures:"*)
                recent_probe_failures=$(echo "$line" | sed 's/.*recent_probe_failures: *//')
                ;;
            *"intervention_runs:"*)
                intervention_runs=$(echo "$line" | sed 's/.*intervention_runs: *//')
                ;;
            *"recent_intervention_failures:"*)
                recent_intervention_failures=$(echo "$line" | sed 's/.*recent_intervention_failures: *//')
                # Print the row
                printf "  %-30s %-10s %-8s %-8s %-10s %-10s\n" \
                    "$current_skill" "$status" "$probe_runs" "$recent_probe_failures" "$intervention_runs" "$recent_intervention_failures"
                current_skill=""
                ;;
        esac
    fi
done < "$REGISTRY_FILE"

echo ""
exit 0

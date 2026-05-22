#!/usr/bin/env bash
# skill-manager: install-skill intervention
# Installs/activates a skill by updating the registry cache.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SKILL_DIR="$(dirname "$SCRIPT_DIR")"

SKILL_NAME="${1:-}"
SKILLS_DIR="${RUSSELL_SKILLS_DIR:-$HOME/.local/share/harness/skills}"
REGISTRY_FILE="${RUSSELL_REGISTRY_FILE:-$HOME/.local/share/harness/registry/local-cache.yaml}"
JOURNAL_FILE="${RUSSELL_JOURNAL_FILE:-$HOME/.local/share/harness/journal.db}"

if [[ -z "$SKILL_NAME" ]]; then
    echo "Usage: install-skill.sh <skill-name>" >&2
    exit 1
fi

TARGET_DIR="$SKILLS_DIR/$SKILL_NAME"

if [[ ! -d "$TARGET_DIR" ]]; then
    echo "Error: Skill '$SKILL_NAME' not found at $TARGET_DIR" >&2
    echo "Run 'russell skill build $SKILL_NAME' to create it first." >&2
    exit 1
fi

if [[ ! -f "$TARGET_DIR/manifest.yaml" ]]; then
    echo "Error: manifest.yaml missing in $TARGET_DIR" >&2
    exit 1
fi

# Ensure registry directory exists
mkdir -p "$(dirname "$REGISTRY_FILE")"

# Get version from manifest
VERSION=$(grep "^version:" "$TARGET_DIR/manifest.yaml" | sed 's/version: *//' | head -1)
if [[ -z "$VERSION" ]]; then
    VERSION="0.1.0"
fi

TODAY=$(date +%Y-%m-%d)

# Check if skill is already in registry (use word-boundary match to avoid partial matches)
if [[ -f "$REGISTRY_FILE" ]] && grep -qE "^  ${SKILL_NAME}:" "$REGISTRY_FILE" 2>/dev/null; then
    # Check current status
    status=$(grep -A5 "^  ${SKILL_NAME}:" "$REGISTRY_FILE" 2>/dev/null | grep "status:" | head -1 | sed 's/.*status: *//' | tr -d ' ')
    
    if [[ "$status" == "installed" ]]; then
        echo "Skill '$SKILL_NAME' is already installed (v$VERSION)"
        exit 2
    fi
    
    echo "Updating registry: $SKILL_NAME → installed"
    
    # Atomic update with backup
    cp "$REGISTRY_FILE" "${REGISTRY_FILE}.bak.$$"
    
    # Create updated registry (use word-boundary match)
    awk -v skill="$SKILL_NAME" -v status="installed" -v today="$TODAY" '
    BEGIN { in_skill = 0 }
    /^  [a-z][a-z0-9-]*:/ {
        if (in_skill) { in_skill = 0 }
        if ($0 ~ "^  " skill ":") { in_skill = 1 }
    }
    in_skill && /status:/ {
        sub(/status: *[a-z]+/, "status: " status)
    }
    in_skill && /installed:/ {
        sub(/installed: *[0-9-]+/, "installed: " today)
    }
    { print }
    ' "$REGISTRY_FILE" > "${REGISTRY_FILE}.new.$$"
    
    # Atomic move with verification
    if ! mv "${REGISTRY_FILE}.new.$$" "$REGISTRY_FILE"; then
        echo "Error: Failed to update registry" >&2
        rm -f "${REGISTRY_FILE}.new.$$"
        exit 3
    fi
    rm -f "${REGISTRY_FILE}.bak.$$"
else
    # Add new entry to registry
    echo "Adding skill to registry: $SKILL_NAME (v$VERSION)"
    
    cat >> "$REGISTRY_FILE" << EOF
  $SKILL_NAME:
    status: installed
    version: $VERSION
    installed: $TODAY
    evaluated: $TODAY
    source: manual
    probe_runs: 0
    recent_probe_failures: 0
    intervention_runs: 0
    recent_intervention_failures: 0
    avg_probe_duration_ms: null
    last_probe_run_at: null
    coverage_score: null
EOF
fi

# Journal the transition (if journal exists)
if command -v sqlite3 &>/dev/null && [[ -f "$JOURNAL_FILE" ]]; then
    ts=$(date +%s)
    ts_iso=$(date -Iseconds)
    sqlite3 "$JOURNAL_FILE" <<EOF
INSERT INTO events (ts_unix, ts, scope, module, action, severity, summary)
VALUES ($ts, '$ts_iso', 'skill', 'skill-manager', 'install', 'info', 'Installed skill $SKILL_NAME v$VERSION');
EOF
fi

echo "Installed skill: $SKILL_NAME (v$VERSION)"
echo "  Status: active (will be loaded on next Russell start)"
echo "  Registry: $REGISTRY_FILE"

exit 0

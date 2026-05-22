#!/usr/bin/env bash
# skill-manager: restore-from-archive intervention
# Restores a retired skill from archive.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

SKILL_NAME="${1:-}"
SKILLS_DIR="${RUSSELL_SKILLS_DIR:-$HOME/.local/share/harness/skills}"
REGISTRY_FILE="${RUSSELL_REGISTRY_FILE:-$HOME/.local/share/harness/registry/local-cache.yaml}"
JOURNAL_FILE="${RUSSELL_JOURNAL_FILE:-$HOME/.local/share/harness/journal.db}"

if [[ -z "$SKILL_NAME" ]]; then
    echo "Usage: restore-from-archive.sh <skill-name>" >&2
    exit 1
fi

ARCHIVE_DIR="${SKILLS_DIR}/../archive"
ARCHIVED_SKILL_DIR="$ARCHIVE_DIR/$SKILL_NAME"
TARGET_DIR="$SKILLS_DIR/$SKILL_NAME"

if [[ ! -d "$ARCHIVED_SKILL_DIR" ]]; then
    echo "Error: Skill '$SKILL_NAME' not found in archive at $ARCHIVED_SKILL_DIR" >&2
    echo "Run 'russell skill put' to list archived skills." >&2
    exit 1
fi

if [[ -d "$TARGET_DIR" ]]; then
    echo "Error: Skill '$SKILL_NAME' already exists at $TARGET_DIR" >&2
    exit 2
fi

echo "Restoring skill from archive: $SKILL_NAME"
echo "  From: $ARCHIVED_SKILL_DIR"
echo "  To: $TARGET_DIR"

# Move from archive to skills
mv "$ARCHIVED_SKILL_DIR" "$TARGET_DIR"

# Get version from manifest
VERSION=$(grep "^version:" "$TARGET_DIR/manifest.yaml" 2>/dev/null | sed 's/version: *//' | head -1)
if [[ -z "$VERSION" ]]; then
    VERSION="0.1.0"
fi

TODAY=$(date +%Y-%m-%d)

# Ensure registry directory exists
mkdir -p "$(dirname "$REGISTRY_FILE")"

# Add entry to registry
if [[ -f "$REGISTRY_FILE" ]] && grep -q "^  ${SKILL_NAME}:" "$REGISTRY_FILE" 2>/dev/null; then
    # Update existing entry
    cp "$REGISTRY_FILE" "${REGISTRY_FILE}.bak"
    
    awk -v skill="$SKILL_NAME" -v today="$TODAY" '
    BEGIN { in_skill = 0 }
    /^  [a-z]/ {
        if (in_skill) { in_skill = 0 }
        if ($0 ~ "^  " skill ":") { in_skill = 1 }
    }
    in_skill && /status:/ {
        sub(/status: *[a-z]+/, "status: installed")
    }
    in_skill && /installed:/ {
        sub(/installed: *[0-9-]+/, "installed: " today)
    }
    { print }
    ' "$REGISTRY_FILE" > "${REGISTRY_FILE}.new"
    mv "${REGISTRY_FILE}.new" "$REGISTRY_FILE"
    rm -f "${REGISTRY_FILE}.bak"
else
    # Add new entry
    cat >> "$REGISTRY_FILE" << EOF
  $SKILL_NAME:
    status: installed
    version: $VERSION
    installed: $TODAY
    evaluated: $TODAY
    source: archive
    probe_runs: 0
    recent_probe_failures: 0
    intervention_runs: 0
    recent_intervention_failures: 0
    avg_probe_duration_ms: null
    last_probe_run_at: null
    coverage_score: null
EOF
fi

# Journal the transition
if command -v sqlite3 &>/dev/null && [[ -f "$JOURNAL_FILE" ]]; then
    ts=$(date +%s)
    ts_iso=$(date -Iseconds)
    sqlite3 "$JOURNAL_FILE" <<EOF
INSERT INTO events (ts_unix, ts, scope, module, action, severity, summary)
VALUES ($ts, '$ts_iso', 'skill', 'skill-manager', 'restore-from-archive', 'info', 'Restored skill $SKILL_NAME from archive');
EOF
fi

echo "Restored skill: $SKILL_NAME (v$VERSION)"
echo "  Status: active (will be loaded on next Russell start)"

exit 0

#!/usr/bin/env bash
# skill-manager: prune-skill intervention
# Deprecates a skill (marks as deprecated but keeps files).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

SKILL_NAME="${1:-}"
SKILLS_DIR="${RUSSELL_SKILLS_DIR:-$HOME/.local/share/harness/skills}"
REGISTRY_FILE="${RUSSELL_REGISTRY_FILE:-$HOME/.local/share/harness/registry/local-cache.yaml}"
JOURNAL_FILE="${RUSSELL_JOURNAL_FILE:-$HOME/.local/share/harness/journal.db}"

if [[ -z "$SKILL_NAME" ]]; then
    echo "Usage: prune-skill.sh <skill-name>" >&2
    exit 1
fi

TARGET_DIR="$SKILLS_DIR/$SKILL_NAME"

if [[ ! -d "$TARGET_DIR" ]]; then
    echo "Error: Skill '$SKILL_NAME' not found at $TARGET_DIR" >&2
    exit 1
fi

if [[ ! -f "$REGISTRY_FILE" ]]; then
    echo "Error: Registry file not found at $REGISTRY_FILE" >&2
    exit 1
fi

# Check current status
status=$(grep -A5 "^  ${SKILL_NAME}:" "$REGISTRY_FILE" 2>/dev/null | grep "status:" | head -1 | sed 's/.*status: *//' | tr -d ' ')

if [[ "$status" == "deprecated" ]]; then
    echo "Skill '$SKILL_NAME' is already deprecated"
    exit 2
fi

if [[ "$status" == "retired" ]]; then
    echo "Skill '$SKILL_NAME' is already retired (use restore-from-archive to recover)"
    exit 2
fi

echo "Deprecating skill: $SKILL_NAME"
echo "  Files remain on disk at: $TARGET_DIR"
echo "  Status: $status → deprecated"

# Update registry
cp "$REGISTRY_FILE" "${REGISTRY_FILE}.bak"

awk -v skill="$SKILL_NAME" '
BEGIN { in_skill = 0 }
/^  [a-z]/ {
    if (in_skill) { in_skill = 0 }
    if ($0 ~ "^  " skill ":") { in_skill = 1 }
}
in_skill && /status:/ {
    sub(/status: *[a-z]+/, "status: deprecated")
}
{ print }
' "$REGISTRY_FILE" > "${REGISTRY_FILE}.new"
mv "${REGISTRY_FILE}.new" "$REGISTRY_FILE"
rm -f "${REGISTRY_FILE}.bak"

# Journal the transition
if command -v sqlite3 &>/dev/null && [[ -f "$JOURNAL_FILE" ]]; then
    ts=$(date +%s)
    ts_iso=$(date -Iseconds)
    sqlite3 "$JOURNAL_FILE" <<EOF
INSERT INTO events (ts_unix, ts, scope, module, action, severity, summary)
VALUES ($ts, '$ts_iso', 'skill', 'skill-manager', 'prune', 'info', 'Deprecated skill $SKILL_NAME');
EOF
fi

echo "Pruned skill: $SKILL_NAME"
echo "  To restore: russell skill restore $SKILL_NAME"

exit 0

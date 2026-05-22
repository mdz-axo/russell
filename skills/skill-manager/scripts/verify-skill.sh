#!/usr/bin/env bash
# skill-manager: verify-skill evaluation check
# Verifies that a skill exists on disk after build/install interventions.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SKILL_DIR="$(dirname "$SCRIPT_DIR")"

SKILL_NAME="${1:-}"
SKILLS_DIR="${RUSSELL_SKILLS_DIR:-$HOME/.local/share/harness/skills}"

if [[ -z "$SKILL_NAME" ]]; then
    echo "Usage: verify-skill.sh <skill-name>" >&2
    exit 1
fi

TARGET_DIR="$SKILLS_DIR/$SKILL_NAME"
MANIFEST="$TARGET_DIR/manifest.yaml"

if [[ ! -d "$TARGET_DIR" ]]; then
    echo "FAIL: Skill directory missing: $TARGET_DIR" >&2
    exit 1
fi

if [[ ! -f "$MANIFEST" ]]; then
    echo "FAIL: Manifest missing: $MANIFEST" >&2
    exit 1
fi

# Validate manifest has required fields
required_fields=("id:" "version:" "probes:" "safety:")
for field in "${required_fields[@]}"; do
    if ! grep -q "$field" "$MANIFEST"; then
        echo "FAIL: Manifest missing required field: $field" >&2
        exit 1
    fi
done

# Validate id matches directory name
manifest_id=$(grep "^id:" "$MANIFEST" | sed 's/id: *//')
if [[ "$manifest_id" != "$SKILL_NAME" ]]; then
    echo "FAIL: Manifest id '$manifest_id' doesn't match directory '$SKILL_NAME'" >&2
    exit 1
fi

echo "OK: Skill $SKILL_NAME verified"
exit 0

#!/usr/bin/env bash
# skill-manager: retire-skill intervention
# Retires a skill: archives it and removes from disk.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

SKILL_NAME="${1:-}"
SKILLS_DIR="${RUSSELL_SKILLS_DIR:-$HOME/.local/share/harness/skills}"
REGISTRY_FILE="${RUSSELL_REGISTRY_FILE:-$HOME/.local/share/harness/registry/local-cache.yaml}"
JOURNAL_FILE="${RUSSELL_JOURNAL_FILE:-$HOME/.local/share/harness/journal.db}"

if [[ -z "$SKILL_NAME" ]]; then
    echo "Usage: retire-skill.sh <skill-name>" >&2
    exit 1
fi

# Security: Verify SKILLS_DIR is under home
SKILLS_DIR=$(realpath -m "$SKILLS_DIR" 2>/dev/null || echo "$SKILLS_DIR")
HOME_DIR=$(realpath -m "$HOME" 2>/dev/null || echo "$HOME")
if [[ ! "$SKILLS_DIR" == "$HOME_DIR"* ]]; then
    echo "Error: Skills directory must be under $HOME" >&2
    exit 1
fi

TARGET_DIR="$SKILLS_DIR/$SKILL_NAME"
ARCHIVE_DIR="${SKILLS_DIR}/../archive"

# Security: Verify TARGET_DIR is under SKILLS_DIR
TARGET_DIR=$(realpath -m "$TARGET_DIR" 2>/dev/null || echo "$TARGET_DIR")
if [[ ! "$TARGET_DIR" == "$SKILLS_DIR/"* ]]; then
    echo "Error: Invalid skill path (potential path traversal)" >&2
    exit 1
fi

if [[ ! -d "$TARGET_DIR" ]]; then
    echo "Error: Skill '$SKILL_NAME' not found at $TARGET_DIR" >&2
    exit 1
fi

# Create archive directory
mkdir -p "$ARCHIVE_DIR"

# Security: Verify ARCHIVE_DIR is under parent of SKILLS_DIR
ARCHIVE_DIR=$(realpath -m "$ARCHIVE_DIR" 2>/dev/null || echo "$ARCHIVE_DIR")
if [[ ! "$ARCHIVE_DIR" == "$(dirname "$SKILLS_DIR")/"* ]]; then
    echo "Error: Invalid archive path" >&2
    exit 1
fi

echo "Retiring skill: $SKILL_NAME"
echo "  Archiving to: $ARCHIVE_DIR/$SKILL_NAME"

# Move to archive
if [[ -d "$ARCHIVE_DIR/$SKILL_NAME" ]]; then
    echo "  Removing old archive..."
    rm -rf "$ARCHIVE_DIR/$SKILL_NAME"
fi

mv "$TARGET_DIR" "$ARCHIVE_DIR/$SKILL_NAME"

# Remove from registry with atomic operation
if [[ -f "$REGISTRY_FILE" ]]; then
    cp "$REGISTRY_FILE" "${REGISTRY_FILE}.bak.$$"
    
    # Remove the skill entry from registry (word-boundary match)
    awk -v skill="$SKILL_NAME" '
    BEGIN { in_skill = 0; skip = 0 }
    /^  [a-z][a-z0-9-]*:/ {
        if (in_skill && !skip) { print prev }
        in_skill = 0
        skip = 0
        prev = ""
        if ($0 ~ "^  " skill ":") {
            in_skill = 1
            skip = 1
            next
        }
    }
    in_skill {
        if (/^  [a-z]/ || /^$/) {
            if (!skip) { print prev }
            in_skill = 0
            skip = 0
        }
        prev = $0
        next
    }
    {
        if (prev != "") { print prev }
        prev = $0
    }
    END { if (prev != "" && !skip) print prev }
    ' "$REGISTRY_FILE" > "${REGISTRY_FILE}.new.$$"
    
    if ! mv "${REGISTRY_FILE}.new.$$" "$REGISTRY_FILE"; then
        echo "Error: Failed to update registry" >&2
        rm -f "${REGISTRY_FILE}.new.$$"
        exit 3
    fi
    rm -f "${REGISTRY_FILE}.bak.$$"
fi

# Journal the transition
if command -v sqlite3 &>/dev/null && [[ -f "$JOURNAL_FILE" ]]; then
    ts=$(date +%s)
    ts_iso=$(date -Iseconds)
    sqlite3 "$JOURNAL_FILE" <<EOF
INSERT INTO events (ts_unix, ts, scope, module, action, severity, summary)
VALUES ($ts, '$ts_iso', 'skill', 'skill-manager', 'retire', 'info', 'Retired skill $SKILL_NAME (archived)');
EOF
fi

echo "Retired skill: $SKILL_NAME"
echo "  Files archived to: $ARCHIVE_DIR/$SKILL_NAME"
echo "  To restore: russell skill restore-from-archive $SKILL_NAME"

exit 0

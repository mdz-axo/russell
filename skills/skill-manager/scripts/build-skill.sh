#!/usr/bin/env bash
# skill-manager: build-skill intervention
# Creates a new skill skeleton on disk.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SKILL_DIR="$(dirname "$SCRIPT_DIR")"

SKILL_NAME="${1:-}"
SKILLS_DIR="${RUSSELL_SKILLS_DIR:-$HOME/.local/share/harness/skills}"

if [[ -z "$SKILL_NAME" ]]; then
    echo "Usage: build-skill.sh <skill-name>" >&2
    exit 1
fi

# Validate skill name (kebab-case only, no path traversal)
if ! [[ "$SKILL_NAME" =~ ^[a-z][a-z0-9-]*$ ]]; then
    echo "Error: Skill name must be kebab-case (e.g., package-checker), got: '$SKILL_NAME'" >&2
    exit 1
fi

# Security: Ensure SKILLS_DIR is under user's home
SKILLS_DIR=$(realpath -m "$SKILLS_DIR" 2>/dev/null || echo "$SKILLS_DIR")
HOME_DIR=$(realpath -m "$HOME" 2>/dev/null || echo "$HOME")
if [[ ! "$SKILLS_DIR" == "$HOME_DIR"* ]]; then
    echo "Error: Skills directory must be under $HOME" >&2
    exit 1
fi

TARGET_DIR="$SKILLS_DIR/$SKILL_NAME"

# Security: Verify no path traversal in TARGET_DIR
TARGET_DIR=$(realpath -m "$TARGET_DIR" 2>/dev/null || echo "$TARGET_DIR")
if [[ ! "$TARGET_DIR" == "$SKILLS_DIR/"* ]]; then
    echo "Error: Invalid skill path (potential path traversal)" >&2
    exit 1
fi

if [[ -d "$TARGET_DIR" ]]; then
    echo "Skill '$SKILL_NAME' already exists at $TARGET_DIR"
    echo "Use 'russell skill adapt $SKILL_NAME' to edit it."
    exit 2
fi

# Create directory structure
mkdir -p "$TARGET_DIR/scripts"

# Get current date
TODAY=$(date +%Y-%m-%d)

# Create manifest with valid symptom
cat > "$TARGET_DIR/manifest.yaml" << EOF
# Skill manifest for $SKILL_NAME
id: $SKILL_NAME
version: 0.1.0
authored: $TODAY
min_harness_version: 0.20.0

kind: actionable

symptoms:
  - skill_manifest_invalid

applies_when:
  - os_family: linux

probes:
  - id: health
    cmd: ["bash", "./scripts/health.sh"]
    timeout: 30s

interventions: []

safety:
  max_auto_risk: low
EOF

# Create KNOWLEDGE.md
cat > "$TARGET_DIR/KNOWLEDGE.md" << EOF
# $SKILL_NAME Knowledge

Add skill-specific knowledge here.

## Purpose

Describe what this skill does and when Jack should use it.

## Probes

- **health**: Describe what this probe checks.

## Interventions

None yet.

## Safety

This skill is safe to run with max_auto_risk: low.
EOF

# Create health probe script
cat > "$TARGET_DIR/scripts/health.sh" << 'EOF'
#!/usr/bin/env bash
# Health check probe
set -euo pipefail
echo "Skill is healthy and ready to run."
exit 0
EOF
chmod +x "$TARGET_DIR/scripts/health.sh"

echo "Created skill skeleton: $TARGET_DIR"
echo "  - manifest.yaml (edit to add probes/interventions)"
echo "  - KNOWLEDGE.md (add context for Jack)"
echo "  - scripts/health.sh (default health probe)"
echo ""
echo "Next steps:"
echo "  1. Edit manifest.yaml to define your probes and interventions"
echo "  2. Create scripts in the scripts/ directory"
echo "  3. Run: russell skill install $SKILL_NAME"

exit 0

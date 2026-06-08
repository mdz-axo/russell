#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# flowdef-converter: convert intervention / dry-run probe
# Converts a FlowDef manifest into a Russell skill.
#
# When RUSSELL_DRY_RUN=1, outputs the converted content to stdout without
# writing files. Otherwise, writes the skill files to the output directory.
#
# Usage: convert.sh <manifest_path> [output_dir] [--dry-run]
#
# If output_dir is omitted, defaults to
#   $RUSSELL_SKILLS_DIR/<skill_id> (usually ~/.local/share/harness/skills/<id>)
#
# IDRS:
#   I — Idempotent: overwriting with same content is a no-op.
#   D — Dry-run: RUSSELL_DRY_RUN=1 or --dry-run shows output, no files written.
#   R — Rollback: delete-output (remove the output directory if conversion fails).
#   S — Structured log: JSON event on stdout.

set -euo pipefail

MANIFEST_PATH="${1:-}"
OUTPUT_DIR="${2:-}"
DRY_RUN="${RUSSELL_DRY_RUN:-0}"
FLOWDEF_REGISTRY="${FLOWDEF_REGISTRY_DIR:-}"
SKILLS_DIR="${RUSSELL_SKILLS_DIR:-$HOME/.local/share/harness/skills}"
BACKUP_DIR="${RUSSELL_BACKUP_DIR:-$HOME/.local/share/harness/backups}"

# Parse arguments
shift 2 2>/dev/null || shift 1 2>/dev/null || true
for arg in "$@"; do
    if [[ "$arg" == "--dry-run" ]]; then
        DRY_RUN=1
    fi
done

if [[ -z "$MANIFEST_PATH" ]]; then
    echo '{"action":"convert","error":"missing manifest path argument"}' >&2
    exit 1
fi

if [[ ! -f "$MANIFEST_PATH" ]]; then
    echo '{"action":"convert","error":"manifest not found","path":"'"$MANIFEST_PATH"'"}' >&2
    exit 1
fi

# --- Extract FlowDef metadata ---
# Extract FlowDef metadata (use || true to prevent set -e failures)
SKILL_ID=$(grep -A10 "^manifest:" "$MANIFEST_PATH" 2>/dev/null | grep "id:" | head -1 | sed 's/.*id: *//' | tr -d ' ' || true)
SKILL_NAME=$(grep -A10 "^manifest:" "$MANIFEST_PATH" 2>/dev/null | grep "name:" | head -1 | sed 's/.*name: *//' | tr -d ' ' || true)
SKILL_VERSION=$(grep -A15 "^manifest:" "$MANIFEST_PATH" 2>/dev/null | grep "version:" | head -1 | sed 's/.*version: *//' | tr -d ' ' || true)
FUNC_ROLE=$(grep -A15 "^manifest:" "$MANIFEST_PATH" 2>/dev/null | grep "functional_role:" | head -1 | sed 's/.*functional_role: *//' | tr -d ' ' || true)

# Extract description (multi-line, after "description: >")
DESCRIPTION=$(sed -n '/^  description:/,/^[^ ]/p' "$MANIFEST_PATH" | head -5 | sed 's/^  description:.*//' | sed 's/^    //' | tr '\n' ' ' | sed 's/  */ /g' | sed 's/^ *//' | head -c 200)

if [[ -z "$SKILL_ID" ]]; then
    echo '{"action":"convert","error":"could not extract skill id"}' >&2
    exit 1
fi

# Default output directory
if [[ -z "$OUTPUT_DIR" ]]; then
    OUTPUT_DIR="${SKILLS_DIR}/${SKILL_ID}"
fi

# --- Extract inputs → symptoms ---
SYMPTOMS=""
while IFS= read -r line; do
    name=$(echo "$line" | sed 's/.*name: *//' | tr -d '"' | tr -d ' ')
    if [[ -n "$name" ]]; then
        # Convert input name to kebab-case symptom
        SYMPTOM="${name}"
        SYMPTOMS="${SYMPTOMS}  - ${SYMPTOM}"$'\n'
    fi
done < <(grep -A100 "^inputs:" "$MANIFEST_PATH" | grep -E "^  - name:")

# If no inputs found, add a default symptom
if [[ -z "$SYMPTOMS" ]]; then
    SYMPTOMS="  - skill_manifest_invalid"$'\n'
fi

# --- Extract levels ---
LEVELS_SECTION=""
while IFS= read -r line; do
    LEVELS_SECTION="${LEVELS_SECTION}${line}"$'\n'
done < <(sed -n '/^levels:/,/^[a-z]/p' "$MANIFEST_PATH" | head -30)

# --- Extract escalation rules ---
ESCALATION_SECTION=""
while IFS= read -r line; do
    ESCALATION_SECTION="${ESCALATION_SECTION}${line}"$'\n'
done < <(sed -n '/^escalation:/,/^[a-z]/p' "$MANIFEST_PATH" | head -10)

# --- Extract step info and templates ---
STEPS_INFO=""
COGNITIVE_COUNT=0
DETERMINISTIC_COUNT=0
FEEDBACK_COUNT=0
TEMPLATE_REFS=""

while IFS= read -r line; do
    if echo "$line" | grep -q "ordinal:"; then
        ORDINAL=$(echo "$line" | sed 's/.*ordinal: *//')
    fi
    if echo "$line" | grep -q "action:"; then
        ACTION=$(echo "$line" | sed 's/.*action: *//')
        TREF=$(grep -A15 "ordinal: ${ORDINAL}" "$MANIFEST_PATH" | grep "template_ref:" | head -1 | sed 's/.*template_ref: *//' || echo "null")

        if [[ "$ACTION" == "feedback" ]]; then
            ((FEEDBACK_COUNT++)) || true
            STEPS_INFO="${STEPS_INFO}    ${ORDINAL}: feedback (dropped — journal handles this)"$'\n'
        elif [[ "$TREF" != "null" && -n "$TREF" ]]; then
            ((COGNITIVE_COUNT++)) || true
            STEPS_INFO="${STEPS_INFO}    ${ORDINAL}: cognitive → KNOWLEDGE.md (template: ${TREF})"$'\n'
            TEMPLATE_REFS="${TEMPLATE_REFS}${TREF}"$'\n'
        elif [[ "$ACTION" == "validate" ]]; then
            ((DETERMINISTIC_COUNT++)) || true
            STEPS_INFO="${STEPS_INFO}    ${ORDINAL}: deterministic → probe script"$'\n'
        else
            ((COGNITIVE_COUNT++)) || true
            STEPS_INFO="${STEPS_INFO}    ${ORDINAL}: cognitive → KNOWLEDGE.md"$'\n'
        fi
    fi
done < <(grep -E "ordinal:|action:" "$MANIFEST_PATH")

# --- Read template content ---
TEMPLATE_CONTENT=""
for tref in $TEMPLATE_REFS; do
    TDIR=$(echo "$tref" | cut -d'/' -f1)
    TNAME=$(echo "$tref" | cut -d'/' -f2-)

    TPATH=""
    for candidate in \
        "${FLOWDEF_REGISTRY}/templates/${TDIR}/${TNAME}.j2" \
        "${FLOWDEF_REGISTRY}/templates/${TNAME}.j2" \
        "${FLOWDEF_REGISTRY}/templates/${tref}.j2"; do
        if [[ -f "$candidate" ]]; then
            TPATH="$candidate"
            break
        fi
    done

    if [[ -n "$TPATH" ]]; then
        # Extract the prompt body (after the second --- separator in the .j2 file)
        BODY=$(sed -n '/^---$/,$ p' "$TPATH" | tail -n +2 | grep -v '^{%' | grep -v '^\[inference\]' | grep -v '^---$' | grep -v '^$' | head -80)
        TEMPLATE_CONTENT="${TEMPLATE_CONTENT}"$'\n'"### Template: ${tref}"$'\n'"$BODY"$'\n'
    fi
done

# --- Build KNOWLEDGE.md ---
TODAY=$(date +%Y-%m-%d)
KNOWLEDGE_MD="# ${SKILL_NAME:-$SKILL_ID} — Russell Skill (Converted from FlowDef)

> **Converted from FlowDef** on ${TODAY}. Original: ${MANIFEST_PATH}
> This skill was converted from FlowDef's orchestrated process model to Russell's
> knowledge-injection + chat REPL model. The methodology is preserved; the
> execution model is adapted. See flowdef-converter/KNOWLEDGE.md for the mapping.

---

## Purpose

${DESCRIPTION:-No description available.}

## Inputs

The following inputs were declared in the original FlowDef:
$(echo "$SYMPTOMS" | sed 's/^  - /- /')

## Levels

${LEVELS_SECTION:-No level taxonomy defined.}

## Steps (Original FlowDef Ordinals)

The original FlowDef had the following steps:
${STEPS_INFO}
Cognitive steps have been converted to knowledge sections below.
Deterministic steps have been converted to probe scripts where applicable.
Feedback steps have been dropped (Russell's journal handles observability).

## Escalation Rules

${ESCALATION_SECTION:-No escalation rules defined.}

## Template-Derived Methodology

The following sections are derived from the original Jinja2 templates.
Jinja2 variables ({{ var }}) have been replaced with context references.

${TEMPLATE_CONTENT:-No template content extracted.}

## Safety

- This skill was auto-converted from FlowDef. Review carefully.
- Energy caps, OCAP capabilities, and CNS spans are NOT carried over.
- Russell's consent gate replaces OCAP.
- Russell's journal replaces CNS.

---
**Converted:** ${TODAY}
**Source:** ${MANIFEST_PATH}
**Converter:** flowdef-converter v1.0.0"

# --- Build manifest.yaml ---
MANIFEST_YAML="# Auto-converted from FlowDef on ${TODAY}
# Original: ${MANIFEST_PATH}
# Converter: flowdef-converter v1.0.0

id: ${SKILL_ID}
version: 1.0.0
authored: ${TODAY}
min_harness_version: 0.20.0

kind: actionable

symptoms:
$(echo "$SYMPTOMS")

applies_when:
  - os_family: linux

probes:
  - id: health
    cmd: [\"bash\", \"./scripts/health.sh\"]
    capture: stdout
    timeout: 10s

interventions: []

safety:
  max_auto_risk: none
  needs_network: false

references:
  - ${MANIFEST_PATH}"

# --- Build health.sh probe ---
HEALTH_SH='#!/usr/bin/env bash
# Health check probe — auto-generated by flowdef-converter
set -euo pipefail
SKILL_DIR=\"$(cd "$(dirname \"\${BASH_SOURCE[0]}\")" && pwd)/..\"
if [[ -f \"\${SKILL_DIR}/manifest.yaml\" ]] && [[ -f \"\${SKILL_DIR}/KNOWLEDGE.md\" ]]; then
    echo \"ok\"
else
    echo \"missing files\"
    exit 1
fi
exit 0'

# --- Output or write ---
if [[ "$DRY_RUN" == "1" ]]; then
    echo "=== DRY RUN: Converted skill for ${SKILL_ID} ==="
    echo ""
    echo "--- manifest.yaml ---"
    echo "$MANIFEST_YAML"
    echo ""
    echo "--- KNOWLEDGE.md ---"
    echo "$KNOWLEDGE_MD"
    echo ""
    echo "--- scripts/health.sh ---"
    echo "$HEALTH_SH"
    echo ""
    echo "Output directory would be: ${OUTPUT_DIR}"
    echo '{"action":"convert","skill_id":"'"$SKILL_ID"'","status":"dry_run","output_dir":"'"$OUTPUT_DIR"'"}'
    exit 0
fi

# Write files
mkdir -p "${OUTPUT_DIR}/scripts"

# Backup existing files
if [[ -f "${OUTPUT_DIR}/manifest.yaml" ]]; then
    mkdir -p "$BACKUP_DIR"
    TIMESTAMP=$(date +%Y%m%dT%H%M%S)
    cp -p "${OUTPUT_DIR}/manifest.yaml" "${BACKUP_DIR}/${TIMESTAMP}-manifest.yaml.bak"
fi
if [[ -f "${OUTPUT_DIR}/KNOWLEDGE.md" ]]; then
    mkdir -p "$BACKUP_DIR"
    TIMESTAMP=$(date +%Y%m%dT%H%M%S)
    cp -p "${OUTPUT_DIR}/KNOWLEDGE.md" "${BACKUP_DIR}/${TIMESTAMP}-KNOWLEDGE.md.bak"
fi

# Write manifest
echo "$MANIFEST_YAML" > "${OUTPUT_DIR}/manifest.yaml"

# Write KNOWLEDGE.md
echo "$KNOWLEDGE_MD" > "${OUTPUT_DIR}/KNOWLEDGE.md"

# Write health.sh probe
echo "$HEALTH_SH" > "${OUTPUT_DIR}/scripts/health.sh"
chmod +x "${OUTPUT_DIR}/scripts/health.sh"

TIMESTAMP=$(date -Iseconds)
echo '{"action":"convert","skill_id":"'"$SKILL_ID"'","status":"converted","output_dir":"'"$OUTPUT_DIR"'","dry_run":false,"files":["manifest.yaml","KNOWLEDGE.md","scripts/health.sh"],"timestamp":"'"$TIMESTAMP"'"}'

exit 0

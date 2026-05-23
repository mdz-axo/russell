#!/usr/bin/env bash
# check-persona-drift.sh — CI check for persona file divergence.
#
# Compares Russell's authoritative jack.md with hKask's distilled
# jack-nurse.md. Warns if they diverge beyond expected differences.
#
# Usage: ./scripts/check-persona-drift.sh [russell_dir] [hkask_dir]
# Exit 0 = in sync or acceptable drift, Exit 1 = significant drift.

set -euo pipefail

RUSSELL_DIR="${1:-.}"
HKASK_DIR="${2:-../hKask}"

RUSSELL_PERSONA="$RUSSELL_DIR/crates/russell-meta/prompts/jack.md"
HKASK_PERSONA="$HKASK_DIR/hkask-templates/personas/jack-nurse.md"

if [ ! -f "$RUSSELL_PERSONA" ]; then
    echo "ERROR: Russell persona not found at $RUSSELL_PERSONA"
    exit 1
fi

if [ ! -f "$HKASK_PERSONA" ]; then
    echo "WARN: hKask persona not found at $HKASK_PERSONA (skipping drift check)"
    exit 0
fi

RUSSELL_LINES=$(wc -l < "$RUSSELL_PERSONA")
HKASK_LINES=$(wc -l < "$HKASK_PERSONA")

echo "Russell persona: $RUSSELL_LINES lines"
echo "hKask persona:   $HKASK_LINES lines"

# hKask persona is expected to be a distilled subset (~44 lines).
# Flag if hKask persona grows beyond 50% of Russell's size (unexpected expansion)
# or shrinks below 10 lines (unexpected truncation).
RATIO=$(echo "scale=2; $HKASK_LINES * 100 / $RUSSELL_LINES" | bc)
echo "Size ratio: ${RATIO}% (hKask/Russell)"

if [ "$HKASK_LINES" -lt 10 ]; then
    echo "ERROR: hKask persona too short (< 10 lines) — possible truncation"
    exit 1
fi

# Check that key phrases from Russell's persona appear in hKask's
MISSING=0
for phrase in "JR-2" "JR-3" "SOAP" "ACTION:" "Never emit shell"; do
    if ! grep -q "$phrase" "$HKASK_PERSONA"; then
        echo "WARN: hKask persona missing key phrase: '$phrase'"
        MISSING=$((MISSING + 1))
    fi
done

if [ "$MISSING" -gt 2 ]; then
    echo "ERROR: hKask persona missing $MISSING key phrases — drift detected"
    exit 1
fi

echo "OK: Persona files in acceptable sync"
exit 0

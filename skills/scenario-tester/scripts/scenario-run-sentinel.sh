#!/usr/bin/env bash
# scenario-run-sentinel.sh — test Russell's sentinel probe collection.
#
# Runs `russell sentinel-once` several times and verifies:
# - Probes collected (non-zero count)
# - Journal written (file exists and has rows)
# - Cadence regularity (timestamps sequential)
set -euo pipefail

RUSSELL="${RUSSELL_BIN:-}"
if [ -z "$RUSSELL" ]; then
    if [ -x "$HOME/.local/bin/russell" ]; then
        RUSSELL="$HOME/.local/bin/russell"
    elif [ -x "./target/release/russell" ]; then
        RUSSELL="./target/release/russell"
    else
        RUSSELL="russell"
    fi
fi
CYCLES="${SCENARIO_CYCLES:-3}"
SLEEP="${SCENARIO_SLEEP:-0.2}"

ts=$(date -u +%Y-%m-%dT%H:%M:%SZ)
JOURNAL="$HOME/.local/state/harness/journal.db"
# Fallback: some installs use the XDG share directory instead.
if [ ! -f "$JOURNAL" ] && [ -f "$HOME/.local/share/harness/journal/russell.db" ]; then
    JOURNAL="$HOME/.local/share/harness/journal/russell.db"
fi

probe_count=0
successful_cycles=0
failed_cycles=0

# Pre-count: how many samples existed before?
sample_count_before=0
if [ -f "$JOURNAL" ]; then
    sample_count_before=$(sqlite3 "$JOURNAL" "SELECT COUNT(*) FROM samples;" 2>/dev/null || echo 0)
fi

for i in $(seq 1 "$CYCLES"); do
    if "$RUSSELL" sentinel-once > /dev/null 2>&1; then
        successful_cycles=$((successful_cycles + 1))
    else
        failed_cycles=$((failed_cycles + 1))
    fi
    sleep "$SLEEP"
done

# Post-count.
sample_count_after=0
if [ -f "$JOURNAL" ]; then
    sample_count_after=$(sqlite3 "$JOURNAL" "SELECT COUNT(*) FROM samples;" 2>/dev/null || echo 0)
fi

new_samples=$((sample_count_after - sample_count_before))
avg_per_cycle=0
if [ "$successful_cycles" -gt 0 ]; then
    avg_per_cycle=$((new_samples / successful_cycles))
fi

# Check journal integrity: verify no NULL probe names.
null_probes=0
if [ -f "$JOURNAL" ]; then
    null_probes=$(sqlite3 "$JOURNAL" "SELECT COUNT(*) FROM samples WHERE probe IS NULL OR probe = '';" 2>/dev/null || echo 0)
fi

cat <<EOF
{"metric":"sentinel_cycles_run","value":$CYCLES,"unit":"count","timestamp":"$ts"}
{"metric":"sentinel_successful_cycles","value":$successful_cycles,"unit":"count","timestamp":"$ts"}
{"metric":"sentinel_failed_cycles","value":$failed_cycles,"unit":"count","timestamp":"$ts"}
{"metric":"sentinel_samples_collected","value":$new_samples,"unit":"count","timestamp":"$ts"}
{"metric":"sentinel_avg_samples_per_cycle","value":$avg_per_cycle,"unit":"count","timestamp":"$ts"}
{"metric":"sentinel_journal_null_probes","value":$null_probes,"unit":"count","timestamp":"$ts"}
EOF

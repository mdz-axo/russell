#!/usr/bin/env bash
# scenario-journal.sh — write scenario metrics as journal samples.
#
# Reads metrics from the scenario-evaluate probe's output (piped via stdin
# or from a file) and inserts them as samples into the journal, so the
# sentinel rule engine can evaluate them against rules.d/agent-testing.toml.
#
# Metrics journaled: okapi_latency_p95_ms, okapi_error_rate_pct,
# latency_regression_pct, error_rate_regression_pp.
set -euo pipefail

JOURNAL="$HOME/.local/state/harness/journal.db"
if [ ! -f "$JOURNAL" ] && [ -f "$HOME/.local/share/harness/journal/russell.db" ]; then
    JOURNAL="$HOME/.local/share/harness/journal/russell.db"
fi

if [ ! -f "$JOURNAL" ]; then
    echo '{"status":"no_journal"}'
    exit 0
fi

ts=$(date +%s)
count=0

# Read JSON lines from stdin, write key metrics as samples.
while IFS= read -r line 2>/dev/null; do
    # Skip non-JSON lines.
    if [[ ! "$line" == \{* ]]; then
        continue
    fi

    metric=$(echo "$line" | python3 -c "import sys,json; print(json.load(sys.stdin).get('metric',''))" 2>/dev/null || echo "")
    value=$(echo "$line" | python3 -c "import sys,json; print(json.load(sys.stdin).get('value',0))" 2>/dev/null || echo 0)

    case "$metric" in
        okapi_latency_p50_ms|okapi_latency_p95_ms|okapi_latency_p99_ms|\
        okapi_throughput_rps|okapi_error_rate_pct|okapi_tokens_per_sec_p50|\
        latency_regression_pct|error_rate_regression_pp|\
        sentinel_avg_samples_per_cycle|sentinel_journal_null_probes|\
        russell_chat_error_rate_pct|russell_chat_throughput_tps)
            sqlite3 "$JOURNAL" \
                "INSERT OR REPLACE INTO samples (ts, scope, probe, value_num, unit)
                 VALUES ($ts, 'host', '$metric', $value, '');" 2>/dev/null || true
            count=$((count + 1))
            ;;
    esac
done < /dev/stdin 2>/dev/null || true

echo "{\"metric\":\"scenario_metrics_journaled\",\"value\":$count,\"unit\":\"count\",\"timestamp\":\"$(date -u +%Y-%m-%dT%H:%M:%SZ)\"}"
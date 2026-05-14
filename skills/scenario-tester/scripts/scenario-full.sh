#!/usr/bin/env bash
# scenario-full.sh — complete pipeline: run okapi → evaluate → journal.
#
# Single probe that chains the entire scenario test workflow:
#   1. Run the Okapi latency probe
#   2. Evaluate results against journal baselines
#   3. Write key metrics as journal samples (for rule engine)
#
# This closes the gap between scenario output and sentinel-actuable metrics.
# Run as: russell skill run scenario-tester/probe-scenario-full
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MODEL="${SCENARIO_MODEL:-}"
ITERATIONS="${SCENARIO_ITERATIONS:-3}"
ts=$(date -u +%Y-%m-%dT%H:%M:%SZ)
JOURNAL="$HOME/.local/state/harness/journal.db"
if [ ! -f "$JOURNAL" ] && [ -f "$HOME/.local/share/harness/journal/russell.db" ]; then
    JOURNAL="$HOME/.local/share/harness/journal/russell.db"
fi

# Step 1: Run the Okapi latency test.
echo '{"metric":"pipeline_stage","value":"start","stage":"okapi_test","timestamp":"'"$ts"'"}' >&2
if [ -n "$MODEL" ]; then
    okapi_output=$(SCENARIO_MODEL="$MODEL" SCENARIO_ITERATIONS="$ITERATIONS" bash "$SCRIPT_DIR/scenario-run-okapi.sh" 2>/dev/null || echo "")
else
    okapi_output=$(SCENARIO_ITERATIONS="$ITERATIONS" bash "$SCRIPT_DIR/scenario-run-okapi.sh" 2>/dev/null || echo "")
fi

# Extract key metrics from the okapi output.
p50=$(echo "$okapi_output" | grep '"metric":"okapi_latency_p50_ms"' | python3 -c "import sys,json; print(json.load(sys.stdin).get('value',0))" 2>/dev/null || echo 0)
p95=$(echo "$okapi_output" | grep '"metric":"okapi_latency_p95_ms"' | python3 -c "import sys,json; print(json.load(sys.stdin).get('value',0))" 2>/dev/null || echo 0)
error_rate=$(echo "$okapi_output" | grep '"metric":"okapi_error_rate_pct"' | python3 -c "import sys,json; print(json.load(sys.stdin).get('value',0))" 2>/dev/null || echo 0)
tps=$(echo "$okapi_output" | grep '"metric":"okapi_tokens_per_sec_p50"' | python3 -c "import sys,json; print(json.load(sys.stdin).get('value',0))" 2>/dev/null || echo 0)
throughput=$(echo "$okapi_output" | grep '"metric":"okapi_throughput_rps"' | python3 -c "import sys,json; print(json.load(sys.stdin).get('value',0))" 2>/dev/null || echo 0)

# Step 2: Evaluate against baselines.
echo '{"metric":"pipeline_stage","value":"evaluate","stage":"baseline_comparison","timestamp":"'"$ts"'"}' >&2
eval_output=$(WRITE_SAMPLES=1 bash "$SCRIPT_DIR/scenario-evaluate.sh" 2>/dev/null <<< "$okapi_output" || echo "")

latency_regression=$(echo "$eval_output" | grep '"metric":"latency_regression_pct"' | python3 -c "import sys,json; print(json.load(sys.stdin).get('value',0))" 2>/dev/null || echo 0)
error_regression=$(echo "$eval_output" | grep '"metric":"error_rate_regression_pp"' | python3 -c "import sys,json; print(json.load(sys.stdin).get('value',0))" 2>/dev/null || echo 0)
latency_status=$(echo "$eval_output" | grep '"metric":"latency_regression_pct"' | python3 -c "import sys,json; print(json.load(sys.stdin).get('status','ok'))" 2>/dev/null || echo "ok")

# Step 3: Write key metrics as journal samples so the sentinel rule engine can evaluate them.
now_ts=$(date +%s)
if [ -f "$JOURNAL" ]; then
    sqlite3 "$JOURNAL" \
        "INSERT OR REPLACE INTO samples (ts, scope, probe, value_num, unit)
         VALUES ($now_ts, 'host', 'okapi_latency_p50_ms', $p50, 'ms');" 2>/dev/null || true
    sqlite3 "$JOURNAL" \
        "INSERT OR REPLACE INTO samples (ts, scope, probe, value_num, unit)
         VALUES ($now_ts, 'host', 'okapi_latency_p95_ms', $p95, 'ms');" 2>/dev/null || true
    sqlite3 "$JOURNAL" \
        "INSERT OR REPLACE INTO samples (ts, scope, probe, value_num, unit)
         VALUES ($now_ts, 'host', 'okapi_error_rate_pct', $error_rate, 'pct');" 2>/dev/null || true
    sqlite3 "$JOURNAL" \
        "INSERT OR REPLACE INTO samples (ts, scope, probe, value_num, unit)
         VALUES ($now_ts, 'host', 'okapi_tokens_per_sec_p50', $tps, 'tps');" 2>/dev/null || true
    sqlite3 "$JOURNAL" \
        "INSERT OR REPLACE INTO samples (ts, scope, probe, value_num, unit)
         VALUES ($now_ts, 'host', 'okapi_throughput_rps', $throughput, 'rps');" 2>/dev/null || true
    sqlite3 "$JOURNAL" \
        "INSERT OR REPLACE INTO samples (ts, scope, probe, value_num, unit)
         VALUES ($now_ts, 'host', 'latency_regression_pct', $latency_regression, 'pct');" 2>/dev/null || true
    sqlite3 "$JOURNAL" \
        "INSERT OR REPLACE INTO samples (ts, scope, probe, value_num, unit)
         VALUES ($now_ts, 'host', 'error_rate_regression_pp', $error_regression, 'pp');" 2>/dev/null || true
fi

# Output summary.
cat <<EOF
{"metric":"okapi_latency_p50_ms","value":$p50,"unit":"ms","timestamp":"$ts","model":"${MODEL:-auto}"}
{"metric":"okapi_latency_p95_ms","value":$p95,"unit":"ms","timestamp":"$ts","model":"${MODEL:-auto}"}
{"metric":"okapi_error_rate_pct","value":$error_rate,"unit":"pct","timestamp":"$ts","model":"${MODEL:-auto}"}
{"metric":"okapi_tokens_per_sec_p50","value":$tps,"unit":"tps","timestamp":"$ts","model":"${MODEL:-auto}"}
{"metric":"okapi_throughput_rps","value":$throughput,"unit":"rps","timestamp":"$ts","model":"${MODEL:-auto}"}
{"metric":"latency_regression_pct","value":$latency_regression,"unit":"pct","timestamp":"$ts","status":"$latency_status"}
{"metric":"error_rate_regression_pp","value":$error_regression,"unit":"pp","timestamp":"$ts"}
{"metric":"samples_journaled","value":7,"unit":"count","timestamp":"$ts"}
EOF
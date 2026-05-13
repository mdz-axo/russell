#!/usr/bin/env bash
# scenario-evaluate.sh — compare scenario test results against baselines.
#
# Reads the journal for baseline metrics and compares against current values
# provided via env vars or from the most recent scenario run.
# Detects regressions: value > baseline * threshold_factor.
set -euo pipefail

JOURNAL="$HOME/.local/state/harness/journal.db"
if [ ! -f "$JOURNAL" ] && [ -f "$HOME/.local/share/harness/journal/russell.db" ]; then
    JOURNAL="$HOME/.local/share/harness/journal/russell.db"
fi
BASELINE_DAYS="${BASELINE_DAYS:-7}"
THRESHOLD_FACTOR="${THRESHOLD_FACTOR:-2.0}"
ts=$(date -u +%Y-%m-%dT%H:%M:%SZ)

# If no journal exists, report unknown baselines.
if [ ! -f "$JOURNAL" ]; then
    echo "{\"metric\":\"evaluation_status\",\"value\":\"no_journal\",\"timestamp\":\"$ts\"}"
    exit 0
fi

# Query baseline P95 latency from help_sessions (last BASELINE_DAYS).
baseline_latency_p95=0
baseline_latency_p95=$(sqlite3 "$JOURNAL" \
    "SELECT COALESCE(
        (SELECT latency_ms FROM help_sessions
         WHERE latency_ms IS NOT NULL
         AND ts > datetime('now', '-${BASELINE_DAYS} days')
         ORDER BY latency_ms
         LIMIT 1 OFFSET (SELECT CAST(COUNT(*) * 0.95 AS INTEGER) FROM help_sessions WHERE latency_ms IS NOT NULL AND ts > datetime('now', '-${BASELINE_DAYS} days')) - 1
        ), 0);" 2>/dev/null || echo 0)

# Query baseline error rate.
baseline_error_sessions=0
total_help_sessions=0
baseline_error_sessions=$(sqlite3 "$JOURNAL" \
    "SELECT COUNT(*) FROM help_sessions
     WHERE status IN ('error', 'fallback')
     AND ts > datetime('now', '-${BASELINE_DAYS} days');" 2>/dev/null || echo 0)
total_help_sessions=$(sqlite3 "$JOURNAL" \
    "SELECT COUNT(*) FROM help_sessions
     WHERE ts > datetime('now', '-${BASELINE_DAYS} days');" 2>/dev/null || echo 1)

baseline_error_rate=0
if [ "$total_help_sessions" -gt 0 ]; then
    baseline_error_rate=$(python3 -c "print(round($baseline_error_sessions / $total_help_sessions * 100, 1))" 2>/dev/null || echo 0)
fi

# Query baseline sentinel cadence (avg samples per hour).
baseline_samples=0
baseline_samples=$(sqlite3 "$JOURNAL" \
    "SELECT COALESCE(CAST(COUNT(*) / MAX(($BASELINE_DAYS * 24.0), 1) AS INTEGER), 0)
     FROM samples WHERE scope='host'
     AND ts > CAST(strftime('%s', datetime('now', '-${BASELINE_DAYS} days')) AS INTEGER);" 2>/dev/null || echo 0)

# Read current values from stdin if piped (JSON lines from scenario-run-* probes).
current_latency_p95=0
current_error_rate=0
current_samples=0

while IFS= read -r line 2>/dev/null; do
    metric=$(echo "$line" | python3 -c "import sys,json; print(json.load(sys.stdin).get('metric',''))" 2>/dev/null || echo "")
    value=$(echo "$line" | python3 -c "import sys,json; print(json.load(sys.stdin).get('value',0))" 2>/dev/null || echo 0)

    case "$metric" in
        okapi_latency_p95_ms) current_latency_p95=$value ;;
        okapi_error_rate_pct) current_error_rate=$value ;;
        sentinel_samples_collected) current_samples=$value ;;
    esac
done < /dev/stdin 2>/dev/null || true

# Evaluate regressions.
latency_regression=0
latency_status="ok"
if [ "$baseline_latency_p95" -gt 0 ] && [ "$current_latency_p95" -gt 0 ]; then
    latency_regression=$(python3 -c "print(round(($current_latency_p95 - $baseline_latency_p95) / $baseline_latency_p95 * 100, 1))" 2>/dev/null || echo 0)
    if python3 -c "exit(0 if $current_latency_p95 > $baseline_latency_p95 * $THRESHOLD_FACTOR else 1)" 2>/dev/null; then
        latency_status="warn"
    fi
fi

error_regression=0
error_status="ok"
if python3 -c "exit(0 if $current_error_rate > $baseline_error_rate + 5 else 1)" 2>/dev/null; then
    error_status="warn"
    error_regression=$(python3 -c "print(round($current_error_rate - $baseline_error_rate, 1))" 2>/dev/null || echo 0)
fi

cat <<EOF
{"metric":"baseline_latency_p95_ms","value":$baseline_latency_p95,"unit":"ms","timestamp":"$ts","window_days":$BASELINE_DAYS}
{"metric":"baseline_error_rate_pct","value":$baseline_error_rate,"unit":"pct","timestamp":"$ts","window_days":$BASELINE_DAYS}
{"metric":"baseline_samples_per_day","value":$baseline_samples,"unit":"count","timestamp":"$ts","window_days":$BASELINE_DAYS}
{"metric":"current_latency_p95_ms","value":$current_latency_p95,"unit":"ms","timestamp":"$ts"}
{"metric":"current_error_rate_pct","value":$current_error_rate,"unit":"pct","timestamp":"$ts"}
{"metric":"latency_regression_pct","value":$latency_regression,"unit":"pct","timestamp":"$ts","status":"$latency_status"}
{"metric":"error_rate_regression_pp","value":$error_regression,"unit":"pp","timestamp":"$ts","status":"$error_status"}
EOF

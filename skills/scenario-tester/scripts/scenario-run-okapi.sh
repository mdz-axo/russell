#!/usr/bin/env bash
# scenario-run-okapi.sh — probe Okapi inference latency and throughput.
#
# Configuration via env vars:
#   SCENARIO_MODEL      — model to test (default: first loaded model)
#   SCENARIO_ITERATIONS — number of requests (default: 5)
#   SCENARIO_PROMPT     — prompt to send (default: short test prompt)
#   OKAPI_BASE_URL      — Okapi API base (default: http://localhost:11435/v1)
set -euo pipefail

MODEL="${SCENARIO_MODEL:-}"
ITERATIONS="${SCENARIO_ITERATIONS:-5}"
PROMPT="${SCENARIO_PROMPT:-Explain the purpose of a Linux OOM killer in one sentence.}"
BASE_URL="${OKAPI_BASE_URL:-http://localhost:11435/v1}"
TIMEOUT_S="${SCENARIO_TIMEOUT_S:-120}"

# If no model specified, discover the first loaded model from Okapi.
if [ -z "$MODEL" ]; then
    MODEL=$(curl -sS --connect-timeout 5 "${BASE_URL%/*}/api/tags" 2>/dev/null \
        | python3 -c "
import sys, json
data = json.load(sys.stdin)
models = data.get('models', [])
print(models[0]['name'] if models else '')
" 2>/dev/null || echo "")
    if [ -z "$MODEL" ]; then
        echo '{"error":"no model loaded in Okapi"}'
        exit 0
    fi
fi

# Pre-build the JSON body template.
BODY_TEMPLATE=$(python3 -c "
import json, sys
print(json.dumps({
    'model': sys.argv[1],
    'messages': [{'role': 'user', 'content': sys.argv[2]}],
    'stream': False
}))
" "$MODEL" "$PROMPT")

# Collect per-request metrics.
declare -a latencies=()
declare -a tokens_per_sec=()
errors=0
total_tokens=0
start_time=$(date +%s)

for i in $(seq 1 "$ITERATIONS"); do
    req_start=$(date +%s%3N)
    resp=$(curl -sS --connect-timeout 5 --max-time "$TIMEOUT_S" \
        "$BASE_URL/chat/completions" \
        -H "Content-Type: application/json" \
        -d "$BODY_TEMPLATE" 2>/dev/null || echo '{"error":"curl_failed"}')
    req_end=$(date +%s%3N)
    latency_ms=$((req_end - req_start))

    # Parse the response with Python.
    parsed=$(python3 -c "
import sys, json
try:
    d = json.loads(sys.argv[1])
    if 'error' in d:
        print('ERROR:' + str(d.get('error', 'unknown')))
    else:
        content = d['choices'][0]['message'].get('content', '')
        pt = d.get('usage', {}).get('prompt_tokens', 0)
        ct = d.get('usage', {}).get('completion_tokens', 0)
        print(f'{pt}:{ct}:OK')
except Exception as e:
    print(f'ERROR:parse:{e}')
" "$resp" 2>/dev/null)

    if [[ "$parsed" == ERROR:* ]]; then
        errors=$((errors + 1))
        continue
    fi

    # Extract prompt_tokens:completion_tokens from parsed output.
    pt=$(echo "$parsed" | cut -d: -f1)
    ct=$(echo "$parsed" | cut -d: -f2)
    total_tokens=$((total_tokens + ct))

    tps=0
    if [ "$ct" -gt 0 ] && [ "$latency_ms" -gt 0 ]; then
        tps=$(python3 -c "print(round($ct / ($latency_ms / 1000.0), 1))" 2>/dev/null || echo 0)
    fi

    latencies+=("$latency_ms")
    tokens_per_sec+=("$tps")
done

end_time=$(date +%s)
total_duration=$((end_time - start_time))

# Compute percentiles using Python.
if [ ${#latencies[@]} -eq 0 ]; then
    echo '{"error":"all requests failed"}'
    exit 0
fi

stats=$(python3 -c "
import sys, json

lats = [$(IFS=,; echo "${latencies[*]}")]
tps_vals = [$(IFS=,; echo "${tokens_per_sec[*]}")]

lats_sorted = sorted(lats)
tps_sorted = sorted(tps_vals)
n = len(lats_sorted)

def pct(arr, p):
    idx = max(0, min(n - 1, int(round(n * p / 100.0)) - 1))
    return arr[idx] if arr else 0

print(json.dumps({
    'latency_p50': pct(lats_sorted, 50),
    'latency_p95': pct(lats_sorted, 95),
    'latency_p99': pct(lats_sorted, 99),
    'latency_min': lats_sorted[0],
    'latency_max': lats_sorted[-1],
    'tps_p50': pct(tps_sorted, 50),
    'tps_p95': pct(tps_sorted, 95),
}))
" 2>/dev/null)

# Parse stats JSON.
latency_p50=$(echo "$stats" | python3 -c "import sys,json; print(json.load(sys.stdin)['latency_p50'])" 2>/dev/null || echo 0)
latency_p95=$(echo "$stats" | python3 -c "import sys,json; print(json.load(sys.stdin)['latency_p95'])" 2>/dev/null || echo 0)
latency_p99=$(echo "$stats" | python3 -c "import sys,json; print(json.load(sys.stdin)['latency_p99'])" 2>/dev/null || echo 0)
latency_min=$(echo "$stats" | python3 -c "import sys,json; print(json.load(sys.stdin)['latency_min'])" 2>/dev/null || echo 0)
latency_max=$(echo "$stats" | python3 -c "import sys,json; print(json.load(sys.stdin)['latency_max'])" 2>/dev/null || echo 0)
tps_p50=$(echo "$stats" | python3 -c "import sys,json; print(json.load(sys.stdin)['tps_p50'])" 2>/dev/null || echo 0)
tps_p95=$(echo "$stats" | python3 -c "import sys,json; print(json.load(sys.stdin)['tps_p95'])" 2>/dev/null || echo 0)

throughput=0
if [ "$total_duration" -gt 0 ]; then
    throughput=$(python3 -c "print(round($ITERATIONS / $total_duration, 2))" 2>/dev/null || echo 0)
fi

error_rate=0
if [ "$ITERATIONS" -gt 0 ]; then
    error_rate=$(python3 -c "print(round($errors / $ITERATIONS * 100, 1))" 2>/dev/null || echo 0)
fi

ts=$(date -u +%Y-%m-%dT%H:%M:%SZ)

cat <<EOF
{"metric":"okapi_latency_p50_ms","value":$latency_p50,"unit":"ms","timestamp":"$ts","model":"$MODEL"}
{"metric":"okapi_latency_p95_ms","value":$latency_p95,"unit":"ms","timestamp":"$ts","model":"$MODEL"}
{"metric":"okapi_latency_p99_ms","value":$latency_p99,"unit":"ms","timestamp":"$ts","model":"$MODEL"}
{"metric":"okapi_latency_min_ms","value":$latency_min,"unit":"ms","timestamp":"$ts","model":"$MODEL"}
{"metric":"okapi_latency_max_ms","value":$latency_max,"unit":"ms","timestamp":"$ts","model":"$MODEL"}
{"metric":"okapi_tokens_per_sec_p50","value":$tps_p50,"unit":"tps","timestamp":"$ts","model":"$MODEL"}
{"metric":"okapi_tokens_per_sec_p95","value":$tps_p95,"unit":"tps","timestamp":"$ts","model":"$MODEL"}
{"metric":"okapi_throughput_rps","value":$throughput,"unit":"rps","timestamp":"$ts","model":"$MODEL"}
{"metric":"okapi_error_rate_pct","value":$error_rate,"unit":"pct","timestamp":"$ts","model":"$MODEL"}
{"metric":"okapi_total_tokens","value":$total_tokens,"unit":"tokens","timestamp":"$ts","model":"$MODEL"}
{"metric":"okapi_iterations","value":$ITERATIONS,"unit":"count","timestamp":"$ts","model":"$MODEL"}
EOF

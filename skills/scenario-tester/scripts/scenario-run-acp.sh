#!/usr/bin/env bash
# scenario-run-acp.sh — test ACP server responsiveness.
#
# Sends JSON-RPC requests to russell-acp-server and measures response characteristics.
set -euo pipefail

ACP_SERVER="${ACP_SERVER_BIN:-}"
if [ -z "$ACP_SERVER" ]; then
    if [ -x "$HOME/.local/bin/russell-acp-server" ]; then
        ACP_SERVER="$HOME/.local/bin/russell-acp-server"
    elif [ -x "./target/release/russell-acp-server" ]; then
        ACP_SERVER="./target/release/russell-acp-server"
    else
        ACP_SERVER="russell-acp-server"
    fi
fi
TURNS="${SCENARIO_TURNS:-3}"

ts=$(date -u +%Y-%m-%dT%H:%M:%SZ)

start_time=$(date +%s)
errors=0
successes=0

for i in $(seq 1 "$TURNS"); do
    turn_start=$(date +%s%3N)
    response=$(echo '{"jsonrpc":"2.0","id":'$i',"method":"acp/capabilities","params":{}}' | timeout 10 "$ACP_SERVER" 2>&1) || true
    turn_end=$(date +%s%3N)

    if echo "$response" | grep -qi '"result"'; then
        successes=$((successes + 1))
    else
        errors=$((errors + 1))
    fi
done

end_time=$(date +%s)
total_duration=$((end_time - start_time))

throughput=0
if [ "$total_duration" -gt 0 ]; then
    throughput=$(python3 -c "print(round($TURNS / $total_duration, 2))" 2>/dev/null || echo 0)
fi

error_rate=0
if [ "$TURNS" -gt 0 ]; then
    error_rate=$(python3 -c "print(round($errors / $TURNS * 100, 1))" 2>/dev/null || echo 0)
fi

cat <<EOF
{"metric":"russell_acp_turns","value":$TURNS,"unit":"count","timestamp":"$ts"}
{"metric":"russell_acp_successes","value":$successes,"unit":"count","timestamp":"$ts"}
{"metric":"russell_acp_errors","value":$errors,"unit":"count","timestamp":"$ts"}
{"metric":"russell_acp_error_rate_pct","value":$error_rate,"unit":"pct","timestamp":"$ts"}
{"metric":"russell_acp_total_duration_s","value":$total_duration,"unit":"s","timestamp":"$ts"}
{"metric":"russell_acp_throughput_tps","value":$throughput,"unit":"tps","timestamp":"$ts"}
EOF

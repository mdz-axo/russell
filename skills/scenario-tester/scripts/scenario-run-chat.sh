#!/usr/bin/env bash
# scenario-run-chat.sh — test russell chat REPL responsiveness.
#
# Runs several quick russell chat turns and measures response characteristics.
# Uses RUSSELL_DOCTOR_BACKEND=offline for deterministic testing or okapi for live.
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
TURNS="${SCENARIO_TURNS:-3}"
TEST_PROMPT="${SCENARIO_PROMPT:-what is the current load average}"
BACKEND="${RUSSELL_DOCTOR_BACKEND:-offline}"

ts=$(date -u +%Y-%m-%dT%H:%M:%SZ)

if [ "$BACKEND" = "offline" ]; then
    # Offline mode: verify chat starts and responds deterministically.
    output=$(echo "/quit" | timeout 10 "$RUSSELL" chat 2>&1) || true
    if echo "$output" | grep -qi "jack\|chat\|goodbye\|session"; then
        echo "{\"metric\":\"russell_chat_startup\",\"value\":1,\"unit\":\"boolean\",\"timestamp\":\"$ts\"}"
    else
        echo "{\"metric\":\"russell_chat_startup\",\"value\":0,\"unit\":\"boolean\",\"timestamp\":\"$ts\"}"
        echo "{\"metric\":\"russell_chat_error\",\"value\":1,\"unit\":\"count\",\"timestamp\":\"$ts\"}"
    fi
    exit 0
fi

# Live mode: measure chat response time.
# We drive russell chat via stdin, capturing timing and response.
start_time=$(date +%s)
total_chars=0
errors=0
successes=0

for i in $(seq 1 "$TURNS"); do
    turn_start=$(date +%s%3N)
    response=$(printf '%s\n/quit\n' "$TEST_PROMPT $i" | timeout 60 "$RUSSELL" chat 2>&1) || true
    turn_end=$(date +%s%3N)
    turn_ms=$((turn_end - turn_start))

    if echo "$response" | grep -qi "error\|failed\|unreachable"; then
        errors=$((errors + 1))
    else
        successes=$((successes + 1))
        chars=$(echo "$response" | wc -c)
        total_chars=$((total_chars + chars))
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

avg_chars=0
if [ "$successes" -gt 0 ]; then
    avg_chars=$((total_chars / successes))
fi

cat <<EOF
{"metric":"russell_chat_turns","value":$TURNS,"unit":"count","timestamp":"$ts"}
{"metric":"russell_chat_successes","value":$successes,"unit":"count","timestamp":"$ts"}
{"metric":"russell_chat_errors","value":$errors,"unit":"count","timestamp":"$ts"}
{"metric":"russell_chat_error_rate_pct","value":$error_rate,"unit":"pct","timestamp":"$ts"}
{"metric":"russell_chat_total_duration_s","value":$total_duration,"unit":"s","timestamp":"$ts"}
{"metric":"russell_chat_throughput_tps","value":$throughput,"unit":"tps","timestamp":"$ts"}
{"metric":"russell_chat_avg_response_chars","value":$avg_chars,"unit":"chars","timestamp":"$ts"}
EOF

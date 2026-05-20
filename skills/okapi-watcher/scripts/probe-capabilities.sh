#!/bin/bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# probe-capabilities.sh — probe Okapi runner capabilities
#
# Queries Okapi's /api/engine/status endpoint to determine:
# - Runner type (ollamarunner vs llamarunner)
# - Feature availability (LoRA hot-swap, token probs, etc.)
# - Degraded mode detection
#
# Output: JSON with capability flags for journal recording

set -euo pipefail

OKAPI_HOST="${OLLAMA_HOST:-127.0.0.1:11435}"
OKAPI_URL="http://${OKAPI_HOST}"

# Query engine status
STATUS_RESPONSE=$(curl -s --max-time 10 "${OKAPI_URL}/api/engine/status" 2>/dev/null) || {
    echo '{"error": "failed to connect to Okapi", "okapi_host": "'${OKAPI_HOST}'"}'
    exit 0
}

# Check if response is valid JSON
if ! echo "$STATUS_RESPONSE" | jq -e '.' >/dev/null 2>&1; then
    echo '{"error": "invalid JSON response", "raw_response": "'$(echo "$STATUS_RESPONSE" | head -c 200 | tr -d '\n')'"}'
    exit 0
fi

# Extract capability information
RUNNER_TYPE=$(echo "$STATUS_RESPONSE" | jq -r '.runner_type // "unknown"')
MODEL_LOADED=$(echo "$STATUS_RESPONSE" | jq -r '.model_loaded // false')
MODEL_NAME=$(echo "$STATUS_RESPONSE" | jq -r '.model_name // ""')

# Extract capability flags
LORA_HOT_SWAP=$(echo "$STATUS_RESPONSE" | jq -r '.capabilities.lora_hot_swap // false')
TOKEN_PROBS=$(echo "$STATUS_RESPONSE" | jq -r '.capabilities.token_probs // false')
FULL_METRICS=$(echo "$STATUS_RESPONSE" | jq -r '.capabilities.full_metrics // false')
ADVANCED_SAMPLING=$(echo "$STATUS_RESPONSE" | jq -r '.capabilities.advanced_sampling // false')
GRAMMAR_NATIVE=$(echo "$STATUS_RESPONSE" | jq -r '.capabilities.grammar_native // false')
SPECULATIVE_DECODING=$(echo "$STATUS_RESPONSE" | jq -r '.capabilities.speculative_decoding // false')
DRY_SAMPLER=$(echo "$STATUS_RESPONSE" | jq -r '.capabilities.dry_sampler // false')
XTC_SAMPLER=$(echo "$STATUS_RESPONSE" | jq -r '.capabilities.xtc_sampler // false')
MIN_KEEP=$(echo "$STATUS_RESPONSE" | jq -r '.capabilities.min_keep // false')
CHUNKED_PREFILL=$(echo "$STATUS_RESPONSE" | jq -r '.capabilities.chunked_prefill // false')
MOE_OBSERVABILITY=$(echo "$STATUS_RESPONSE" | jq -r '.capabilities.moe_observability // false')

# Determine degraded mode (llamarunner fallback)
DEGRADED_MODE="false"
if [ "$RUNNER_TYPE" = "llamarunner" ]; then
    DEGRADED_MODE="true"
fi

# Count available features
FEATURE_COUNT=0
for feat in "$LORA_HOT_SWAP" "$TOKEN_PROBS" "$FULL_METRICS" "$ADVANCED_SAMPLING" \
            "$GRAMMAR_NATIVE" "$SPECULATIVE_DECODING" "$DRY_SAMPLER" "$XTC_SAMPLER" \
            "$MIN_KEEP" "$CHUNKED_PREFILL" "$MOE_OBSERVABILITY"; do
    if [ "$feat" = "true" ]; then
        FEATURE_COUNT=$((FEATURE_COUNT + 1))
    fi
done

# Output structured JSON
jq -n \
    --arg runner_type "$RUNNER_TYPE" \
    --argjson model_loaded "$MODEL_LOADED" \
    --arg model_name "$MODEL_NAME" \
    --argjson lora_hot_swap "$LORA_HOT_SWAP" \
    --argjson token_probs "$TOKEN_PROBS" \
    --argjson full_metrics "$FULL_METRICS" \
    --argjson advanced_sampling "$ADVANCED_SAMPLING" \
    --argjson grammar_native "$GRAMMAR_NATIVE" \
    --argjson speculative_decoding "$SPECULATIVE_DECODING" \
    --argjson dry_sampler "$DRY_SAMPLER" \
    --argjson xtc_sampler "$XTC_SAMPLER" \
    --argjson min_keep "$MIN_KEEP" \
    --argjson chunked_prefill "$CHUNKED_PREFILL" \
    --argjson moe_observability "$MOE_OBSERVABILITY" \
    --argjson degraded_mode "$DEGRADED_MODE" \
    --argjson feature_count "$FEATURE_COUNT" \
    '{
        runner_type: $runner_type,
        model_loaded: $model_loaded,
        model_name: $model_name,
        capabilities: {
            lora_hot_swap: $lora_hot_swap,
            token_probs: $token_probs,
            full_metrics: $full_metrics,
            advanced_sampling: $advanced_sampling,
            grammar_native: $grammar_native,
            speculative_decoding: $speculative_decoding,
            dry_sampler: $dry_sampler,
            xtc_sampler: $xtc_sampler,
            min_keep: $min_keep,
            chunked_prefill: $chunked_prefill,
            moe_observability: $moe_observability
        },
        degraded_mode: $degraded_mode,
        feature_count: $feature_count,
        timestamp: (now | todate)
    }'

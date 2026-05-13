#!/bin/bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# okapi-watcher: probe-gpu-libs
#
# Checks that Okapi's GPU libraries (libggml-hip.so, libggml-base.so, etc.)
# are ABI-compatible with the running Okapi binary by comparing modification
# timestamps. When the binary is newer than the GPU libraries, GPU discovery
# will fail (30s timeout → CPU-only fallback).
#
# Also verifies:
#   - libggml-hip.so exists and its dynamic dependencies resolve
#   - The okapi binary and libs are from the same build generation
#   - GPU discovery is actually working (not timed out to CPU)
#
# Output format (one line):
#   "ok gpu=<device_count>"                — all good
#   "stale_libs binary=<ts> libs=<ts>"     — binary newer than GPU libs (rebuild needed)
#   "missing_hip"                          — libggml-hip.so not found
#   "unresolved_deps <lib1> <lib2> ..."    — missing shared library dependencies
#   "gpu_discovery_failed"                 — okapi running CPU-only despite libs present
#
# Exit codes:
#   0 — healthy (GPU libs match binary)
#   1 — mismatch or problem detected

set -euo pipefail

# --- Configuration -----------------------------------------------------------
OKAPI_BIN="${OKAPI_BIN:-$HOME/.local/bin/okapi}"
OKAPI_LIB_DIR="${OKAPI_LIB_DIR:-$HOME/.local/lib/ollama}"
OKAPI_HOST="${OKAPI_HOST:-http://127.0.0.1:11435}"

# --- Existence checks --------------------------------------------------------
if [ ! -x "$OKAPI_BIN" ]; then
    echo "missing_binary"
    exit 1
fi

HIP_LIB="$OKAPI_LIB_DIR/rocm/libggml-hip.so"
BASE_LIB="$OKAPI_LIB_DIR/libggml-base.so.0"

if [ ! -f "$HIP_LIB" ]; then
    echo "missing_hip"
    exit 1
fi

if [ ! -f "$BASE_LIB" ]; then
    echo "missing_base"
    exit 1
fi

# --- Timestamp comparison ----------------------------------------------------
# Get modification times as epoch seconds.
bin_mtime=$(stat -c %Y "$OKAPI_BIN" 2>/dev/null || echo 0)
hip_mtime=$(stat -c %Y "$HIP_LIB" 2>/dev/null || echo 0)
base_mtime=$(stat -c %Y "$BASE_LIB" 2>/dev/null || echo 0)

# The GPU library must be from the same build or newer than the binary.
# A tolerance of 300s (5 min) accounts for build pipeline ordering.
TOLERANCE=300

if [ "$bin_mtime" -gt $((hip_mtime + TOLERANCE)) ]; then
    bin_date=$(date -d "@$bin_mtime" '+%Y-%m-%d_%H:%M' 2>/dev/null || echo "$bin_mtime")
    hip_date=$(date -d "@$hip_mtime" '+%Y-%m-%d_%H:%M' 2>/dev/null || echo "$hip_mtime")
    echo "stale_libs binary=$bin_date libs=$hip_date"
    exit 1
fi

if [ "$bin_mtime" -gt $((base_mtime + TOLERANCE)) ]; then
    bin_date=$(date -d "@$bin_mtime" '+%Y-%m-%d_%H:%M' 2>/dev/null || echo "$bin_mtime")
    base_date=$(date -d "@$base_mtime" '+%Y-%m-%d_%H:%M' 2>/dev/null || echo "$base_mtime")
    echo "stale_libs binary=$bin_date libs=$base_date"
    exit 1
fi

# --- Dependency resolution check ---------------------------------------------
# Verify that libggml-hip.so's dynamic dependencies all resolve.
unresolved=$(LD_LIBRARY_PATH="$OKAPI_LIB_DIR:${LD_LIBRARY_PATH:-}" \
    ldd "$HIP_LIB" 2>/dev/null | grep "not found" | awk '{print $1}' || true)

if [ -n "$unresolved" ]; then
    echo "unresolved_deps $unresolved"
    exit 1
fi

# --- GPU discovery health check ----------------------------------------------
# If Okapi is running, verify it actually found GPUs (not CPU-only fallback).
if curl -s --max-time 3 "${OKAPI_HOST}/api/tags" >/dev/null 2>&1; then
    # Check recent journal logs for GPU discovery status.
    # Look for "inference compute" lines — if only "cpu" exists, GPU failed.
    gpu_line=$(journalctl --user -u okapi --since "10 minutes ago" --no-pager 2>/dev/null \
        | grep "inference compute" | grep -v "id=cpu" | tail -1 || true)

    cpu_only=$(journalctl --user -u okapi --since "10 minutes ago" --no-pager 2>/dev/null \
        | grep "failure during GPU discovery" | tail -1 || true)

    if [ -n "$cpu_only" ] && [ -z "$gpu_line" ]; then
        echo "gpu_discovery_failed"
        exit 1
    fi

    # Count GPU devices from the inference compute lines
    gpu_count=$(journalctl --user -u okapi --since "10 minutes ago" --no-pager 2>/dev/null \
        | grep "inference compute" | grep -v "id=cpu" | wc -l || echo 0)
    echo "ok gpu=$gpu_count"
    exit 0
fi

# Okapi not running — just report lib status is fine
echo "ok gpu=unknown(okapi_not_running)"
exit 0

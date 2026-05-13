#!/bin/bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# sysadmin: probe-systemd-degraded
# Checks whether the system is in a degraded state.
# Output: "degraded" or "running" or "unknown:<state>".
set -euo pipefail

STATE=$(systemctl is-system-running 2>/dev/null || echo "unknown")
echo "state=$STATE"

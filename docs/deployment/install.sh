#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
#
# This script is superseded by the root-level install.sh.
# It is kept only for backward compatibility — it delegates entirely.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CANONICAL="${SCRIPT_DIR}/../../install.sh"

if [ ! -f "$CANONICAL" ]; then
    echo "ERROR: canonical install.sh not found at ${CANONICAL}"
    exit 1
fi

echo "NOTE: delegating to canonical install.sh at repository root" >&2
exec "$CANONICAL" "$@"

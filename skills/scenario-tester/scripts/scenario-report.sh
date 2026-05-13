#!/usr/bin/env bash
# scenario-report.sh — generate a test summary report from the journal.
#
# Produces a compact Markdown summary suitable for memory/test-reports/.
set -euo pipefail

JOURNAL="$HOME/.local/state/harness/journal.db"
if [ ! -f "$JOURNAL" ] && [ -f "$HOME/.local/share/harness/journal/russell.db" ]; then
    JOURNAL="$HOME/.local/share/harness/journal/russell.db"
fi
ts=$(date -u +%Y-%m-%dT%H:%M:%SZ)
today=$(date +%Y-%m-%d)

echo "# Agentic AI Test Report — $today"
echo

if [ ! -f "$JOURNAL" ]; then
    echo "**No journal found.** Run scenarios first to populate test data."
    exit 0
fi

# -- Okapi metrics (from help_sessions) --
echo "## Okapi (LLM Inference)"
echo
total_sessions=$(sqlite3 "$JOURNAL" "SELECT COUNT(*) FROM help_sessions WHERE ts > datetime('now', '-1 days');" 2>/dev/null || echo 0)
ok_sessions=$(sqlite3 "$JOURNAL" "SELECT COUNT(*) FROM help_sessions WHERE status='ok' AND ts > datetime('now', '-1 days');" 2>/dev/null || echo 0)
err_sessions=$(sqlite3 "$JOURNAL" "SELECT COUNT(*) FROM help_sessions WHERE status IN ('error','fallback') AND ts > datetime('now', '-1 days');" 2>/dev/null || echo 0)
p50_latency=$(sqlite3 "$JOURNAL" "SELECT COALESCE(CAST(AVG(latency_ms) AS INTEGER), 0) FROM help_sessions WHERE latency_ms IS NOT NULL AND ts > datetime('now', '-1 days');" 2>/dev/null || echo 0)
max_latency=$(sqlite3 "$JOURNAL" "SELECT COALESCE(MAX(latency_ms), 0) FROM help_sessions WHERE ts > datetime('now', '-1 days');" 2>/dev/null || echo 0)

echo "- Sessions (24h): $total_sessions ($ok_sessions ok, $err_sessions error)"
echo "- Avg latency: ${p50_latency}ms"
echo "- Max latency: ${max_latency}ms"

# -- Russell host health (from samples) --
echo
echo "## Russell (Host Health)"
echo

sample_count=$(sqlite3 "$JOURNAL" "SELECT COUNT(*) FROM samples WHERE scope='host' AND ts > CAST(strftime('%s', datetime('now', '-1 days')) AS INTEGER);" 2>/dev/null || echo 0)
events_24h=$(sqlite3 "$JOURNAL" "SELECT COUNT(*) FROM events WHERE ts > CAST(strftime('%s', datetime('now', '-1 days')) AS INTEGER);" 2>/dev/null || echo 0)
alerts_24h=$(sqlite3 "$JOURNAL" "SELECT COUNT(*) FROM events WHERE severity IN ('alert','crit') AND ts > CAST(strftime('%s', datetime('now', '-1 days')) AS INTEGER);" 2>/dev/null || echo 0)

echo "- Samples collected (24h): $sample_count"
echo "- Events (24h): $events_24h"
echo "- Alerts (24h): $alerts_24h"

# -- Recent events --
echo
echo "## Recent Events"
echo

sqlite3 "$JOURNAL" \
    "SELECT severity, summary FROM events
     WHERE ts > CAST(strftime('%s', datetime('now', '-1 hours')) AS INTEGER)
     ORDER BY ts DESC LIMIT 5;" 2>/dev/null | while IFS='|' read -r sev summary; do
    echo "- **[$sev]** $summary"
done

echo
echo "---"
echo "Generated: $ts"

// SPDX-License-Identifier: MIT OR Apache-2.0
//! MCP tool implementations for Russell.
//!
//! All tools are read-only (risk: none) per JR-2 and ADR-0003.

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use russell_core::paths::Paths;
use serde::Deserialize;

use crate::RussellServer;

// ─── Constructor (must be here to access generated tool_router()) ─

impl RussellServer {
    /// Create a new server instance anchored at the given paths.
    pub fn new(paths: Paths) -> Self {
        Self {
            paths,
            tool_router: Self::tool_router(),
        }
    }
}

// ─── Request schemas ─────────────────────────────────────────────

/// Parameters for `russell_host_snapshot`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct HostSnapshotParams {
    /// Hours of history to include (default: 24).
    pub hours: Option<u32>,
}

/// Parameters for `russell_recent_events`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct RecentEventsParams {
    /// Maximum number of events to return (default: 20, max: 100).
    pub limit: Option<usize>,
}

/// Parameters for `russell_journal_query`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct JournalQueryParams {
    /// Hours back from now to query (default: 24).
    pub hours: Option<u32>,
    /// Filter by minimum severity: "info", "warn", "alert", "crit".
    pub min_severity: Option<String>,
    /// Filter by scope: "host" or "self".
    pub scope: Option<String>,
}

/// Parameters for `russell_probe_history`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct ProbeHistoryParams {
    /// Hours of history to summarize (default: 24).
    pub hours: Option<u32>,
    /// Filter to a specific probe name (optional).
    pub probe: Option<String>,
}

/// Parameters for `russell_health_summary`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct HealthSummaryParams {
    /// Hours window for severity counts (default: 24).
    pub hours: Option<u32>,
}

/// Parameters for `russell_run_sentinel`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct RunSentinelParams {
    /// If true, only report what would be collected (no journal write).
    pub dry_run: Option<bool>,
}

// ─── Tool implementations ────────────────────────────────────────

#[tool_router]
impl RussellServer {
    /// Current system probe values with min/avg/max over the requested
    /// time window and 30-day EWMA baselines for comparison.
    #[tool(description = "Get a snapshot of current host telemetry: per-probe last value, \
        min/avg/max over the time window, and 30-day p95 baselines for anomaly detection.")]
    fn russell_host_snapshot(
        &self,
        Parameters(params): Parameters<HostSnapshotParams>,
    ) -> String {
        let hours = params.hours.unwrap_or(24);
        let now = russell_core::time::now_unix();
        let since = now - (hours as i64 * 3600);

        let journal_path = self.paths.journal();
        if !journal_path.exists() {
            return serde_json::json!({
                "error": "journal not found — run `russell sentinel-once` first"
            })
            .to_string();
        }

        let reader = russell_core::journal::JournalReader::new(&journal_path);

        let samples = reader.host_samples_summary(since, now).unwrap_or_default();
        let baselines = reader.read_baselines().unwrap_or_default();

        let mut probes: Vec<serde_json::Value> = samples
            .iter()
            .map(|s| {
                let baseline = baselines.iter().find(|b| b.probe == s.probe);
                serde_json::json!({
                    "probe": s.probe,
                    "unit": s.unit,
                    "last": s.last,
                    "last_ts": s.last_ts,
                    "min": s.min,
                    "avg": s.avg,
                    "max": s.max,
                    "count": s.count,
                    "p95_30d": baseline.and_then(|b| b.p95),
                })
            })
            .collect();

        probes.sort_by(|a, b| {
            a["probe"]
                .as_str()
                .unwrap_or("")
                .cmp(b["probe"].as_str().unwrap_or(""))
        });

        serde_json::json!({
            "window_hours": hours,
            "since_unix": since,
            "now_unix": now,
            "probe_count": probes.len(),
            "probes": probes,
        })
        .to_string()
    }

    /// Recent journal events (newest first).
    #[tool(description = "List the most recent journal events. Returns timestamp, severity, \
        scope, module, action, and summary for each event.")]
    fn russell_recent_events(
        &self,
        Parameters(params): Parameters<RecentEventsParams>,
    ) -> String {
        let limit = params.limit.unwrap_or(20).min(100);

        let journal_path = self.paths.journal();
        if !journal_path.exists() {
            return serde_json::json!({
                "error": "journal not found — run `russell sentinel-once` first"
            })
            .to_string();
        }

        let reader = russell_core::journal::JournalReader::new(&journal_path);
        let rows = reader.recent(limit).unwrap_or_default();

        let events: Vec<serde_json::Value> = rows
            .iter()
            .map(|r| {
                serde_json::json!({
                    "id": r.id,
                    "ts": r.ts,
                    "severity": r.severity.as_str(),
                    "scope": r.scope.as_str(),
                    "module": r.module,
                    "action": r.action,
                    "summary": r.summary,
                })
            })
            .collect();

        serde_json::json!({
            "count": events.len(),
            "events": events,
        })
        .to_string()
    }

    /// Query journal events filtered by time, severity, and scope.
    #[tool(description = "Query journal events within a time window, optionally filtered by \
        minimum severity (info/warn/alert/crit) and scope (host/self).")]
    fn russell_journal_query(
        &self,
        Parameters(params): Parameters<JournalQueryParams>,
    ) -> String {
        let hours = params.hours.unwrap_or(24);
        let now = russell_core::time::now_unix();
        let since = now - (hours as i64 * 3600);

        let journal_path = self.paths.journal();
        if !journal_path.exists() {
            return serde_json::json!({
                "error": "journal not found — run `russell sentinel-once` first"
            })
            .to_string();
        }

        let reader = russell_core::journal::JournalReader::new(&journal_path);

        // Get severity counts for the window.
        let counts = reader.severity_counts(since, now).unwrap_or_default();

        // Get recent events (up to 200) and filter by severity/scope.
        let rows = reader.recent(200).unwrap_or_default();

        let min_sev = params
            .min_severity
            .as_deref()
            .and_then(|s| s.parse::<russell_core::event::Severity>().ok())
            .unwrap_or(russell_core::event::Severity::Info);

        let scope_filter = params
            .scope
            .as_deref()
            .and_then(|s| s.parse::<russell_core::event::Scope>().ok());

        let events: Vec<serde_json::Value> = rows
            .iter()
            .filter(|r| r.severity >= min_sev)
            .filter(|r| scope_filter.is_none() || scope_filter == Some(r.scope))
            .take(50)
            .map(|r| {
                serde_json::json!({
                    "id": r.id,
                    "ts": r.ts,
                    "severity": r.severity.as_str(),
                    "scope": r.scope.as_str(),
                    "module": r.module,
                    "action": r.action,
                    "summary": r.summary,
                })
            })
            .collect();

        serde_json::json!({
            "window_hours": hours,
            "severity_counts": {
                "info": counts.info,
                "warn": counts.warn,
                "alert": counts.alert,
                "crit": counts.crit,
            },
            "filtered_events": events.len(),
            "events": events,
        })
        .to_string()
    }

    /// Per-probe sample summaries (min/avg/max/last) over the time window.
    #[tool(description = "Get per-probe sample statistics (min, avg, max, last value, count) \
        over the requested time window. Optionally filter to a single probe name.")]
    fn russell_probe_history(
        &self,
        Parameters(params): Parameters<ProbeHistoryParams>,
    ) -> String {
        let hours = params.hours.unwrap_or(24);
        let now = russell_core::time::now_unix();
        let since = now - (hours as i64 * 3600);

        let journal_path = self.paths.journal();
        if !journal_path.exists() {
            return serde_json::json!({
                "error": "journal not found — run `russell sentinel-once` first"
            })
            .to_string();
        }

        let reader = russell_core::journal::JournalReader::new(&journal_path);
        let samples = reader.host_samples_summary(since, now).unwrap_or_default();

        let filtered: Vec<serde_json::Value> = samples
            .iter()
            .filter(|s| {
                params
                    .probe
                    .as_ref()
                    .is_none_or(|p| s.probe.contains(p.as_str()))
            })
            .map(|s| {
                serde_json::json!({
                    "probe": s.probe,
                    "unit": s.unit,
                    "min": s.min,
                    "avg": s.avg,
                    "max": s.max,
                    "last": s.last,
                    "last_ts": s.last_ts,
                    "count": s.count,
                })
            })
            .collect();

        serde_json::json!({
            "window_hours": hours,
            "probe_count": filtered.len(),
            "probes": filtered,
        })
        .to_string()
    }

    /// Overall health summary: severity breakdown, staleness, baseline deviations.
    #[tool(description = "Get an overall health summary: severity breakdown over the time \
        window, data freshness (seconds since last sample), and probes that exceed \
        their 30-day p95 baseline by more than 1.5x.")]
    fn russell_health_summary(
        &self,
        Parameters(params): Parameters<HealthSummaryParams>,
    ) -> String {
        let hours = params.hours.unwrap_or(24);
        let now = russell_core::time::now_unix();
        let since = now - (hours as i64 * 3600);

        let journal_path = self.paths.journal();
        if !journal_path.exists() {
            return serde_json::json!({
                "error": "journal not found — run `russell sentinel-once` first"
            })
            .to_string();
        }

        let reader = russell_core::journal::JournalReader::new(&journal_path);

        let counts = reader.severity_counts(since, now).unwrap_or_default();
        let last_sample_ts = reader.last_host_sample_ts().ok().flatten();
        let staleness_s = last_sample_ts.map(|ts| now - ts);

        let samples = reader.host_samples_summary(since, now).unwrap_or_default();
        let baselines = reader.read_baselines().unwrap_or_default();

        // Find probes exceeding 1.5x their p95 baseline.
        let deviations: Vec<serde_json::Value> = samples
            .iter()
            .filter_map(|s| {
                let baseline = baselines.iter().find(|b| b.probe == s.probe)?;
                let p95 = baseline.p95?;
                let last = s.last?;
                if p95 > 0.0 && last > p95 * 1.5 {
                    Some(serde_json::json!({
                        "probe": s.probe,
                        "last": last,
                        "p95_30d": p95,
                        "ratio": format!("{:.1}x", last / p95),
                    }))
                } else {
                    None
                }
            })
            .collect();

        let status = if counts.crit > 0 {
            "critical"
        } else if counts.alert > 0 {
            "alert"
        } else if counts.warn > 0 {
            "warn"
        } else {
            "healthy"
        };

        serde_json::json!({
            "status": status,
            "window_hours": hours,
            "severity_counts": {
                "info": counts.info,
                "warn": counts.warn,
                "alert": counts.alert,
                "crit": counts.crit,
            },
            "staleness_seconds": staleness_s,
            "baseline_deviations": deviations,
        })
        .to_string()
    }

    /// Run the Sentinel once and return fresh probe samples.
    #[tool(description = "Run Russell's Sentinel telemetry collector once, writing fresh \
        samples to the journal and returning the collected values. This is a read-only \
        observation — it never mutates host state beyond appending to Russell's own journal.")]
    fn russell_run_sentinel(
        &self,
        Parameters(params): Parameters<RunSentinelParams>,
    ) -> String {
        let dry_run = params.dry_run.unwrap_or(false);

        if dry_run {
            // Collect without writing.
            let samples = russell_sentinel::probes::collect();
            let values: Vec<serde_json::Value> = samples
                .iter()
                .map(|s| {
                    serde_json::json!({
                        "probe": s.name,
                        "value": s.value_num,
                        "text": s.value_text,
                        "unit": s.unit,
                    })
                })
                .collect();
            return serde_json::json!({
                "dry_run": true,
                "sample_count": values.len(),
                "samples": values,
            })
            .to_string();
        }

        // Run sentinel and write to journal.
        let journal_path = self.paths.journal();
        let writer = match russell_core::journal::JournalWriter::open(&journal_path) {
            Ok(w) => w,
            Err(e) => {
                return serde_json::json!({
                    "error": format!("failed to open journal: {e}")
                })
                .to_string();
            }
        };

        match russell_sentinel::run_once(&writer) {
            Ok(count) => {
                // Read back the freshest samples.
                let now = russell_core::time::now_unix();
                let reader = russell_core::journal::JournalReader::new(&journal_path);
                let samples = reader
                    .host_samples_summary(now - 60, now)
                    .unwrap_or_default();

                let values: Vec<serde_json::Value> = samples
                    .iter()
                    .map(|s| {
                        serde_json::json!({
                            "probe": s.probe,
                            "last": s.last,
                            "unit": s.unit,
                        })
                    })
                    .collect();

                serde_json::json!({
                    "dry_run": false,
                    "sample_count": count,
                    "samples": values,
                })
                .to_string()
            }
            Err(e) => serde_json::json!({
                "error": format!("sentinel run failed: {e}")
            })
            .to_string(),
        }
    }
}

// SPDX-License-Identifier: MIT OR Apache-2.0
//! `harness.event.v1` — the canonical structured log record.
//!
//! Every mutating action, every Sentinel cycle, every Doctor
//! run emits one of these. Human-readable output derives from
//! this type, never the reverse. See
//! [`docs/standards/safety.md`](../../../docs/standards/safety.md)
//! §1 (the "S" in IDRS) and
//! `cybernetic-health-harness.md` §14.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ulid::Ulid;

/// Schema tag persisted on every `Event`. Readers that do not
/// recognise the version refuse the record rather than silently
/// downgrade.
pub const EVENT_SCHEMA: &str = "harness.event.v1";

/// Journal row / event-stream identifier. A ULID is used so rows
/// sort by time naturally and collisions are vanishingly unlikely
/// across a single host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EventId(pub Ulid);

impl EventId {
    /// Allocate a fresh ID from the current wall clock.
    #[must_use]
    pub fn new() -> Self {
        Self(Ulid::new())
    }
}

impl Default for EventId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for EventId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Five-valued severity band used across journal events.
///
/// See `cybernetic-health-harness.md` §8 for the symptom ladder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Normal-state observation.
    Info,
    /// Soft threshold breach, baseline drift, or a condition we
    /// intend to act on within the next cadence.
    Warn,
    /// Statistically significant anomaly; likely requires action.
    Alert,
    /// Known-dangerous hard threshold crossed; action now.
    Crit,
}

/// Event scope. Host-observation rows land with `Host`;
/// proprioception rows land with `Self_`
/// (see [ADR-0015](../../../docs/adr/0015-proprioception-self-health.md)).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    /// Event is about the host machine.
    #[default]
    Host,
    /// Event is about Russell itself.
    #[serde(rename = "self")]
    Self_,
}

fn default_schema() -> String {
    EVENT_SCHEMA.to_string()
}

/// A single structured log record conforming to
/// `harness.event.v1`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// Stable identifier for the event row.
    #[serde(default)]
    pub id: EventId,
    /// Timestamp in RFC 3339, UTC.
    pub ts: String,
    /// Unix seconds equivalent of `ts`, for cheap range queries.
    pub ts_unix: i64,
    /// Schema tag. Expected to equal [`EVENT_SCHEMA`]; callers
    /// validate via [`Event::schema_matches`] and refuse unknown
    /// versions rather than downgrade.
    #[serde(default = "default_schema")]
    pub schema: String,
    /// Correlation ID scoping a multi-step run (e.g. a Doctor
    /// triage). Optional because Sentinel samples are standalone.
    pub run_id: Option<String>,
    /// Tier, if this event originates from a tiered module.
    /// One of `"daily" | "weekly" | "monthly" | "quarterly" | "sentinel" | "doctor" | "proprio"`.
    pub tier: Option<String>,
    /// Module / skill / probe name.
    pub module: Option<String>,
    /// Severity band.
    pub severity: Severity,
    /// Scope: host vs. self.
    #[serde(default)]
    pub scope: Scope,
    /// Short verb describing the action: `"observe"`, `"would_restart"`,
    /// `"restart"`, `"confirmed"`, etc.
    pub action: String,
    /// Whether this record describes a dry-run invocation.
    #[serde(default)]
    pub dry_run: bool,
    /// Structured free-form inputs (probe name, parameters).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub inputs: BTreeMap<String, serde_json::Value>,
    /// Structured free-form outputs (counters, measurements).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub outputs: BTreeMap<String, serde_json::Value>,
    /// Evidence bundle reference, if one was produced.
    pub evidence_ref: Option<String>,
    /// Wall-clock duration of the action in milliseconds.
    pub duration_ms: Option<u64>,
    /// Human-readable summary (one line).
    pub summary: Option<String>,
}

impl Event {
    /// Build a minimal `Event` with current time and the given
    /// severity / action. Callers populate fields further via the
    /// struct-update syntax.
    #[must_use]
    pub fn new(action: impl Into<String>, severity: Severity) -> Self {
        Self {
            id: EventId::new(),
            ts: crate::time::now_rfc3339(),
            ts_unix: crate::time::now_unix(),
            schema: EVENT_SCHEMA.to_string(),
            run_id: None,
            tier: None,
            module: None,
            severity,
            scope: Scope::Host,
            action: action.into(),
            dry_run: false,
            inputs: BTreeMap::new(),
            outputs: BTreeMap::new(),
            evidence_ref: None,
            duration_ms: None,
            summary: None,
        }
    }

    /// Returns `true` iff the record schema matches this build.
    #[must_use]
    pub fn schema_matches(&self) -> bool {
        self.schema == EVENT_SCHEMA
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_json() {
        let e = Event {
            module: Some("daily/gpu-sanity".into()),
            tier: Some("daily".into()),
            summary: Some("ok".into()),
            ..Event::new("observe", Severity::Info)
        };
        let j = serde_json::to_string(&e).unwrap();
        let back: Event = serde_json::from_str(&j).unwrap();
        assert_eq!(back.module.as_deref(), Some("daily/gpu-sanity"));
        assert!(back.schema_matches());
    }

    #[test]
    fn unknown_schema_flagged() {
        let j = r#"{
            "ts":"2026-04-17T00:00:00Z",
            "ts_unix":1776556800,
            "schema":"harness.event.v999",
            "run_id":null,"tier":null,"module":null,
            "severity":"info","scope":"host","action":"observe","dry_run":false,
            "evidence_ref":null,"duration_ms":null,"summary":null
        }"#;
        let parsed: Event = serde_json::from_str(j).expect("parses");
        assert!(!parsed.schema_matches());
    }

    #[test]
    fn severity_serializes_lowercase() {
        let s = serde_json::to_string(&Severity::Alert).unwrap();
        assert_eq!(s, "\"alert\"");
    }

    #[test]
    fn scope_defaults_to_host() {
        let j = r#"{
            "ts":"2026-04-17T00:00:00Z","ts_unix":0,
            "severity":"info","action":"observe",
            "run_id":null,"tier":null,"module":null,
            "evidence_ref":null,"duration_ms":null,"summary":null
        }"#;
        let parsed: Event = serde_json::from_str(j).expect("parses");
        assert_eq!(parsed.scope, Scope::Host);
        assert!(parsed.schema_matches(), "default schema applied");
    }
}

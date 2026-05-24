// SPDX-License-Identifier: MIT OR Apache-2.0
//! Reflex arc engine — autonomous corrective actions.
//!
//! A reflex arc is a mapping from a (probe, severity) pair to a
//! skill intervention. When a threshold breach event fires at or
//! above the arc's minimum severity, the reflex engine proposes
//! the intervention. If the risk band is within the auto-execute
//! cap, the intervention fires immediately. Otherwise it escalates
//! to the operator.
//!
//! ## Schema
//!
//! Each file is tagged `schema = "russell.reflex.v1"`:
//!
//! ```toml
//! schema = "russell.reflex.v1"
//!
//! [[arc]]
//! probe = "disk_root_used_pct"
//! min_severity = "alert"
//! intervention = "sysadmin/sweep-caches"
//! cooldown_secs = 3600
//! max_retries = 3
//! ```

use std::collections::VecDeque;

use serde::Deserialize;

use crate::event::Severity;

/// Schema tag for versioned reflex arc files.
pub const REFLEX_SCHEMA: &str = "russell.reflex.v1";

/// One reflex arc: a probe + severity trigger → intervention mapping.
#[derive(Debug, Clone, Deserialize)]
pub struct ReflexArc {
    /// Probe name that triggers this arc, e.g. `"disk_root_used_pct"`.
    pub probe: String,
    /// Minimum severity that triggers the arc. `"warn"`, `"alert"`,
    /// or `"crit"`. Defaults to `"alert"`.
    #[serde(default = "default_min_severity")]
    pub min_severity: Severity,
    /// Intervention ID in `skill_id/intervention_id` format.
    pub intervention: String,
    /// Minimum seconds between successive firings of this arc for
    /// the same probe. Prevents oscillation.
    #[serde(default = "default_cooldown")]
    pub cooldown_secs: i64,
    /// Maximum automatic retries before escalating to operator.
    /// After this many failures, further breaches only notify.
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
}

fn default_min_severity() -> Severity {
    Severity::Alert
}

const fn default_cooldown() -> i64 {
    3600
}

const fn default_max_retries() -> u32 {
    3
}

/// Root document for a reflex arc file.
#[derive(Debug, Clone, Deserialize)]
struct ReflexFile {
    #[serde(default)]
    schema: Option<String>,
    #[serde(default)]
    arc: Vec<ReflexArc>,
}

/// A loaded collection of reflex arcs, keyed by probe name.
#[derive(Debug, Clone, Default)]
pub struct ReflexSet {
    arcs: Vec<ReflexArc>,
}

impl ReflexSet {
    /// Create an empty reflex set.
    #[must_use]
    pub fn new() -> Self {
        Self { arcs: Vec::new() }
    }

    /// Load built-in default reflex arcs.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self {
            arcs: default_arcs(),
        }
    }

    /// Load arcs from a directory of `*.toml` files. Operator overrides
    /// take precedence: any arc with a matching probe name replaces the
    /// existing built-in.
    pub fn load_from_dir(&mut self, dir: &std::path::Path) {
        let entries = match std::fs::read_dir(dir) {
            Ok(d) => d,
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    tracing::debug!(dir = %dir.display(), "reflex.d not found — using defaults only");
                } else {
                    tracing::warn!(dir = %dir.display(), error = %e, "cannot read reflex.d");
                }
                return;
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|e| e != "toml") {
                continue;
            }

            let content = match std::fs::read_to_string(&path) {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(path = %path.display(), error = %e, "skipping unreadable reflex file");
                    continue;
                }
            };

            let file: ReflexFile = match toml::from_str(&content) {
                Ok(f) => f,
                Err(e) => {
                    tracing::warn!(path = %path.display(), error = %e, "skipping malformed reflex file");
                    continue;
                }
            };

            if let Some(ref schema) = file.schema
                && schema != REFLEX_SCHEMA
            {
                tracing::warn!(
                    path = %path.display(),
                    expected = REFLEX_SCHEMA,
                    found = %schema,
                    "skipping reflex file with unknown schema",
                );
                continue;
            }

            for arc in file.arc {
                if let Some(pos) = self.arcs.iter().position(|a| a.probe == arc.probe) {
                    tracing::debug!(probe = %arc.probe, "operator override for reflex arc");
                    self.arcs[pos] = arc;
                } else {
                    self.arcs.push(arc);
                }
            }
        }
    }

    /// Find the arc matching a probe and severity. Returns `None` if
    /// no arc is registered or the severity is below the arc's minimum.
    #[must_use]
    pub fn find(&self, probe: &str, severity: Severity) -> Option<&ReflexArc> {
        self.arcs
            .iter()
            .find(|a| a.probe == probe && severity >= a.min_severity)
    }

    /// Number of loaded arcs.
    #[must_use]
    pub fn len(&self) -> usize {
        self.arcs.len()
    }

    /// Whether the reflex set is empty (paired with [`len`](Self::len)).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.arcs.is_empty()
    }
}

// ─── Global reflex budget & circuit breaker (T10) ───────────────

/// Default maximum interventions per hour across all arcs.
const DEFAULT_BUDGET_PER_HOUR: u32 = 5;
/// Default consecutive failures before the breaker opens.
const DEFAULT_BREAKER_THRESHOLD: u32 = 3;

/// Global execution budget preventing cascading intervention storms.
///
/// Tracks the rolling count of interventions within the budget window
/// and opens a circuit breaker after consecutive failures.
///
/// ## Design rationale
///
/// Per-arc cooldowns prevent a single arc from oscillating, but cannot
/// prevent N arcs from firing simultaneously. The budget caps total
/// intervention throughput. The circuit breaker detects cascading
/// failures (3 consecutive interventions fail → halt all reflex
/// activity and escalate to operator).
#[derive(Debug, Clone)]
pub struct ReflexBudget {
    /// Maximum interventions allowed in the budget window.
    max_per_hour: u32,
    /// Rolling record of intervention timestamps (epoch seconds).
    recent_firings: VecDeque<i64>,
    /// Consecutive failure counter — resets on success.
    consecutive_failures: u32,
    /// Threshold for opening the circuit breaker.
    breaker_threshold: u32,
    /// Whether the circuit breaker is currently open (tripped).
    breaker_open: bool,
}

/// Result of a budget check before reflex execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetVerdict {
    /// Intervention may proceed.
    Allowed,
    /// Budget exhausted — too many interventions this hour.
    BudgetExhausted,
    /// Circuit breaker tripped — consecutive failures exceeded threshold.
    BreakerOpen,
}

impl ReflexBudget {
    /// Create a budget with default thresholds.
    ///
    /// ## Q10: Reflex Budget Persistence
    ///
    /// The budget is now journal-backed — it queries for `reflex_proposed`
    /// events in the last hour to enforce the hourly limit across
    /// `sentinel-once` invocations. Without this, each invocation would
    /// start with a fresh budget (ineffective rate limiting).
    #[must_use]
    pub fn new() -> Self {
        Self {
            max_per_hour: DEFAULT_BUDGET_PER_HOUR,
            recent_firings: VecDeque::new(),
            consecutive_failures: 0,
            breaker_threshold: DEFAULT_BREAKER_THRESHOLD,
            breaker_open: false,
        }
    }

    /// Create with custom thresholds.
    #[must_use]
    pub fn with_limits(max_per_hour: u32, breaker_threshold: u32) -> Self {
        Self {
            max_per_hour,
            recent_firings: VecDeque::new(),
            consecutive_failures: 0,
            breaker_threshold,
            breaker_open: false,
        }
    }

    /// Create a budget initialized from the journal.
    ///
    /// Queries the journal for `reflex_proposed` events in the last hour
    /// and pre-populates `recent_firings`. This makes the budget effective
    /// across multiple `sentinel-once` invocations.
    ///
    /// # Arguments
    ///
    /// * `reader` - Journal reader to query for recent reflex events
    /// * `now_unix` - Current unix timestamp for window calculation
    pub fn from_journal(reader: &crate::journal::JournalReader, now_unix: i64) -> Self {
        let window_start = now_unix - 3600; // 1 hour ago

        // Query reflex_proposed events in the last hour.
        let recent_events = reader
            .list_events_by_action("reflex_proposed", window_start, now_unix)
            .unwrap_or_default();

        // Convert RFC 3339 timestamps to unix timestamps and populate the budget.
        let mut recent_firings: VecDeque<i64> = recent_events
            .into_iter()
            .filter_map(|e| {
                // Parse RFC 3339 timestamp to unix timestamp.
                // Format: 2026-05-20T12:34:56+00:00 or similar.
                time::OffsetDateTime::parse(&e.ts, &time::format_description::well_known::Rfc3339)
                    .map(|odt| odt.unix_timestamp())
                    .ok()
            })
            .collect();

        // Sort by timestamp (oldest first) for efficient eviction.
        recent_firings.make_contiguous().sort();

        // Restore circuit breaker state by checking for recent breaker-open events.
        let breaker_open = reader
            .list_events_by_action("reflex_breaker_open", now_unix - 3600, now_unix)
            .unwrap_or_default()
            .iter()
            .any(|e| {
                time::OffsetDateTime::parse(&e.ts, &time::format_description::well_known::Rfc3339)
                    .map(|odt| odt.unix_timestamp() > now_unix - 3600)
                    .unwrap_or(false)
            });

        Self {
            max_per_hour: DEFAULT_BUDGET_PER_HOUR,
            recent_firings,
            consecutive_failures: 0,
            breaker_threshold: DEFAULT_BREAKER_THRESHOLD,
            breaker_open,
        }
    }

    /// Check whether a new intervention is allowed right now.
    ///
    /// Call this before dispatching a reflex intervention. If the
    /// verdict is not [`BudgetVerdict::Allowed`], the caller should
    /// skip execution and escalate to the operator.
    #[must_use]
    pub fn check(&mut self, now_unix: i64) -> BudgetVerdict {
        if self.breaker_open {
            return BudgetVerdict::BreakerOpen;
        }

        // Evict firings older than 1 hour.
        let window_start = now_unix - 3600;
        while self
            .recent_firings
            .front()
            .is_some_and(|&ts| ts < window_start)
        {
            self.recent_firings.pop_front();
        }

        if self.recent_firings.len() as u32 >= self.max_per_hour {
            return BudgetVerdict::BudgetExhausted;
        }

        BudgetVerdict::Allowed
    }

    /// Record that an intervention was dispatched.
    pub fn record_firing(&mut self, now_unix: i64) {
        self.recent_firings.push_back(now_unix);
    }

    /// Record an intervention outcome. Resets the failure counter
    /// on success; increments and possibly trips the breaker on failure.
    pub fn record_outcome(&mut self, success: bool) {
        if success {
            self.consecutive_failures = 0;
        } else {
            self.consecutive_failures += 1;
            if self.consecutive_failures >= self.breaker_threshold {
                self.breaker_open = true;
                tracing::warn!(
                    failures = self.consecutive_failures,
                    threshold = self.breaker_threshold,
                    "reflex circuit breaker OPEN — all reflex interventions halted"
                );
            }
        }
    }

    /// Manually reset the circuit breaker (operator action).
    pub fn reset_breaker(&mut self) {
        self.breaker_open = false;
        self.consecutive_failures = 0;
        tracing::info!("reflex circuit breaker reset by operator");
    }

    /// Whether the circuit breaker is currently open.
    #[must_use]
    pub fn is_breaker_open(&self) -> bool {
        self.breaker_open
    }

    /// Number of interventions fired in the current hour window.
    #[must_use]
    pub fn firings_this_hour(&self) -> u32 {
        self.recent_firings.len() as u32
    }
}

impl Default for ReflexBudget {
    fn default() -> Self {
        Self::new()
    }
}

/// Built-in default reflex arcs shipped with Russell.
const DEFAULT_ARCS_TOML: &str = include_str!("defaults.toml");

fn default_arcs() -> Vec<ReflexArc> {
    let file: ReflexFile = toml::from_str(DEFAULT_ARCS_TOML)
        .expect("embedded reflex defaults.toml must be valid TOML");
    file.arc
}

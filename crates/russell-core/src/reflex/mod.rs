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
    #[cfg(test)]
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
    #[cfg(test)]
    #[must_use]
    pub fn len(&self) -> usize {
        self.arcs.len()
    }

    /// Whether the reflex set is empty (paired with [`len`](Self::len)).
    #[cfg(test)]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.arcs.is_empty()
    }
}

/// Built-in default reflex arcs shipped with Russell.
const DEFAULT_ARCS_TOML: &str = include_str!("defaults.toml");

fn default_arcs() -> Vec<ReflexArc> {
    let file: ReflexFile = toml::from_str(DEFAULT_ARCS_TOML)
        .expect("embedded reflex defaults.toml must be valid TOML");
    file.arc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_set_returns_none() {
        let rs = ReflexSet::new();
        assert!(rs.find("disk_root_used_pct", Severity::Alert).is_none());
    }

    #[test]
    fn defaults_have_disk_arc() {
        let rs = ReflexSet::with_defaults();
        assert!(!rs.is_empty());
        let arc = rs.find("disk_root_used_pct", Severity::Alert);
        assert!(arc.is_some());
        let arc = arc.unwrap();
        assert_eq!(arc.probe, "disk_root_used_pct");
        assert_eq!(arc.intervention, "sysadmin/sweep-caches");
        assert_eq!(arc.cooldown_secs, 3600);
        assert_eq!(arc.max_retries, 3);
    }

    #[test]
    fn below_min_severity_returns_none() {
        let rs = ReflexSet::with_defaults();
        assert!(rs.find("disk_root_used_pct", Severity::Warn).is_none());
    }

    #[test]
    fn crit_triggers_alert_arc() {
        let rs = ReflexSet::with_defaults();
        assert!(rs.find("disk_root_used_pct", Severity::Crit).is_some());
    }
}

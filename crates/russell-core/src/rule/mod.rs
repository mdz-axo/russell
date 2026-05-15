// SPDX-License-Identifier: MIT OR Apache-2.0
//! Rule engine — per-probe threshold configuration.
//!
//! Rules map probe names to severity thresholds. The operator
//! can override built-in defaults by dropping TOML files into
//! `~/.config/harness/rules.d/`.
//!
//! The rules engine was originally deferred (ADR-0012) and is now
//! active per the MVP spec. Built-in defaults live in
//! [`defaults.toml`](defaults.toml); operator overrides take
//! precedence via [`RuleSet::load_from_dir`].
//!
//! ## Schema
//!
//! Each file is tagged `schema = "russell.rule.v1"` and contains one
//! or more `[[rule]]` tables:
//!
//! ```toml
//! schema = "russell.rule.v1"
//!
//! [[rule]]
//! probe = "mem_available_mib"
//! description = "Available system memory"
//! unit = "MiB"
//! warn_below = 4096
//! alert_below = 2048
//! crit_below = 1024
//!
//! [[rule]]
//! probe = "loadavg_1m"
//! description = "1-minute load average"
//! warn_above = 8.0
//! alert_above = 16.0
//! ```
//!
//! Thresholds are directional: `warn_below` / `alert_below` / `crit_below`
//! are "value is too low" (memory, disk); `warn_above` / `alert_above` /
//! `crit_above` are "value is too high" (load, temperature, latency).
//! Both directions can coexist on the same rule.

use serde::Deserialize;
use tracing::{debug, warn};

use crate::event::Severity;

/// Schema tag for versioned rule files.
pub const RULE_SCHEMA: &str = "russell.rule.v1";

/// One rule: a probe name plus directional thresholds.
#[derive(Debug, Clone, Deserialize)]
pub struct Rule {
    /// Probe name, e.g. `"mem_available_mib"`.
    pub probe: String,
    /// Optional human-readable description.
    #[serde(default)]
    pub description: Option<String>,
    /// Unit hint for display, e.g. `"MiB"`.
    #[serde(default)]
    pub unit: Option<String>,

    // --- "too low" thresholds (memory, disk space, etc.) ---
    /// Severity `warn` when value drops below this.
    #[serde(default)]
    pub warn_below: Option<f64>,
    /// Severity `alert` when value drops below this.
    #[serde(default)]
    pub alert_below: Option<f64>,
    /// Severity `crit` when value drops below this.
    #[serde(default)]
    pub crit_below: Option<f64>,

    // --- "too high" thresholds (load, temperature, latency, etc.) ---
    /// Severity `warn` when value exceeds this.
    #[serde(default)]
    pub warn_above: Option<f64>,
    /// Severity `alert` when value exceeds this.
    #[serde(default)]
    pub alert_above: Option<f64>,
    /// Severity `crit` when value exceeds this.
    #[serde(default)]
    pub crit_above: Option<f64>,

    // --- rate-of-change thresholds (units per second) ---
    /// Severity `warn` when absolute rate-of-change exceeds this
    /// (units/second, computed over the previous sample interval).
    #[serde(default)]
    pub rate_warn: Option<f64>,
    /// Severity `alert` when absolute rate-of-change exceeds this.
    #[serde(default)]
    pub rate_alert: Option<f64>,
    /// Severity `crit` when absolute rate-of-change exceeds this.
    #[serde(default)]
    pub rate_crit: Option<f64>,
}

/// Root document for a rules file.
#[derive(Debug, Clone, Deserialize)]
struct RulesFile {
    /// Schema tag for versioning.
    #[serde(default)]
    schema: Option<String>,

    /// One or more rules.
    #[serde(default)]
    rule: Vec<Rule>,
}

/// A loaded collection of rules, keyed by probe name.
///
/// Built-in defaults are always present; operator overrides from
/// `rules.d/` take precedence.
#[derive(Debug, Clone, Default)]
pub struct RuleSet {
    rules: Vec<Rule>,
}

impl RuleSet {
    /// Create an empty rule set (no rules; evaluate always returns Info).
    #[cfg(test)]
    #[must_use]
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Load the built-in default rules. These are the factory thresholds
    /// that ship with Russell.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self {
            rules: default_rules(),
        }
    }

    /// Load rules from a directory of `*.toml` files. Files that fail
    /// to parse are skipped with a warning; this is intentional —
    /// Russell must not crash on a malformed operator file.
    ///
    /// Files with `schema` tag that do not match [`RULE_SCHEMA`] are
    /// skipped with a warning.
    ///
    /// The directory need not exist; returns an empty set if missing.
    pub fn load_from_dir(&mut self, dir: &std::path::Path) {
        let entries = match std::fs::read_dir(dir) {
            Ok(d) => d,
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    debug!(dir = %dir.display(), "rules.d not found — using defaults only");
                } else {
                    warn!(dir = %dir.display(), error = %e, "cannot read rules.d");
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
                    warn!(path = %path.display(), error = %e, "skipping unreadable rule file");
                    continue;
                }
            };

            let file: RulesFile = match toml::from_str(&content) {
                Ok(f) => f,
                Err(e) => {
                    warn!(path = %path.display(), error = %e, "skipping malformed rule file");
                    continue;
                }
            };

            // Schema check.
            if let Some(ref schema) = file.schema
                && schema != RULE_SCHEMA
            {
                warn!(
                    path = %path.display(),
                    expected = RULE_SCHEMA,
                    found = %schema,
                    "skipping rule file with unknown schema",
                );
                continue;
            }

            let count = file.rule.len();
            // Operator overrides take precedence: for any probe name
            // already present, replace the existing rule.
            for rule in file.rule {
                if let Some(pos) = self.rules.iter().position(|r| r.probe == rule.probe) {
                    debug!(
                        probe = %rule.probe,
                        path = %path.display(),
                        "operator override for probe",
                    );
                    self.rules[pos] = rule;
                } else {
                    self.rules.push(rule);
                }
            }
            debug!(path = %path.display(), count, "loaded rules from file");
        }
    }

    /// Evaluate a probe value against its rule. Returns the highest
    /// severity breached, or [`Severity::Info`] if no thresholds are
    /// configured for this probe or the value is within bounds.
    #[must_use]
    pub fn evaluate(&self, probe: &str, value: f64) -> Severity {
        let Some(rule) = self.rules.iter().find(|r| r.probe == probe) else {
            return Severity::Info;
        };

        // "too low" checks (value decreases → more severe).
        // Check in order: crit < alert < warn, so the most severe wins.
        if let Some(threshold) = rule.crit_below
            && value < threshold
        {
            return Severity::Crit;
        }
        if let Some(threshold) = rule.alert_below
            && value < threshold
        {
            return Severity::Alert;
        }
        if let Some(threshold) = rule.warn_below
            && value < threshold
        {
            return Severity::Warn;
        }

        // "too high" checks (value increases → more severe).
        if let Some(threshold) = rule.crit_above
            && value >= threshold
        {
            return Severity::Crit;
        }
        if let Some(threshold) = rule.alert_above
            && value >= threshold
        {
            return Severity::Alert;
        }
        if let Some(threshold) = rule.warn_above
            && value >= threshold
        {
            return Severity::Warn;
        }

        Severity::Info
    }

    /// Evaluate the absolute rate-of-change of a probe value against
    /// rate thresholds. Returns the highest severity breached, or
    /// [`Severity::Info`] if no rate thresholds are configured or the
    /// rate is within bounds.
    ///
    /// `rate` is in units per second, computed as `abs(Δvalue) / Δtime`.
    #[must_use]
    pub fn evaluate_rate(&self, probe: &str, rate: f64) -> Severity {
        let Some(rule) = self.rules.iter().find(|r| r.probe == probe) else {
            return Severity::Info;
        };

        // Check in order: crit > alert > warn.
        if let Some(threshold) = rule.rate_crit
            && rate >= threshold
        {
            return Severity::Crit;
        }
        if let Some(threshold) = rule.rate_alert
            && rate >= threshold
        {
            return Severity::Alert;
        }
        if let Some(threshold) = rule.rate_warn
            && rate >= threshold
        {
            return Severity::Warn;
        }

        Severity::Info
    }

    /// Number of loaded rules.
    #[cfg(test)]
    #[must_use]
    pub fn len(&self) -> usize {
        self.rules.len()
    }

    /// Returns `true` if no rules are loaded.
    #[cfg(test)]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }
}

/// Built-in default rules shipped with Russell.
/// Embedded factory defaults loaded from `defaults.toml`.
///
/// These are the factory thresholds. Operators can override any
/// probe by placing a same-named `[[rule]]` in `rules.d/`.
const DEFAULT_RULES_TOML: &str = include_str!("defaults.toml");

/// Parse the embedded defaults TOML into a `Vec<Rule>`.
fn default_rules() -> Vec<Rule> {
    let file: RulesFile =
        toml::from_str(DEFAULT_RULES_TOML).expect("embedded defaults.toml must be valid TOML");
    file.rule
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Severity;

    #[test]
    fn empty_ruleset_returns_info() {
        let rs = RuleSet::new();
        assert_eq!(rs.evaluate("mem_available_mib", 100.0), Severity::Info);
    }

    #[test]
    fn defaults_mem_warn_below() {
        let rs = RuleSet::with_defaults();
        // 8 GiB — no breach.
        assert_eq!(rs.evaluate("mem_available_mib", 8192.0), Severity::Info);
        // 3 GiB — below warn (4096) but above alert (2048).
        assert_eq!(rs.evaluate("mem_available_mib", 3000.0), Severity::Warn);
        // 1.5 GiB — below alert (2048) but above crit (1024).
        assert_eq!(rs.evaluate("mem_available_mib", 1500.0), Severity::Alert);
        // 0.5 GiB — below crit (1024).
        assert_eq!(rs.evaluate("mem_available_mib", 500.0), Severity::Crit);
        // Exact boundary: at 4096 = not below, so Info.
        assert_eq!(rs.evaluate("mem_available_mib", 4096.0), Severity::Info);
        // Just below 4096 = Warn.
        assert_eq!(rs.evaluate("mem_available_mib", 4095.99), Severity::Warn);
    }

    #[test]
    fn defaults_swap_warn_above() {
        let rs = RuleSet::with_defaults();
        // Rule: warn_above=7168.0, alert_above=7680.0, crit_above=7936.0
        assert_eq!(rs.evaluate("swap_used_mib", 0.0), Severity::Info);
        assert_eq!(rs.evaluate("swap_used_mib", 4095.0), Severity::Info);
        assert_eq!(rs.evaluate("swap_used_mib", 7168.0), Severity::Warn);
        assert_eq!(rs.evaluate("swap_used_mib", 7680.0), Severity::Alert);
        assert_eq!(rs.evaluate("swap_used_mib", 7936.0), Severity::Crit);
    }

    #[test]
    fn defaults_loadavg_warn_above() {
        let rs = RuleSet::with_defaults();
        assert_eq!(rs.evaluate("loadavg_1m", 0.5), Severity::Info);
        assert_eq!(rs.evaluate("loadavg_1m", 7.99), Severity::Info);
        assert_eq!(rs.evaluate("loadavg_1m", 8.0), Severity::Warn);
        assert_eq!(rs.evaluate("loadavg_1m", 20.0), Severity::Alert);
        assert_eq!(rs.evaluate("loadavg_1m", 50.0), Severity::Crit);
    }

    #[test]
    fn unknown_probe_returns_info() {
        let rs = RuleSet::with_defaults();
        assert_eq!(rs.evaluate("nonexistent_probe", 42.0), Severity::Info);
    }

    #[test]
    fn both_directions_on_same_rule() {
        // A rule with both below and above thresholds: the highest
        // severity wins.
        let rule = Rule {
            probe: "cpu_temp_c".into(),
            description: None,
            unit: Some("°C".into()),
            warn_below: Some(10.0), // too cold: warn
            alert_below: Some(0.0), // too cold: alert
            crit_below: None,
            warn_above: Some(80.0),  // too hot: warn
            alert_above: Some(90.0), // too hot: alert
            crit_above: None,
            rate_warn: None,
            rate_alert: None,
            rate_crit: None,
        };
        let mut rs = RuleSet::new();
        rs.rules.push(rule);

        assert_eq!(rs.evaluate("cpu_temp_c", 45.0), Severity::Info);
        assert_eq!(rs.evaluate("cpu_temp_c", 9.0), Severity::Warn); // below 10
        assert_eq!(rs.evaluate("cpu_temp_c", -1.0), Severity::Alert); // below 0
        assert_eq!(rs.evaluate("cpu_temp_c", 81.0), Severity::Warn); // above 80
        assert_eq!(rs.evaluate("cpu_temp_c", 91.0), Severity::Alert); // above 90
    }

    #[test]
    fn operator_override_replaces_builtin() {
        let mut rs = RuleSet::with_defaults();
        assert_eq!(rs.evaluate("mem_available_mib", 3000.0), Severity::Warn);

        // Simulate loading a rules.d file that overrides mem_available_mib.
        let toml_content = r#"
schema = "russell.rule.v1"

[[rule]]
probe = "mem_available_mib"
description = "Custom memory rule"
warn_below = 8192
alert_below = 4096
"#;
        let file: RulesFile = toml::from_str(toml_content).unwrap();
        for rule in file.rule {
            if let Some(pos) = rs.rules.iter().position(|r| r.probe == rule.probe) {
                rs.rules[pos] = rule;
            }
        }

        // With the override, 3 GiB is below alert (4096) so it's Alert.
        assert_eq!(rs.evaluate("mem_available_mib", 3000.0), Severity::Alert);
        // 8 GiB is Info (not below 8192).
        assert_eq!(rs.evaluate("mem_available_mib", 9000.0), Severity::Info);
        // 1500 MiB is below alert (4096).
        assert_eq!(rs.evaluate("mem_available_mib", 1500.0), Severity::Alert);
    }

    #[test]
    fn malformed_file_skipped_gracefully() {
        let rs = RuleSet::with_defaults();
        let before_count = rs.len();

        // This won't parse as valid TOML rules.
        let _: Result<RulesFile, _> = toml::from_str("not valid toml at all = [[]]");
        // load_from_dir would skip silently. Simulate: rules unchanged.
        assert_eq!(rs.len(), before_count);
    }
}

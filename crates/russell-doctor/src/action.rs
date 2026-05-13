// SPDX-License-Identifier: MIT OR Apache-2.0
//! ACTION: protocol parser and resolver.
//!
//! The LLM (Jack) may propose actions using the `ACTION:` syntax:
//!
//! ```text
//! ACTION: <skill-id>/<probe-or-intervention-id>
//! ```
//!
//! This module provides a single entry point, [`resolve`], that:
//!
//! 1. Extracts the last `ACTION:` line from a response.
//! 2. Looks up the referenced skill among the loaded set.
//! 3. Returns a typed [`ResolvedAction`] (probe or intervention)
//!    with all metadata the caller needs to execute.
//!
//! Probes are read-only, risk:none, and may auto-execute.
//! Interventions require operator consent per JR-2.
//!
//! Both `russell jack` and `russell chat` use this module,
//! eliminating duplicated parsing and resolution logic.

use russell_skills::{RiskBand, Skill};

/// A resolved ACTION — either a probe (read-only) or an intervention
/// (mutating, requires consent per JR-2).
#[derive(Debug, Clone)]
pub enum ResolvedAction {
    /// Read-only probe. Auto-executable (risk: none).
    Probe {
        /// Skill that owns the probe.
        skill_id: String,
        /// Probe ID.
        action_id: String,
        /// Argv to execute.
        cmd: Vec<String>,
        /// Skill's max-auto-risk cap.
        max_auto_risk: RiskBand,
    },
    /// Mutating intervention. Requires operator consent.
    Intervention {
        /// Skill that owns the intervention.
        skill_id: String,
        /// Intervention ID.
        action_id: String,
        /// Argv to execute.
        cmd: Vec<String>,
        /// Risk band declared in the manifest.
        risk: RiskBand,
        /// Whether this requires sudo.
        needs_sudo: bool,
        /// Skill's max-auto-risk cap.
        max_auto_risk: RiskBand,
        /// Whether the manifest's safety.require_human_for lists this ID.
        requires_human: bool,
    },
}

impl ResolvedAction {
    /// Returns `true` if this is a probe (read-only, auto-execute).
    #[must_use]
    pub fn is_probe(&self) -> bool {
        matches!(self, Self::Probe { .. })
    }

    /// The skill ID, regardless of action type.
    #[must_use]
    pub fn skill_id(&self) -> &str {
        match self {
            Self::Probe { skill_id, .. } => skill_id,
            Self::Intervention { skill_id, .. } => skill_id,
        }
    }

    /// The action (probe or intervention) ID.
    #[must_use]
    pub fn action_id(&self) -> &str {
        match self {
            Self::Probe { action_id, .. } => action_id,
            Self::Intervention { action_id, .. } => action_id,
        }
    }

    /// The command argv.
    #[must_use]
    pub fn cmd(&self) -> &[String] {
        match self {
            Self::Probe { cmd, .. } => cmd,
            Self::Intervention { cmd, .. } => cmd,
        }
    }
}

/// Error returned when an ACTION: line cannot be resolved.
#[derive(Debug, Clone)]
pub enum ActionError {
    /// The ACTION: prefix was malformed.
    MalformedPrefix {
        /// The raw line that could not be parsed.
        raw: String,
    },
    /// Missing `/` separator between skill and action ID.
    MissingSeparator {
        /// The spec string without the separator.
        spec: String,
    },
    /// Skill or action ID was empty.
    EmptyId {
        /// The spec string with empty components.
        spec: String,
    },
    /// Referenced skill is not loaded.
    UnknownSkill {
        /// The skill ID that was not found.
        skill_id: String,
        /// List of all loaded skill IDs for diagnostics.
        loaded: Vec<String>,
    },
    /// Referenced action is not a known probe or intervention.
    UnknownAction {
        /// Skill that was found but doesn't contain this action.
        skill_id: String,
        /// The action ID that was not found.
        action_id: String,
        /// Available probe IDs in this skill.
        probes: Vec<String>,
        /// Available intervention IDs in this skill.
        interventions: Vec<String>,
    },
}

impl std::fmt::Display for ActionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MalformedPrefix { raw } => {
                write!(
                    f,
                    "Jack proposed an action but the parser couldn't strip the ACTION: prefix. Raw line: `{raw}`"
                )
            }
            Self::MissingSeparator { spec } => {
                write!(
                    f,
                    "'ACTION: {spec}' — missing `/` separator between skill and action ID. Use ACTION: <skill>/<action>."
                )
            }
            Self::EmptyId { spec } => {
                write!(f, "'ACTION: {spec}' — skill ID or action ID is empty.")
            }
            Self::UnknownSkill { skill_id, loaded } => {
                write!(
                    f,
                    "'{skill_id}' is not a loaded skill. Loaded skills: {loaded:?}"
                )
            }
            Self::UnknownAction {
                skill_id,
                action_id,
                probes,
                interventions,
            } => {
                write!(
                    f,
                    "'{action_id}' is not a known action in skill '{skill_id}'. Available probes: {probes:?}, interventions: {interventions:?}"
                )
            }
        }
    }
}

/// Parse the last `ACTION:` line from a response and resolve it
/// against the loaded skill set.
///
/// Returns `None` if no `ACTION:` line is present in the response.
///
/// # Errors
///
/// Returns [`ActionError`] if an ACTION: line is present but cannot
/// be parsed or resolved — the caller should surface the error to
/// the operator so they can see what went wrong.
pub fn resolve(
    response: &str,
    skills: &[Skill],
) -> Option<std::result::Result<ResolvedAction, ActionError>> {
    let action_line = response
        .lines()
        .rev()
        .find(|line| line.trim().starts_with("ACTION:"))?;

    let raw = action_line.trim();
    let spec = match raw.strip_prefix("ACTION:") {
        Some(s) if !s.trim().is_empty() => s.trim(),
        _ => {
            return Some(Err(ActionError::MalformedPrefix {
                raw: raw.to_string(),
            }));
        }
    };

    let (skill_id, action_id) = match spec.split_once('/') {
        Some((a, b)) if !a.trim().is_empty() && !b.trim().is_empty() => (a.trim(), b.trim()),
        Some((_, _)) => {
            return Some(Err(ActionError::EmptyId {
                spec: spec.to_string(),
            }));
        }
        None => {
            return Some(Err(ActionError::MissingSeparator {
                spec: spec.to_string(),
            }));
        }
    };

    let skill = match skills.iter().find(|s| s.id == skill_id) {
        Some(s) => s,
        None => {
            let loaded: Vec<String> = skills.iter().map(|s| s.id.clone()).collect();
            return Some(Err(ActionError::UnknownSkill {
                skill_id: skill_id.to_string(),
                loaded,
            }));
        }
    };

    // Check probes first — they're read-only and auto-execute.
    if let Some(probe) = skill.probes.iter().find(|p| p.id == action_id) {
        return Some(Ok(ResolvedAction::Probe {
            skill_id: skill_id.to_string(),
            action_id: action_id.to_string(),
            cmd: probe.cmd.clone(),
            max_auto_risk: skill.safety.max_auto_risk,
        }));
    }

    // Then check interventions.
    if let Some(iv) = skill.interventions.iter().find(|i| i.id == action_id) {
        let requires_human = skill.safety.require_human_for.contains(&iv.id);
        return Some(Ok(ResolvedAction::Intervention {
            skill_id: skill_id.to_string(),
            action_id: action_id.to_string(),
            cmd: iv.cmd.clone(),
            risk: iv.risk,
            needs_sudo: iv.needs_sudo,
            max_auto_risk: skill.safety.max_auto_risk,
            requires_human,
        }));
    }

    let probes: Vec<String> = skill.probes.iter().map(|p| p.id.clone()).collect();
    let interventions: Vec<String> = skill.interventions.iter().map(|i| i.id.clone()).collect();
    Some(Err(ActionError::UnknownAction {
        skill_id: skill_id.to_string(),
        action_id: action_id.to_string(),
        probes,
        interventions,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use russell_skills::{Intervention, Probe, Safety};

    fn make_skill() -> Skill {
        Skill {
            id: "test-skill".into(),
            version: "0.1.0".into(),
            authored: "2026-01-01".into(),
            min_harness_version: "0.1.0".into(),
            symptoms: vec![],
            applies_when: vec![],
            probes: vec![Probe {
                id: "probe-1".into(),
                cmd: vec!["echo".into(), "hello".into()],
                capture: "stdout".into(),
                timeout: "30s".into(),
            }],
            interventions: vec![Intervention {
                id: "iv-1".into(),
                cmd: vec!["echo".into(), "fix".into()],
                risk: RiskBand::Low,
                idempotent: true,
                rollback: russell_skills::Rollback::NoneNeeded {
                    rollback: russell_skills::RollbackNone::NoneNeeded,
                },
                timeout: "120s".into(),
                needs_sudo: false,
            }],
            safety: Safety {
                max_auto_risk: RiskBand::Low,
                require_human_for: vec![],
            },
            evaluation: None,
        }
    }

    #[test]
    fn no_action_line_returns_none() {
        let skills = [make_skill()];
        assert!(resolve("hello world", &skills).is_none());
    }

    #[test]
    fn resolves_probe() {
        let skills = [make_skill()];
        let result = resolve("ACTION: test-skill/probe-1", &skills)
            .unwrap()
            .unwrap();
        assert!(result.is_probe());
        assert_eq!(result.skill_id(), "test-skill");
        assert_eq!(result.action_id(), "probe-1");
    }

    #[test]
    fn resolves_intervention() {
        let skills = [make_skill()];
        let result = resolve("ACTION: test-skill/iv-1", &skills)
            .unwrap()
            .unwrap();
        assert!(!result.is_probe());
        assert_eq!(result.action_id(), "iv-1");
    }

    #[test]
    fn action_line_at_end_of_response() {
        let skills = [make_skill()];
        let response = "Here's what I found.\n\nLet me check further.\nACTION: test-skill/probe-1";
        let result = resolve(response, &skills).unwrap().unwrap();
        assert!(result.is_probe());
    }

    #[test]
    fn unknown_skill_is_error() {
        let skills = [make_skill()];
        let err = resolve("ACTION: bad-skill/probe-1", &skills)
            .unwrap()
            .unwrap_err();
        assert!(err.to_string().contains("bad-skill"));
    }

    #[test]
    fn unknown_action_is_error() {
        let skills = [make_skill()];
        let err = resolve("ACTION: test-skill/nonexistent", &skills)
            .unwrap()
            .unwrap_err();
        assert!(err.to_string().contains("nonexistent"));
    }

    #[test]
    fn missing_separator_is_error() {
        let skills = [make_skill()];
        let err = resolve("ACTION: test-skill", &skills).unwrap().unwrap_err();
        assert!(err.to_string().contains("/"));
    }

    #[test]
    fn empty_ids_are_error() {
        let skills = [make_skill()];
        let err = resolve("ACTION: /", &skills).unwrap().unwrap_err();
        assert!(err.to_string().contains("empty"));
    }
}

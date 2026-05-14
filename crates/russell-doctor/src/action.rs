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

use russell_skills::{RiskBand, Rollback, Skill};

/// A resolved ACTION — either a probe (read-only), an intervention
/// (mutating, requires consent per JR-2), or a Kask MCP tool call
/// (ADR-0025).
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
        /// Rollback strategy from the manifest (for IDRS-R).
        rollback_id: Option<String>,
        /// Rollback command argv, if rollback is via a named intervention.
        rollback_cmd: Option<Vec<String>>,
        /// Whether rollback requires a reboot.
        rollback_is_reboot: bool,
        /// Post-intervention evaluation checks from the skill manifest.
        eval_checks: Vec<EvalCheckInfo>,
    },
    /// Kask MCP tool call (ADR-0025). Executed via the MCP client,
    /// not the local skill dispatcher.
    KaskTool {
        /// The MCP tool name (from `tools/list`).
        tool_name: String,
        /// Risk band from tool annotations. Defaults to `Medium` when
        /// unset — safe default requiring operator consent.
        risk_band: RiskBand,
    },
}

impl ResolvedAction {
    /// Returns `true` if this is a probe (read-only, auto-execute).
    #[must_use]
    pub fn is_probe(&self) -> bool {
        matches!(self, Self::Probe { .. })
    }

    /// Returns `true` if this is a Kask MCP tool call.
    #[must_use]
    pub fn is_kask_tool(&self) -> bool {
        matches!(self, Self::KaskTool { .. })
    }

    /// Returns the risk band for this action.
    #[must_use]
    pub fn risk_band(&self) -> RiskBand {
        match self {
            Self::Probe { .. } => RiskBand::None,
            Self::Intervention { risk, .. } => *risk,
            Self::KaskTool { risk_band, .. } => *risk_band,
        }
    }

    /// The skill ID, regardless of action type.
    #[must_use]
    pub fn skill_id(&self) -> &str {
        match self {
            Self::Probe { skill_id, .. } => skill_id,
            Self::Intervention { skill_id, .. } => skill_id,
            Self::KaskTool { .. } => "kask",
        }
    }

    /// The action (probe or intervention) ID.
    #[must_use]
    pub fn action_id(&self) -> &str {
        match self {
            Self::Probe { action_id, .. } => action_id,
            Self::Intervention { action_id, .. } => action_id,
            Self::KaskTool { tool_name, .. } => tool_name,
        }
    }

    /// The command argv. Empty for Kask tools (they're MCP calls, not subprocesses).
    #[must_use]
    pub fn cmd(&self) -> &[String] {
        match self {
            Self::Probe { cmd, .. } => cmd,
            Self::Intervention { cmd, .. } => cmd,
            Self::KaskTool { .. } => &[],
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

/// Metadata for a Kask tool available in the registry, passed by
/// the caller (keeps `russell-doctor` free of `russell-mcp` dependency).
#[derive(Debug, Clone)]
pub struct KaskToolInfo {
    /// Tool name (the callable ID).
    pub name: String,
    /// Risk band from annotations. Defaults to `RiskBand::Medium`
    /// when unset — safe default per IDRS. Probes should explicitly
    /// declare `RiskBand::None`.
    pub risk_band: RiskBand,
}

/// A post-intervention evaluation check, resolved from the skill manifest.
#[derive(Debug, Clone)]
pub struct EvalCheckInfo {
    /// Unique ID within the evaluation checks.
    pub id: String,
    /// Argv to execute.
    pub cmd: Vec<String>,
    /// Expected exit code (default 0).
    pub expect_exit: i32,
    /// Timeout duration.
    pub timeout: String,
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
    resolve_with_kask(response, skills, &[])
}

/// Parse the last `ACTION:` line from a response and resolve it
/// against both the loaded skill set AND the Kask MCP tool registry.
///
/// When `skill_id == "kask"`, the action_id is validated against
/// `kask_tools` (the poka-yoke for MCP tools per ADR-0025 §7).
///
/// Returns `None` if no `ACTION:` line is present in the response.
pub fn resolve_with_kask(
    response: &str,
    skills: &[Skill],
    kask_tools: &[KaskToolInfo],
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

    // Kask MCP tool path (ADR-0025 §7).
    // Only route to Kask if tools are available (registry is populated).
    if skill_id == "kask" && !kask_tools.is_empty() {
        return resolve_kask_tool(action_id, kask_tools, skills);
    }

    let skill = match skills.iter().find(|s| s.id == skill_id) {
        Some(s) => s,
        None => {
            // Include "kask" in the loaded list if we have kask tools.
            let mut loaded: Vec<String> = skills.iter().map(|s| s.id.clone()).collect();
            if !kask_tools.is_empty() {
                loaded.push("kask".to_string());
            }
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
        let (rollback_id, rollback_cmd, rollback_is_reboot) = match &iv.rollback {
            Rollback::RollbackId { rollback_id: rid } => {
                let rb_cmd = skill
                    .interventions
                    .iter()
                    .find(|i| i.id == *rid)
                    .map(|i| i.cmd.clone());
                (Some(rid.clone()), rb_cmd, false)
            }
            Rollback::NoneNeeded { .. } => (None, None, false),
            Rollback::Reboot { .. } => (None, None, true),
        };
        let eval_checks: Vec<EvalCheckInfo> = skill
            .evaluation
            .as_ref()
            .map(|ev| {
                ev.after_intervention
                    .iter()
                    .map(|c| EvalCheckInfo {
                        id: c.id.clone(),
                        cmd: c.cmd.clone(),
                        expect_exit: c.expect_exit,
                        timeout: c.timeout.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        return Some(Ok(ResolvedAction::Intervention {
            skill_id: skill_id.to_string(),
            action_id: action_id.to_string(),
            cmd: iv.cmd.clone(),
            risk: iv.risk,
            needs_sudo: iv.needs_sudo,
            max_auto_risk: skill.safety.max_auto_risk,
            requires_human,
            rollback_id,
            rollback_cmd,
            rollback_is_reboot,
            eval_checks,
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

/// Resolve a Kask MCP tool reference against the cached tool registry.
fn resolve_kask_tool(
    tool_name: &str,
    kask_tools: &[KaskToolInfo],
    skills: &[Skill],
) -> Option<std::result::Result<ResolvedAction, ActionError>> {
    // Poka-yoke: tool must exist in the registry.
    if let Some(tool) = kask_tools.iter().find(|t| t.name == tool_name) {
        return Some(Ok(ResolvedAction::KaskTool {
            tool_name: tool.name.clone(),
            risk_band: tool.risk_band,
        }));
    }

    // Tool not found — build a diagnostic error.
    let available: Vec<String> = kask_tools.iter().map(|t| t.name.clone()).collect();
    let mut loaded: Vec<String> = skills.iter().map(|s| s.id.clone()).collect();
    if !kask_tools.is_empty() {
        loaded.push("kask".to_string());
    }

    Some(Err(ActionError::UnknownAction {
        skill_id: "kask".to_string(),
        action_id: tool_name.to_string(),
        probes: available,
        interventions: vec![],
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
    fn kask_tool_with_risk_band() {
        let skills = [make_skill()];
        let kask_tools = make_kask_tools();
        let result =
            resolve_with_kask("ACTION: kask/russell_host_snapshot", &skills, &kask_tools)
                .unwrap()
                .unwrap();
        match result {
            ResolvedAction::KaskTool { risk_band, .. } => {
                assert_eq!(risk_band, RiskBand::None);
            }
            _ => panic!("expected KaskTool"),
        }
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

    // ── Kask MCP tool resolution tests (ADR-0025) ──────────────────

    fn make_kask_tools() -> Vec<KaskToolInfo> {
        vec![
            KaskToolInfo {
                name: "paradigm_shift_query".into(),
                risk_band: RiskBand::Medium,
            },
            KaskToolInfo {
                name: "russell_host_snapshot".into(),
                risk_band: RiskBand::None,
            },
        ]
    }

    #[test]
    fn resolves_kask_tool() {
        let skills = [make_skill()];
        let kask_tools = make_kask_tools();
        let result = resolve_with_kask("ACTION: kask/paradigm_shift_query", &skills, &kask_tools)
            .unwrap()
            .unwrap();
        assert!(result.is_kask_tool());
        assert_eq!(result.skill_id(), "kask");
        assert_eq!(result.action_id(), "paradigm_shift_query");
    }

    #[test]
    fn unknown_kask_tool_is_error() {
        let skills = [make_skill()];
        let kask_tools = make_kask_tools();
        let err = resolve_with_kask("ACTION: kask/nonexistent", &skills, &kask_tools)
            .unwrap()
            .unwrap_err();
        assert!(err.to_string().contains("nonexistent"));
    }

    #[test]
    fn kask_prefix_without_tools_is_unknown_skill() {
        let skills = [make_skill()];
        // No kask tools available — "kask" is not a loaded skill.
        let err = resolve_with_kask("ACTION: kask/anything", &skills, &[])
            .unwrap()
            .unwrap_err();
        assert!(err.to_string().contains("kask"));
    }

    #[test]
    fn local_skill_still_resolves_with_kask_tools_present() {
        let skills = [make_skill()];
        let kask_tools = make_kask_tools();
        let result = resolve_with_kask("ACTION: test-skill/probe-1", &skills, &kask_tools)
            .unwrap()
            .unwrap();
        assert!(result.is_probe());
        assert_eq!(result.skill_id(), "test-skill");
    }
}

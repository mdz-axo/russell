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
/// (mutating, requires consent per JR-2), or a hKask MCP tool call
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
    /// hKask MCP tool call (ADR-0025). Executed via the MCP client,
    /// not the local skill dispatcher.
    HKaskTool {
        /// The MCP tool name (from `tools/list`).
        tool_name: String,
        /// Risk band from tool annotations. Defaults to `Medium` when
        /// unset — safe default requiring operator consent.
        risk_band: RiskBand,
        /// Arguments for the tool call, parsed from the LLM response.
        /// `None` if the LLM did not provide any.
        arguments: Option<serde_json::Value>,
        /// Expected arguments (required fields from inputSchema).
        required_fields: Vec<String>,
    },
}

impl ResolvedAction {
    /// Returns `true` if this is a probe (read-only, auto-execute).
    #[must_use]
    pub fn is_probe(&self) -> bool {
        matches!(self, Self::Probe { .. })
    }

    /// Returns `true` if this is a hKask MCP tool call.
    #[must_use]
    pub fn is_hkask_tool(&self) -> bool {
        matches!(self, Self::HKaskTool { .. })
    }

    /// Returns the risk band for this action.
    #[must_use]
    pub fn risk_band(&self) -> RiskBand {
        match self {
            Self::Probe { .. } => RiskBand::None,
            Self::Intervention { risk, .. } => *risk,
            Self::HKaskTool { risk_band, .. } => *risk_band,
        }
    }

    /// The skill ID, regardless of action type.
    #[must_use]
    pub fn skill_id(&self) -> &str {
        match self {
            Self::Probe { skill_id, .. } => skill_id,
            Self::Intervention { skill_id, .. } => skill_id,
            Self::HKaskTool { .. } => "hkask",
        }
    }

    /// The action (probe or intervention) ID.
    #[must_use]
    pub fn action_id(&self) -> &str {
        match self {
            Self::Probe { action_id, .. } => action_id,
            Self::Intervention { action_id, .. } => action_id,
            Self::HKaskTool { tool_name, .. } => tool_name,
        }
    }

    /// The command argv. Empty for hKask tools (they're MCP calls, not subprocesses).
    #[must_use]
    pub fn cmd(&self) -> &[String] {
        match self {
            Self::Probe { cmd, .. } => cmd,
            Self::Intervention { cmd, .. } => cmd,
            Self::HKaskTool { .. } => &[],
        }
    }

    /// Whether this action requires operator consent.
    ///
    /// Probes never require consent (risk: none).
    /// Interventions require consent when their risk exceeds the skill's `max_auto_risk`.
    /// hKask tools require consent when their risk > None.
    #[must_use]
    pub fn consent_required(&self) -> bool {
        match self {
            Self::Probe { .. } => false,
            Self::Intervention {
                risk,
                max_auto_risk,
                ..
            } => *risk > *max_auto_risk,
            Self::HKaskTool { risk_band, .. } => *risk_band > RiskBand::None,
        }
    }

    /// Append extra CLI arguments to the command argv.
    /// Only Applies to probes and interventions; no-op for hKask tools.
    pub fn append_cmd_args(&mut self, args: &[String]) {
        match self {
            Self::Probe { cmd, .. } | Self::Intervention { cmd, .. } => {
                cmd.extend(args.iter().cloned());
            }
            Self::HKaskTool { .. } => {}
        }
    }
}

/// Error returned when an ACTION: line cannot be resolved.
#[derive(Debug, Clone)]
pub enum ActionError {
    /// The ACTION: prefix was malformed.
    MalformedPrefix {
        /// The raw line that was not found.
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
    /// Nested ACTION: detected in LLM output (prompt injection attempt).
    /// Task 3.4: Security hardening against LLM action injection.
    NestedActionDetected {
        /// The raw response containing nested ACTION: patterns.
        raw_response: String,
        /// Count of ACTION: occurrences found.
        count: usize,
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
            Self::NestedActionDetected {
                raw_response,
                count,
            } => {
                write!(
                    f,
                    "Security violation: detected {count} ACTION: patterns in LLM output (prompt injection attempt). Only one ACTION: per response is allowed. Raw response: `{raw_response}`"
                )
            }
        }
    }
}

/// Metadata for a hKask tool available in the registry, passed by
/// the caller (keeps `russell-meta` free of `russell-mcp` dependency).
#[derive(Debug, Clone)]
pub struct HKaskToolInfo {
    /// Tool name (the callable ID).
    pub name: String,
    /// Risk band from annotations. Defaults to `RiskBand::Medium`
    /// when unset — safe default per IDRS. Probes should explicitly
    /// declare `RiskBand::None`.
    pub risk_band: RiskBand,
    /// JSON Schema for the tool's input parameters (from `tools/list`).
    /// Used to extract required field names for operator prompting.
    pub input_schema: Option<serde_json::Value>,
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
    resolve_with_hkask(response, skills, &[])
}

/// Parse the last `ACTION:` line from a response and resolve it
/// against both the loaded skill set AND the hKask MCP tool registry.
///
/// When `skill_id == "hkask"`, the action_id is validated against
/// `hkask_tools` (the poka-yoke for MCP tools per ADR-0025 §7).
///
/// Returns `None` if no `ACTION:` line is present in the response.
///
/// # Task 3.4: Nested ACTION: Detection
///
/// This function now detects prompt injection attempts where the LLM
/// output contains multiple `ACTION:` patterns. If more than one
/// `ACTION:` line is found, returns `ActionError::NestedActionDetected`.
/// This is a security measure per the Schneier/Miller security lens
/// from the adversarial review.
pub fn resolve_with_hkask(
    response: &str,
    skills: &[Skill],
    hkask_tools: &[HKaskToolInfo],
) -> Option<std::result::Result<ResolvedAction, ActionError>> {
    // Task 3.4: Detect nested ACTION: patterns (prompt injection attempt).
    let action_count = response
        .lines()
        .filter(|line| line.trim().starts_with("ACTION:"))
        .count();
    if action_count > 1 {
        return Some(Err(ActionError::NestedActionDetected {
            raw_response: response.to_string(),
            count: action_count,
        }));
    }

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

    // hKask MCP tool path (ADR-0025 §7).
    // Only route to hKask if tools are available (registry is populated).
    if skill_id == "hkask" && !hkask_tools.is_empty() {
        // Strip inline arguments from the action_id (e.g. "tool --arg val" → "tool").
        let bare_tool_name = action_id.split(' ').next().unwrap_or(action_id);
        return resolve_hkask_tool(bare_tool_name, hkask_tools, skills, response);
    }

    let skill = match skills.iter().find(|s| s.id == skill_id) {
        Some(s) => s,
        None => {
            // Include "hkask" in the loaded list if we have hKask tools.
            let mut loaded: Vec<String> = skills.iter().map(|s| s.id.clone()).collect();
            if !hkask_tools.is_empty() {
                loaded.push("hkask".to_string());
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

/// Resolve a hKask MCP tool reference against the cached tool registry.
fn resolve_hkask_tool(
    tool_name: &str,
    hkask_tools: &[HKaskToolInfo],
    skills: &[Skill],
    response: &str,
) -> Option<std::result::Result<ResolvedAction, ActionError>> {
    // Poka-yoke: tool must exist in the registry.
    if let Some(tool) = hkask_tools.iter().find(|t| t.name == tool_name) {
        // Extract required fields from the tool's inputSchema.
        let required_fields = extract_required_fields(&tool.input_schema);

        // Extract arguments from the LLM response body.
        let arguments = extract_arguments_from_response(response, tool_name);

        return Some(Ok(ResolvedAction::HKaskTool {
            tool_name: tool.name.clone(),
            risk_band: tool.risk_band,
            arguments,
            required_fields,
        }));
    }

    // Tool not found — build a diagnostic error.
    let available: Vec<String> = hkask_tools.iter().map(|t| t.name.clone()).collect();
    let mut loaded: Vec<String> = skills.iter().map(|s| s.id.clone()).collect();
    if !hkask_tools.is_empty() {
        loaded.push("hkask".to_string());
    }

    Some(Err(ActionError::UnknownAction {
        skill_id: "hkask".to_string(),
        action_id: tool_name.to_string(),
        probes: available,
        interventions: vec![],
    }))
}

/// Extract required field names from a tool's JSON Schema `input_schema`.
///
/// Returns an empty vec if the schema is `None` or has no `required` array.
fn extract_required_fields(schema: &Option<serde_json::Value>) -> Vec<String> {
    schema
        .as_ref()
        .and_then(|s| s.get("required"))
        .and_then(|r| r.as_array())
        .map(|fields| {
            fields
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

/// Parse tool arguments from the LLM response body.
///
/// Supports two formats:
/// 1. `Arguments: {"key": "value"}` line anywhere in the response
/// 2. An inline JSON object at the end of the ACTION line:
///    `ACTION: hkask/tool --prompt "text" --depth 3`
///
/// The `_tool_name` parameter is unused but kept in the signature
/// for potential future use (e.g., validating args against the
/// tool's input schema). Current implementation re-parses the tool
/// name from the ACTION line directly.
fn extract_arguments_from_response(response: &str, _tool_name: &str) -> Option<serde_json::Value> {
    // Format 1: Look for "Arguments:" line with JSON payload.
    if let Some(line) = response
        .lines()
        .find(|l| l.trim().starts_with("Arguments:"))
        && let Some(json_str) = line
            .trim()
            .strip_prefix("Arguments:")
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
        && let Ok(value) = serde_json::from_str::<serde_json::Value>(json_str)
    {
        return Some(value);
    }

    // Format 2: Key=value pairs on the ACTION line after the tool name.
    // Look for the ACTION line and parse trailing key=value pairs.
    if let Some(action_line) = response
        .lines()
        .rev()
        .find(|line| line.trim().starts_with("ACTION:"))
    {
        let after_prefix = action_line
            .trim()
            .strip_prefix("ACTION:")
            .unwrap_or("")
            .trim();

        // After "hkask/tool-name", look for --key value pairs or key=value.
        // Find the tool name (skip "hkask/" prefix and tool name).
        // Use the part after the first '/' that follows "hkask".
        if let Some(rest) = after_prefix.strip_prefix("hkask/") {
            // Split out the tool name.
            if let Some(first_space) = rest.find(' ') {
                let args_str = rest[first_space..].trim();
                if let Some(args) = parse_key_value_args(args_str) {
                    return Some(args);
                }
            }
        }
    }

    None
}

/// Parse `--key value` or `key=value` pairs into a JSON object.
/// Handles quoted values with internal spaces.
fn parse_key_value_args(args_str: &str) -> Option<serde_json::Value> {
    let mut map = serde_json::Map::new();
    let tokens = tokenize_args(args_str);
    let mut i = 0;

    while i < tokens.len() {
        let token = &tokens[i];

        // Handle `--key` format.
        if let Some(key) = token.strip_prefix("--") {
            if key.is_empty() {
                i += 1;
                continue;
            }
            if i + 1 < tokens.len() {
                let value = parse_arg_value(&tokens[i + 1]);
                map.insert(key.to_string(), value);
                i += 2;
            } else {
                map.insert(key.to_string(), serde_json::Value::Bool(true));
                i += 1;
            }
            continue;
        }

        // Handle `key=value` format.
        if let Some((key, value)) = token.split_once('=') {
            let value = parse_arg_value(value);
            map.insert(key.to_string(), value);
            i += 1;
            continue;
        }

        i += 1;
    }

    if map.is_empty() {
        None
    } else {
        Some(serde_json::Value::Object(map))
    }
}

/// Tokenize argument string, preserving quoted segments.
fn tokenize_args(s: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        // Skip whitespace.
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        if i >= chars.len() {
            break;
        }

        // Quoted string.
        if chars[i] == '"' {
            i += 1; // skip opening quote
            let mut s = String::new();
            while i < chars.len() && chars[i] != '"' {
                s.push(chars[i]);
                i += 1;
            }
            if i < chars.len() {
                i += 1; // skip closing quote
            }
            tokens.push(s);
        } else {
            let mut s = String::new();
            while i < chars.len() && !chars[i].is_whitespace() {
                s.push(chars[i]);
                i += 1;
            }
            tokens.push(s);
        }
    }

    tokens
}

/// Parse a single argument value: try JSON, then number, then string.
fn parse_arg_value(s: &str) -> serde_json::Value {
    // Try JSON literal first (true, false, null, numbers, quoted strings).
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(s)
        && !v.is_string()
    {
        return v;
    }
    // Try number.
    if let Ok(n) = s.parse::<i64>() {
        return serde_json::json!(n);
    }
    if let Ok(n) = s.parse::<f64>() {
        return serde_json::json!(n);
    }
    serde_json::Value::String(s.to_string())
}

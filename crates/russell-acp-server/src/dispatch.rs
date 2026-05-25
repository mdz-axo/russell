// SPDX-License-Identifier: MIT OR Apache-2.0
//! Skill dispatch with visibility enforcement.

use std::path::PathBuf;
use std::time::Duration;

use crate::error::{AcpError, Result};
use crate::port::SkillDispatchPort;
use crate::types::{
    InterventionInfo, LexiconCategorization, LexiconDomain, ProbeInfo, SafetyInfo, SkillInfo,
    Visibility,
};
use russell_core::journal::JournalWriter;
use russell_skills::dispatch::{Dispatcher, StepType};
use russell_skills::{Skill, Visibility as SkillVisibility};

/// ACP dispatch — wraps russell-skills with visibility filtering.
pub struct AcpDispatch {
    /// Russell skills.
    skills: Vec<Skill>,
    /// Skills base directory.
    skills_dir: PathBuf,
    /// Journal for evidence logging.
    journal: Option<std::sync::Arc<JournalWriter>>,
    /// Evidence base directory.
    evidence_base: PathBuf,
    /// Dispatcher pool — cached dispatchers keyed by skill ID.
    dispatcher_pool: dashmap::DashMap<String, std::sync::Arc<Dispatcher>>,
}

impl AcpDispatch {
    /// Create a new ACP dispatch from loaded skills.
    pub fn new(skills: Vec<Skill>, skills_dir: PathBuf) -> Self {
        let evidence_base = skills_dir.join("evidence");
        Self {
            skills,
            skills_dir,
            journal: None,
            evidence_base,
            dispatcher_pool: dashmap::DashMap::new(),
        }
    }

    /// Get or create a dispatcher for a skill (pooled, lock-free reads).
    fn get_dispatcher(&self, skill_id: &str) -> std::sync::Arc<Dispatcher> {
        if let Some(dispatcher) = self.dispatcher_pool.get(skill_id) {
            return std::sync::Arc::clone(dispatcher.value());
        }
        let skill_dir = self.skills_dir.join(skill_id);
        let dispatcher = std::sync::Arc::new(Dispatcher::new(&skill_dir));
        self.dispatcher_pool
            .insert(skill_id.to_string(), std::sync::Arc::clone(&dispatcher));
        dispatcher
    }

    /// Set the journal writer for evidence logging.
    pub fn with_journal(mut self, journal: std::sync::Arc<JournalWriter>) -> Self {
        self.journal = Some(journal);
        self
    }

    /// Load only public skills (for ACP exposure).
    pub fn load_public_skills(&self) -> Vec<SkillInfo> {
        self.skills
            .iter()
            .filter(|s| is_public(s))
            .map(skill_to_info)
            .collect()
    }

    /// Dispatch a skill by ID (enforces visibility).
    pub async fn dispatch_skill(&self, skill_id: &str, args: &serde_json::Value) -> Result<String> {
        let skill = self
            .skills
            .iter()
            .find(|s| s.id == skill_id)
            .ok_or_else(|| AcpError::SkillNotFound(skill_id.to_string()))?;

        // Enforce visibility boundary.
        if !is_public(skill) {
            return Err(AcpError::SkillNotExposed(skill_id.to_string()));
        }

        // Extract probe_id or intervention_id from args.
        let step_id = args.get("step_id").and_then(|v| v.as_str()).unwrap_or("");

        if step_id.is_empty() {
            return Err(AcpError::InvalidRequest("step_id required".to_string()));
        }

        // Find the step in the skill.
        let step = skill
            .find_step(step_id)
            .ok_or_else(|| AcpError::ProbeNotFound(step_id.to_string()))?;

        // Probes auto-execute; interventions require consent (handled upstream).
        if step.is_intervention() {
            return Err(AcpError::InvalidRequest(
                "interventions require consent — use acp/probe/run for probes only".to_string(),
            ));
        }

        // Execute the probe using pooled dispatcher.
        let dispatcher = self.get_dispatcher(skill_id);

        let outcome = if let Some(ref journal) = self.journal {
            dispatcher
                .run_and_journal(
                    journal,
                    &self.evidence_base,
                    &step.cmd,
                    skill_id,
                    step_id,
                    StepType::Probe,
                    "none",
                    Some(Duration::from_secs(30)),
                )
                .await
        } else {
            dispatcher
                .run(&step.cmd, Some(Duration::from_secs(30)))
                .await
        }
        .map_err(|e| AcpError::DispatchError(format!("probe execution failed: {}", e)))?;

        Ok(format!(
            "Probe {} completed: exit={:?}, stdout={}, stderr={}",
            step_id,
            outcome.exit_code,
            outcome.stdout.trim(),
            outcome.stderr.trim()
        ))
    }

    /// Run a probe (read-only, always allowed if skill is public).
    pub async fn run_probe(
        &self,
        skill_id: &str,
        probe_id: &str,
        _args: &serde_json::Value,
    ) -> Result<String> {
        let skill = self
            .skills
            .iter()
            .find(|s| s.id == skill_id)
            .ok_or_else(|| AcpError::SkillNotFound(skill_id.to_string()))?;

        if !is_public(skill) {
            return Err(AcpError::SkillNotExposed(skill_id.to_string()));
        }

        // Find the probe.
        let probe = skill
            .probes
            .iter()
            .find(|p| p.id == probe_id)
            .ok_or_else(|| AcpError::ProbeNotFound(probe_id.to_string()))?;

        // Execute the probe using pooled dispatcher.
        let dispatcher = self.get_dispatcher(skill_id);

        let outcome = if let Some(ref journal) = self.journal {
            dispatcher
                .run_and_journal(
                    journal,
                    &self.evidence_base,
                    &probe.cmd,
                    skill_id,
                    probe_id,
                    StepType::Probe,
                    "none",
                    Some(Duration::from_secs(30)),
                )
                .await
        } else {
            dispatcher
                .run(&probe.cmd, Some(Duration::from_secs(30)))
                .await
        }
        .map_err(|e| AcpError::DispatchError(format!("probe execution failed: {}", e)))?;

        Ok(format!(
            "Probe {}/{} completed: exit={:?}, stdout={}, stderr={}",
            skill_id,
            probe_id,
            outcome.exit_code,
            outcome.stdout.trim(),
            outcome.stderr.trim()
        ))
    }

    /// Get skill info by ID.
    pub fn get_skill_info(&self, skill_id: &str) -> Option<SkillInfo> {
        let skill = self.skills.iter().find(|s| s.id == skill_id)?;
        if !is_public(skill) {
            return None;
        }
        Some(skill_to_info(skill))
    }

    /// List all public probes.
    pub fn list_probes(&self) -> Vec<ProbeInfo> {
        self.skills
            .iter()
            .filter(|s| is_public(s))
            .flat_map(|s| {
                s.probes.iter().map(|p| ProbeInfo {
                    id: p.id.clone(),
                    description: format!("Probe: {}", p.id),
                    timeout: format_duration(p.timeout.clone()),
                })
            })
            .collect()
    }
}

impl Default for AcpDispatch {
    fn default() -> Self {
        Self::new(Vec::new(), PathBuf::from("/tmp"))
    }
}

/// Check if a skill is public.
fn is_public(skill: &Skill) -> bool {
    skill.visibility == SkillVisibility::Public
}

/// Convert russell_skills::Skill to ACP SkillInfo.
fn skill_to_info(skill: &Skill) -> SkillInfo {
    SkillInfo {
        id: skill.id.clone(),
        version: skill.version.clone(),
        description: format!("Skill: {}", skill.id),
        visibility: Visibility::Public,
        lexicon: skill
            .lexicon
            .as_ref()
            .map(|l| LexiconCategorization {
                primary: match l.primary.as_str() {
                    "WordAct" => LexiconDomain::WordAct,
                    "FlowDef" => LexiconDomain::FlowDef,
                    "KnowAct" => LexiconDomain::KnowAct,
                    _ => LexiconDomain::KnowAct,
                },
                terms: l.terms.clone(),
            })
            .unwrap_or_else(|| LexiconCategorization {
                primary: LexiconDomain::KnowAct,
                terms: Vec::new(),
            }),
        symptoms: skill.symptoms.iter().map(|s| s.name().to_string()).collect(),
        probes: skill
            .probes
            .iter()
            .map(|p| ProbeInfo {
                id: p.id.clone(),
                description: format!("Probe: {}", p.id),
                timeout: format_duration(p.timeout.clone()),
            })
            .collect(),
        interventions: skill
            .interventions
            .iter()
            .map(|i| InterventionInfo {
                id: i.id.clone(),
                description: format!("Intervention: {}", i.id),
                risk: i.risk,
                needs_sudo: i.needs_sudo,
                rollback: Some(format!("{:?}", i.rollback)),
            })
            .collect(),
        safety: SafetyInfo {
            max_auto_risk: skill.safety.max_auto_risk,
            require_human_for: skill.safety.require_human_for.clone(),
        },
    }
}

/// Format duration as string.
fn format_duration(d: String) -> String {
    d
}

#[async_trait::async_trait(?Send)]
impl SkillDispatchPort for AcpDispatch {
    fn load_public_skills(&self) -> Vec<SkillInfo> {
        self.load_public_skills()
    }

    fn get_skill_info(&self, skill_id: &str) -> Option<SkillInfo> {
        self.get_skill_info(skill_id)
    }

    fn list_probes(&self) -> Vec<ProbeInfo> {
        self.list_probes()
    }

    async fn dispatch_skill(&self, skill_id: &str, args: &serde_json::Value) -> Result<String> {
        self.dispatch_skill(skill_id, args).await
    }

    async fn run_probe(
        &self,
        skill_id: &str,
        probe_id: &str,
        args: &serde_json::Value,
    ) -> Result<String> {
        self.run_probe(skill_id, probe_id, args).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::port::SkillDispatchPort;
    use crate::types::ProbeInfo;
    use serde_json::json;

    struct MockSkillDispatch {
        skills: Vec<SkillInfo>,
        probes: Vec<ProbeInfo>,
    }

    #[async_trait::async_trait(?Send)]
    impl SkillDispatchPort for MockSkillDispatch {
        fn load_public_skills(&self) -> Vec<SkillInfo> {
            self.skills.clone()
        }

        fn get_skill_info(&self, skill_id: &str) -> Option<SkillInfo> {
            self.skills.iter().find(|s| s.id == skill_id).cloned()
        }

        fn list_probes(&self) -> Vec<ProbeInfo> {
            self.probes.clone()
        }

        async fn dispatch_skill(
            &self,
            skill_id: &str,
            _args: &serde_json::Value,
        ) -> Result<String> {
            Ok(format!("mock dispatch: {}", skill_id))
        }

        async fn run_probe(
            &self,
            skill_id: &str,
            probe_id: &str,
            _args: &serde_json::Value,
        ) -> Result<String> {
            Ok(format!("mock probe: {}/{}", skill_id, probe_id))
        }
    }

    #[tokio::test]
    async fn mock_skill_dispatch_implements_port() {
        let mock = MockSkillDispatch {
            skills: Vec::new(),
            probes: Vec::new(),
        };
        let port: Box<dyn SkillDispatchPort> = Box::new(mock);
        assert!(port.load_public_skills().is_empty());
        assert!(port.list_probes().is_empty());
        assert_eq!(
            port.dispatch_skill("test", &json!({})).await.unwrap(),
            "mock dispatch: test"
        );
        assert_eq!(
            port.run_probe("s", "p", &json!({})).await.unwrap(),
            "mock probe: s/p"
        );
    }
}

// SPDX-License-Identifier: MIT OR Apache-2.0
//! Unified prompt assembly — templated path only.
//!
//! The legacy `compose_with_kask()` and `compose()` functions have been
//! removed. All prompt assembly now flows through `compose_templated()`,
//! which uses the MiniJinja `PromptRegistry` with relevance-scored
//! knowledge injection and inference hints from `.md.j2` templates.
//!
//! ## Skill → Prompt mapping registry
//!
//! Template selection is governed by `prompt-templates.yaml` (compiled-in,
//! operator-overridable at `~/.config/harness/prompts/templates.yaml`).
//! This replaces the ad-hoc logic previously at prompt.rs:563-623.

use crate::client::SoapPrompt;
use crate::prompt_registry::{
    KnowledgeSlot, PromptRegistry, SkillTelemetry,
    score_knowledge_relevance, score_knowledge_relevance_with_telemetry,
    select_knowledge_knapsack,
};
use russell_core::journal::JournalReader;
use russell_core::profile::Profile;
use russell_skills::Skill;
use std::collections::HashMap;
use std::path::Path;

/// Knowledge injection budget in tokens (~3000 tokens ≈ 12000 chars).
const KNOWLEDGE_BUDGET_TOKENS: usize = 3000;

/// Build the SOAP prompt using the MiniJinja template registry.
///
/// This is the **sole prompt assembly path**. The legacy `compose_with_kask()`
/// was removed in the prompt unification refactor (see
/// `docs/architecture/skill-friction-analysis.md` F2).
///
/// # Template selection
///
/// The `registry` parameter determines which template is rendered:
/// - For `russell jack`: the `soap` template (one-shot SOAP)
/// - For `russell chat`: the `chat_objective` template (multi-turn context)
/// - For `russell workshop`: the `workshop` template (now merged into chat,
///   see F6)
///
/// Templates are specified by the caller based on `PromptMode::template_name()`.
///
/// # Knowledge injection
///
/// Uses a token-aware knapsack solver (`select_knowledge_knapsack`) that:
/// 1. Scores each skill's KNOWLEDGE.md relevance to active symptoms
/// 2. Weights scores by `SkillHealth.reliability` from `skill_registry`
/// 3. Maximizes symptom coverage within the 3000-token budget
///
/// This replaces the greedy `select_knowledge()` at `prompt_registry.rs:421`.
pub fn compose_templated(
    registry: &PromptRegistry,
    template_name: &str,
    reader: &JournalReader,
    profile: Option<&Profile>,
    note: Option<&str>,
    loaded_skills: &[Skill],
    skills_base_dir: &Path,
    kask_tool_names: &[(String, Option<String>)],
    skill_registry: Option<&russell_skills::registry::RegistryCache>,
) -> Result<SoapPrompt, crate::error::MetaError> {
    let now = russell_core::time::now_unix();
    let window_start = now - 24 * 3600;

    // ── Context blocks (shared block builders) ────────────────────────
    let subjective = match note {
        Some(n) if !n.trim().is_empty() => n.trim().to_string(),
        _ => "(no operator note)".to_string(),
    };

    let profile_block = build_profile_block(profile);
    let severity_block = build_severity_block(reader, window_start)?;
    let samples_table = build_samples_table(reader, window_start)?;
    let freshness_block = build_freshness_block(reader);
    let events_table = build_events_table(reader)?;
    let reflex_section = build_reflex_block(reader)?;

    // ── Skill context ─────────────────────────────────────────────────
    let actionable: Vec<serde_json::Value> = loaded_skills
        .iter()
        .filter(|s| s.is_actionable())
        .map(|s| {
            serde_json::json!({
                "id": s.id,
                "kind": s.kind.as_str(),
                "probes": s.probes.iter().map(|p| serde_json::json!({
                    "id": p.id,
                })).collect::<Vec<_>>(),
                "interventions": s.interventions.iter().map(|iv| serde_json::json!({
                    "id": iv.id,
                    "risk": iv.risk.as_str(),
                })).collect::<Vec<_>>(),
            })
        })
        .collect();

    // Knowledge skills: include health summary if registry is available.
    let knowledge: Vec<serde_json::Value> = loaded_skills
        .iter()
        .filter(|s| s.is_lens() || s.has_knowledge())
        .map(|s| {
            let mut entry = serde_json::json!({
                "id": s.id,
                "symptoms": s.symptoms.join(", "),
                "kind": s.kind.as_str(),
            });
            if let Some(reg) = skill_registry {
                if let Some(re) = reg.skills.get(&s.id) {
                    let freshness = russell_skills::registry::freshness_score(re);
                    entry["health"] = serde_json::json!({
                        "freshness": freshness,
                        "probe_runs": re.probe_runs,
                        "recent_failures": re.recent_probe_failures,
                    });
                }
            }
            entry
        })
        .collect();

    let kask_tools: Vec<serde_json::Value> = kask_tool_names
        .iter()
        .map(|(name, risk)| {
            serde_json::json!({
                "name": name,
                "risk": risk.as_deref().unwrap_or("medium"),
            })
        })
        .collect();

    // ── Render template ───────────────────────────────────────────────
    let mut ctx = HashMap::new();
    ctx.insert("subjective".to_string(), serde_json::json!(subjective));
    ctx.insert("profile_block".to_string(), serde_json::json!(profile_block));
    ctx.insert("severity_block".to_string(), serde_json::json!(severity_block));
    ctx.insert("samples_table".to_string(), serde_json::json!(samples_table));
    ctx.insert("freshness_block".to_string(), serde_json::json!(freshness_block));
    ctx.insert("events_table".to_string(), serde_json::json!(events_table));
    if !reflex_section.is_empty() {
        ctx.insert("reflex_section".to_string(), serde_json::json!(reflex_section));
    }
    if !actionable.is_empty() {
        ctx.insert("actionable_skills".to_string(), serde_json::json!(actionable));
    }
    if !knowledge.is_empty() {
        ctx.insert("knowledge_skills".to_string(), serde_json::json!(knowledge));
    }
    if !kask_tools.is_empty() {
        ctx.insert("kask_tools".to_string(), serde_json::json!(kask_tools));
    }

    let (rendered, hint) = registry
        .render_with_hint(template_name, &ctx)
        .map_err(|e| crate::error::MetaError::Prompt(format!("template render failed: {e}")))?;

    // ── System prompt: persona + relevance-scored knowledge ──────────
    let mut system_prompt = crate::JACK_PERSONA.to_string();
    append_skill_knowledge_scored(
        &mut system_prompt,
        loaded_skills,
        skills_base_dir,
        reader,
        skill_registry,
    );

    // ── Extract inference parameters from template hint ───────────────
    let temperature = hint.as_ref().and_then(|h| h.temperature);
    let max_tokens = hint.as_ref().and_then(|h| h.max_tokens);

    Ok(SoapPrompt {
        system: system_prompt,
        subjective,
        objective: String::new(),
        rendered,
        temperature,
        max_tokens,
    })
}

// ─── Knowledge injection (relevance-scored with knapsack solver) ────────────

/// Append KNOWLEDGE.md content with relevance scoring, reliability weighting,
/// and knapsack token budgeting.
///
/// # Budget optimization
///
/// Uses `select_knowledge_knapsack()` instead of the greedy `select_knowledge()`.
/// The knapsack solver maximizes symptom overlap per token, using
/// `SkillHealth.reliability` from the registry as a quality weight.
fn append_skill_knowledge_scored(
    system: &mut String,
    skills: &[Skill],
    skills_base_dir: &Path,
    reader: &JournalReader,
    skill_registry: Option<&russell_skills::registry::RegistryCache>,
) {
    // Determine active symptoms from recent warn/alert/crit events.
    let active_symptoms: Vec<String> = reader
        .recent(20)
        .unwrap_or_default()
        .iter()
        .filter(|r| {
            let s = r.severity.as_str();
            s == "warn" || s == "alert" || s == "crit"
        })
        .filter_map(|r| r.module.as_ref())
        .map(|m| m.to_string())
        .collect();

    let mut slots: Vec<KnowledgeSlot> = Vec::new();
    for skill in skills {
        let applies = skill.applies_when.iter().any(|clause| {
            matches!(
                clause,
                russell_skills::AppliesWhen::Scalar {
                    os_family: Some(os),
                    ..
                } if os == "linux"
            )
        });
        if !applies && !skill.applies_when.is_empty() {
            continue;
        }
        let knowledge_path = skills_base_dir.join(&skill.id).join("KNOWLEDGE.md");
        if let Ok(content) = std::fs::read_to_string(&knowledge_path) {
            if content.trim().is_empty() {
                continue;
            }

            // Score with telemetry feedback if registry is available.
            let relevance = match skill_registry.and_then(|reg| reg.skills.get(&skill.id)) {
                Some(entry) => {
                    let telemetry = SkillTelemetry {
                        freshness: russell_skills::registry::freshness_score(entry),
                        probe_runs: entry.probe_runs,
                        recent_failures: entry.recent_probe_failures,
                        intervention_runs: entry.intervention_runs,
                        recent_intervention_failures: entry.recent_intervention_failures,
                    };
                    score_knowledge_relevance_with_telemetry(
                        &skill.symptoms,
                        &active_symptoms,
                        &telemetry,
                    )
                }
                None => score_knowledge_relevance(&skill.symptoms, &active_symptoms),
            };

            let token_estimate = content.len() / 4;
            slots.push(KnowledgeSlot {
                skill_id: skill.id.clone(),
                content,
                relevance,
                token_estimate,
            });
        }
    }

    // Use knapsack solver instead of greedy selection.
    let selected = select_knowledge_knapsack(&mut slots, KNOWLEDGE_BUDGET_TOKENS);
    for slot in selected {
        system.push_str("\n\n---\n\n");
        system.push_str("# Knowledge: ");
        system.push_str(&slot.skill_id);
        system.push_str("\n\n");
        system.push_str(&slot.content);
        tracing::debug!(
            skill = %slot.skill_id,
            relevance = slot.relevance,
            tokens_est = slot.token_estimate,
            "appended knowledge (relevance-scored, knapsack-optimized)",
        );
    }
}

// ─── Block builders ─────────────────────────────────────────────────────────

fn build_profile_block(profile: Option<&Profile>) -> String {
    let mut block = String::new();
    match profile {
        Some(p) => {
            use std::fmt::Write;
            let _ = writeln!(block, "- profile_id: `{}`", p.profile_id);
            let _ = writeln!(block, "- authored_at: `{}`", p.authored_at);
            if !p.host.os.distro.is_empty() {
                let _ = writeln!(
                    block,
                    "- host.os: `{}/{}` kernel `{}`",
                    p.host.os.distro, p.host.os.version, p.host.os.kernel
                );
            }
            if !p.host.cpu.model.is_empty() {
                let _ = writeln!(
                    block,
                    "- host.cpu: `{}` ({} cores / {} threads)",
                    p.host.cpu.model, p.host.cpu.cores, p.host.cpu.threads
                );
            }
            if !p.gpus.is_empty() {
                let _ = writeln!(block, "- gpus:");
                for g in &p.gpus {
                    let _ = writeln!(
                        block,
                        "  - `{}` @ `{}` (role: {})",
                        g.name, g.pci, g.role
                    );
                }
            }
        }
        None => block.push_str("- (no profile.json)"),
    }
    block.trim_end().to_string()
}

fn build_severity_block(reader: &JournalReader, window_start: i64) -> Result<String, crate::error::MetaError> {
    use std::fmt::Write;
    let counts = reader
        .severity_counts(window_start, i64::MAX)
        .map_err(|e| crate::error::MetaError::Journal(format!("severity counts: {e}")))?;
    let mut block = String::new();
    let _ = writeln!(
        block,
        "- info: {} | warn: {} | alert: {} | crit: {}",
        counts.info, counts.warn, counts.alert, counts.crit
    );
    Ok(block)
}

fn build_samples_table(reader: &JournalReader, window_start: i64) -> Result<String, crate::error::MetaError> {
    use std::fmt::Write;
    let summaries = reader
        .host_samples_summary(window_start, i64::MAX)
        .unwrap_or_default();
    if summaries.is_empty() {
        return Ok("- (no samples recorded)".to_string());
    }
    let baselines: std::collections::BTreeMap<String, (Option<f64>, Option<f64>)> = reader
        .read_baselines()
        .unwrap_or_default()
        .into_iter()
        .map(|b| (b.probe, (b.p95, b.ewma_mean)))
        .collect();
    let has_baselines = !baselines.is_empty();

    let mut block = String::new();
    if has_baselines {
        let _ = writeln!(block, "| probe | count | min | avg | max | last | p95 (30d) | ewma (7d) | unit |");
        let _ = writeln!(block, "|---|---|---|---|---|---|---|---|---|");
    } else {
        let _ = writeln!(block, "| probe | count | min | avg | max | last | unit |");
        let _ = writeln!(block, "|---|---|---|---|---|---|---|");
    }
    for s in &summaries {
        let unit = s.unit.as_deref().unwrap_or("");
        let count = s.count;
        let min = s.min;
        let avg = s.avg;
        let max = s.max;
        let last = s.last;
        if has_baselines {
            let (p95, ewma) = baselines
                .get(&s.probe)
                .map(|(p, e)| (*p, *e))
                .unwrap_or((None, None));
            let p95_str = p95.map(|v| format!("{v:.2}")).unwrap_or_else(|| "—".into());
            let ewma_str = ewma.map(|v| format!("{v:.2}")).unwrap_or_else(|| "—".into());
            let _ = writeln!(
                block,
                "| {} | {count} | {min:.2} | {avg:.2} | {max:.2} | {last:.2} | {p95_str} | {ewma_str} | {unit} |",
                s.probe
            );
        } else {
            let _ = writeln!(
                block,
                "| {} | {count} | {min:.2} | {avg:.2} | {max:.2} | {last:.2} | {unit} |",
                s.probe
            );
        }
    }
    Ok(block.trim_end().to_string())
}

fn build_freshness_block(reader: &JournalReader) -> String {
    let age = last_sample_age(reader);
    match age {
        Some(a) => format!("- Last host sample {} seconds ago.", a),
        None => "- No host samples recorded yet.".to_string(),
    }
}

fn build_events_table(reader: &JournalReader) -> Result<String, crate::error::MetaError> {
    use std::fmt::Write;
    let rows = reader
        .recent(20)
        .map_err(|e| crate::error::MetaError::Journal(format!("recent events: {e}")))?;
    if rows.is_empty() {
        return Ok("- (no events recorded)".to_string());
    }
    let mut block = String::new();
    let _ = writeln!(block, "| ts | severity | scope | module | action | summary |");
    let _ = writeln!(block, "|---|---|---|---|---|---|");
    for r in rows {
        let _ = writeln!(
            block,
            "| {} | {} | {} | {} | {} | {} |",
            r.ts,
            r.severity.as_str(),
            r.scope.as_str(),
            r.module.as_deref().unwrap_or("-"),
            r.action,
            r.summary.as_deref().unwrap_or("")
        );
    }
    Ok(block.trim_end().to_string())
}

fn build_reflex_block(reader: &JournalReader) -> Result<String, crate::error::MetaError> {
    use std::fmt::Write;
    let now = russell_core::time::now_unix();
    let since = now - 7 * 86_400;
    let rows = reader.list_reflex_events(since, now).unwrap_or_default();
    if rows.is_empty() {
        return Ok(String::new());
    }
    let mut block = String::new();
    let _ = writeln!(block, "### Reflex arcs — proposed interventions");
    let _ = writeln!(block, "| ts | severity | intervention | summary |");
    let _ = writeln!(block, "|---|---|---|---|");
    for (sev, intervention, summary, ts) in &rows {
        let _ = writeln!(block, "| {ts} | {sev} | `{intervention}` | {summary} |");
    }
    let _ = writeln!(block, "- If any reflex arc above is within the risk cap, propose it via ACTION: <intervention>.");
    Ok(block.trim_end().to_string())
}

fn last_sample_age(reader: &JournalReader) -> Option<i64> {
    let ts = reader.last_host_sample_ts().ok().flatten()?;
    let now = russell_core::time::now_unix();
    Some(now - ts)
}

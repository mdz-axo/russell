// SPDX-License-Identifier: MIT OR Apache-2.0
//! SOAP prompt composition.
//!
//! Reads the last 24h of samples + last 20 events from the
//! journal and renders a Markdown-formatted SOAP bundle the LLM
//! can read directly.
//!
//! F-2 (Phase 2): includes a per-probe sample summary table
//! (min, avg, max, last, count) so Jack can reason about trends,
//! not just event counts.
//!
//! ## Template integration
//!
//! The `compose_with_kask_templated` function uses the prompt registry
//! to render templates from `.md.j2` files. The legacy `compose_with_kask`
//! function retains the original `writeln!()` approach for backward
//! compatibility while callers migrate.

use std::fmt::Write as _;
use std::path::Path;

use russell_core::Profile;
use russell_core::event::Scope;
use russell_core::journal::JournalReader;
use russell_skills::Skill;

use crate::client::SoapPrompt;
use crate::error::Result;
use crate::prompt_registry::{
    KnowledgeSlot, PromptRegistry, SkillTelemetry, score_knowledge_relevance,
    score_knowledge_relevance_with_telemetry, select_knowledge,
};

/// Build the SOAP prompt. The system prompt is always the
/// embedded Jack persona.
///
/// If `loaded_skills` is provided, Jack is also told what
/// probes and interventions are available so he can recommend
/// specific manifest IDs per ADR-0008 (LLM ranks IDs, does not
/// emit shell).
///
/// If `kask_tool_names` is provided (ADR-0025), Jack also knows
/// which Kask MCP tools are available via `ACTION: kask/<tool>`.
///
/// If any loaded skill has a `KNOWLEDGE.md` file and its
/// `applies_when` clause matches the machine profile, that
/// knowledge is appended to Jack's system prompt — giving him
/// domain expertise (Ubuntu, ROCm, etc.) without bloating
/// the base persona.
pub fn compose(
    reader: &JournalReader,
    profile: Option<&Profile>,
    note: Option<&str>,
    loaded_skills: &[Skill],
    skills_base_dir: &Path,
) -> Result<SoapPrompt> {
    compose_with_kask(reader, profile, note, loaded_skills, skills_base_dir, &[])
}

/// Build the SOAP prompt with Kask MCP tool awareness.
///
/// Same as [`compose`] but additionally includes available Kask tools
/// in the prompt's "Available actions" section.
pub fn compose_with_kask(
    reader: &JournalReader,
    profile: Option<&Profile>,
    note: Option<&str>,
    loaded_skills: &[Skill],
    skills_base_dir: &Path,
    kask_tool_names: &[(String, Option<String>)],
) -> Result<SoapPrompt> {
    let now = russell_core::time::now_unix();
    let window_start = now - 24 * 3600;

    let subjective = match note {
        Some(n) if !n.trim().is_empty() => n.trim().to_string(),
        _ => "(no operator note)".to_string(),
    };

    let mut objective = String::new();
    writeln!(objective, "### Profile")?;
    match profile {
        Some(p) => {
            writeln!(objective, "- profile_id: `{}`", p.profile_id)?;
            writeln!(objective, "- authored_at: `{}`", p.authored_at)?;
            if !p.host.os.distro.is_empty() {
                writeln!(
                    objective,
                    "- host.os: `{}/{}` kernel `{}`",
                    p.host.os.distro, p.host.os.version, p.host.os.kernel
                )?;
            }
            if !p.host.cpu.model.is_empty() {
                writeln!(
                    objective,
                    "- host.cpu: `{}` ({} cores / {} threads)",
                    p.host.cpu.model, p.host.cpu.cores, p.host.cpu.threads
                )?;
            }
            if !p.gpus.is_empty() {
                writeln!(objective, "- gpus:")?;
                for g in &p.gpus {
                    writeln!(
                        objective,
                        "  - `{}` @ `{}` (role: {})",
                        g.name, g.pci, g.role
                    )?;
                }
            }
        }
        None => writeln!(objective, "- (no profile.json)")?,
    }

    writeln!(objective, "\n### Severity counts — last 24h")?;
    let counts = reader.severity_counts(window_start, i64::MAX)?;
    writeln!(
        objective,
        "- info: {} | warn: {} | alert: {} | crit: {}",
        counts.info, counts.warn, counts.alert, counts.crit
    )?;

    // F-2: per-probe sample summary for the last 24h.
    // Gives Jack actual telemetry to reason about, not just event counts.
    writeln!(objective, "\n### Host probe samples — last 24h")?;
    let summaries = reader
        .host_samples_summary(window_start, i64::MAX)
        .unwrap_or_default();
    if summaries.is_empty() {
        writeln!(objective, "- (no samples recorded)")?;
    } else {
        // Read 30-day baselines for deviation detection.
        // Task 4.1: Track baseline staleness and warn if outdated.
        let baselines: std::collections::BTreeMap<String, russell_core::journal::BaselineRow> =
            reader
                .read_baselines()
                .unwrap_or_default()
                .into_iter()
                .map(|b| (b.probe.clone(), b))
                .collect();
        let has_baselines = !baselines.is_empty();

        // Check for stale baselines (older than 48 hours).
        let stale_baselines: Vec<&str> = baselines
            .values()
            .filter(|b| b.is_stale(48))
            .map(|b| b.probe.as_str())
            .collect();
        let any_stale = !stale_baselines.is_empty();

        if has_baselines {
            if any_stale {
                writeln!(
                    objective,
                    "\n⚠️ **Baseline staleness warning:** {} probes have baselines older than 48h: {}\n",
                    stale_baselines.len(),
                    stale_baselines.join(", ")
                )?;
            }
            writeln!(
                objective,
                "| probe | count | min | avg | max | last | p95 (30d) | ewma (7d) | unit |"
            )?;
            writeln!(objective, "|---|---|---|---|---|---|---|---|---|")?;
        } else {
            writeln!(
                objective,
                "| probe | count | min | avg | max | last | unit |"
            )?;
            writeln!(objective, "|---|---|---|---|---|---|---|")?;
        }
        for s in &summaries {
            let unit = s.unit.as_deref().unwrap_or("");
            if has_baselines {
                let baseline = baselines.get(&s.probe);
                let (p95, ewma) = baseline
                    .map(|b| (b.p95, b.ewma_mean))
                    .unwrap_or((None, None));
                let p95_str = p95.map(fmt_f64_baseline).unwrap_or_else(|| "—".to_string());
                let ewma_str = ewma
                    .map(fmt_f64_baseline)
                    .unwrap_or_else(|| "—".to_string());
                let stale_marker = if baseline.map(|b| b.is_stale(48)).unwrap_or(false) {
                    " ⚠️"
                } else {
                    ""
                };
                writeln!(
                    objective,
                    "| {}{} | {} | {} | {} | {} | {} | {} | {} | {} |",
                    s.probe,
                    stale_marker,
                    s.count,
                    fmt_opt_f64(s.min),
                    fmt_opt_f64(s.avg),
                    fmt_opt_f64(s.max),
                    fmt_opt_f64(s.last),
                    p95_str,
                    ewma_str,
                    unit,
                )?;
            } else {
                writeln!(
                    objective,
                    "| {} | {} | {} | {} | {} | {} | {} |",
                    s.probe,
                    s.count,
                    fmt_opt_f64(s.min),
                    fmt_opt_f64(s.avg),
                    fmt_opt_f64(s.max),
                    fmt_opt_f64(s.last),
                    unit,
                )?;
            }
        }
    }

    writeln!(objective, "\n### Sentinel freshness")?;
    let last_sample_age_s = last_sample_age(reader).unwrap_or(-1);
    if last_sample_age_s >= 0 {
        writeln!(
            objective,
            "- Last sample {} seconds ago.",
            last_sample_age_s
        )?;
    } else {
        writeln!(objective, "- No samples recorded.")?;
    }

    // Phase 3A: available skills for LLM recommendation.
    if !loaded_skills.is_empty() {
        // Separate actionable skills (have probes/interventions) from knowledge-only.
        let actionable: Vec<&Skill> = loaded_skills
            .iter()
            .filter(|s| !s.probes.is_empty() || !s.interventions.is_empty())
            .collect();
        let knowledge_only: Vec<&Skill> = loaded_skills
            .iter()
            .filter(|s| s.probes.is_empty() && s.interventions.is_empty())
            .collect();

        if !actionable.is_empty() {
            writeln!(objective, "\n### Available skills")?;
            writeln!(objective, "| skill | type | id | risk |")?;
            writeln!(objective, "|---|---|---|---|")?;
            for skill in &actionable {
                for p in &skill.probes {
                    writeln!(objective, "| {} | probe | {} | none |", skill.id, p.id,)?;
                }
                for iv in &skill.interventions {
                    writeln!(
                        objective,
                        "| {} | intervention | {} | {:?} |",
                        skill.id, iv.id, iv.risk,
                    )?;
                }
            }
            writeln!(
                objective,
                "\nWhen you identify a next step and a skill is loaded, \
                     propose it on the final line using ACTION syntax:\n\n\
                     For probes (read-only, auto-execute): \
                     ACTION: <skill-id>/<probe-id>\n\
                     (e.g. ACTION: okapi-watcher/probe-health)\n\n\
                     For interventions (mutations, require consent): \
                     ACTION: <skill-id>/<intervention-id>\n\
                     (e.g. ACTION: okapi-watcher/restart-okapi)\n\n\
                     Prefer probes first to gather evidence. \
                     Probes run immediately. Interventions wait for the \
                     operator to say 'ok'."
            )?;
        }

        if !knowledge_only.is_empty() {
            writeln!(objective, "\n### Loaded knowledge")?;
            writeln!(
                objective,
                "The following knowledge skills are active (their expertise is in your system prompt):"
            )?;
            for skill in &knowledge_only {
                let symptoms: String = skill
                    .symptoms
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                if symptoms.is_empty() {
                    writeln!(objective, "- **{}**", skill.id)?;
                } else {
                    writeln!(objective, "- **{}** — symptoms: {}", skill.id, symptoms)?;
                }
            }
        }
    }

    // Phase 4C: Kask MCP tools (ADR-0025 §7).
    if !kask_tool_names.is_empty() {
        writeln!(objective, "\n### Kask MCP tools")?;
        writeln!(objective, "| tool | risk |")?;
        writeln!(objective, "|---|---|")?;
        for (name, risk) in kask_tool_names {
            let risk_str = risk.as_deref().unwrap_or("medium");
            writeln!(objective, "| {name} | {risk_str} |")?;
        }
        writeln!(
            objective,
            "\nYou can also call Kask tools via ACTION syntax:\n\
             ACTION: kask/<tool-name>\n\
             (e.g. ACTION: kask/paradigm_shift_query)\n\n\
             If the tool needs arguments, add an Arguments line:\n\
             Arguments: {{\"prompt\": \"...\", \"depth\": \"quick\"}}\n\n\
             Tools with risk 'none' auto-execute. Others require operator consent."
        )?;
    }

    writeln!(objective, "\n### Most-recent events (up to 20)")?;
    let rows = reader.recent(20)?;
    if rows.is_empty() {
        writeln!(objective, "- (no events recorded)")?;
    } else {
        writeln!(
            objective,
            "| ts | severity | scope | module | action | summary |"
        )?;
        writeln!(objective, "|---|---|---|---|---|---|")?;
        for r in rows {
            writeln!(
                objective,
                "| {} | {} | {} | {} | {} | {} |",
                r.ts,
                r.severity.as_str(),
                r.scope.as_str(),
                r.module.as_deref().unwrap_or("-"),
                r.action,
                r.summary.as_deref().unwrap_or("")
            )?;
        }
    }

    // Reflex arcs: proposed interventions from the sentinel's reflex engine.
    build_reflex_section(reader, &mut objective)?;

    let mut rendered = String::new();
    writeln!(rendered, "# SOAP — russell help\n")?;
    writeln!(rendered, "## Subjective\n\n{subjective}\n")?;
    writeln!(rendered, "## Objective\n\n{objective}\n")?;
    writeln!(
        rendered,
        "## Assessment\n\n*(your job, Jack — fill this in based on the evidence above.)*\n"
    )?;
    writeln!(rendered, "## Plan\n\n*(your job, Jack — one next step.)*\n")?;

    let mut system_prompt = crate::JACK_PERSONA.to_string();

    // Append KNOWLEDGE.md from applicable skills.
    append_skill_knowledge(&mut system_prompt, loaded_skills, skills_base_dir);

    Ok(SoapPrompt {
        system: system_prompt,
        subjective,
        objective,
        rendered,
        temperature: None,
        max_tokens: None,
    })
}

/// Build the SOAP prompt using the MiniJinja template registry.
///
/// This is the modern path that uses `.md.j2` templates and
/// relevance-scored knowledge injection. It produces the same
/// output shape as [`compose_with_kask`] but is data-driven.
///
/// The `registry` should be created once at startup via
/// [`PromptRegistry::with_defaults()`] and optionally loaded
/// with disk overrides.
///
/// If `skill_registry` is provided, inter-session telemetry (execution
/// success/failure rates) is used to modulate knowledge relevance —
/// closing the feedback loop between past action outcomes and future
/// attention allocation.
pub fn compose_templated(
    registry: &PromptRegistry,
    reader: &JournalReader,
    profile: Option<&Profile>,
    note: Option<&str>,
    loaded_skills: &[Skill],
    skills_base_dir: &Path,
    kask_tool_names: &[(String, Option<String>)],
    skill_registry: Option<&russell_skills::registry::RegistryCache>,
) -> Result<SoapPrompt> {
    use std::collections::HashMap;

    let now = russell_core::time::now_unix();
    let window_start = now - 24 * 3600;

    // ── Build context blocks (same data as the legacy path) ──────────
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

    // ── Skill context ───────────────────────────────────────────────
    let actionable: Vec<serde_json::Value> = loaded_skills
        .iter()
        .filter(|s| s.is_actionable())
        .map(|s| {
            serde_json::json!({
                "id": s.id,
                "probes": s.probes.iter().map(|p| serde_json::json!({"id": p.id})).collect::<Vec<_>>(),
                "interventions": s.interventions.iter().map(|iv| serde_json::json!({"id": iv.id, "risk": format!("{:?}", iv.risk)})).collect::<Vec<_>>(),
            })
        })
        .collect();

    let knowledge: Vec<serde_json::Value> = loaded_skills
        .iter()
        .filter(|s| s.is_lens())
        .map(|s| {
            let symptoms = s.symptoms.join(", ");
            serde_json::json!({"id": s.id, "symptoms": symptoms})
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

    // ── Render template ─────────────────────────────────────────────
    let mut ctx = HashMap::new();
    ctx.insert("subjective".to_string(), serde_json::json!(subjective));
    ctx.insert(
        "profile_block".to_string(),
        serde_json::json!(profile_block),
    );
    ctx.insert(
        "severity_block".to_string(),
        serde_json::json!(severity_block),
    );
    ctx.insert(
        "samples_table".to_string(),
        serde_json::json!(samples_table),
    );
    ctx.insert(
        "freshness_block".to_string(),
        serde_json::json!(freshness_block),
    );
    ctx.insert("events_table".to_string(), serde_json::json!(events_table));
    if !reflex_section.is_empty() {
        ctx.insert(
            "reflex_section".to_string(),
            serde_json::json!(reflex_section),
        );
    }
    if !actionable.is_empty() {
        ctx.insert(
            "actionable_skills".to_string(),
            serde_json::json!(actionable),
        );
    }
    if !knowledge.is_empty() {
        ctx.insert("knowledge_skills".to_string(), serde_json::json!(knowledge));
    }
    if !kask_tools.is_empty() {
        ctx.insert("kask_tools".to_string(), serde_json::json!(kask_tools));
    }

    let (rendered, hint) = registry.render_with_hint("soap", &ctx)?;

    // ── System prompt: persona + relevance-scored knowledge ──────────
    let mut system_prompt = crate::JACK_PERSONA.to_string();
    append_skill_knowledge_scored(
        &mut system_prompt,
        loaded_skills,
        skills_base_dir,
        reader,
        window_start,
        skill_registry,
    );

    // ── Extract inference parameters from hint ──────────────────────
    let temperature = hint.as_ref().and_then(|h| h.temperature);
    let max_tokens = hint.as_ref().and_then(|h| h.max_tokens);

    // The objective is the rendered content minus the Subjective/Assessment/Plan
    // sections — it's the data Jack was given to reason about. For evidence
    // bundles and backward compatibility, we populate it from the rendered output.
    let objective = rendered.clone();

    Ok(SoapPrompt {
        system: system_prompt,
        subjective,
        objective,
        rendered,
        temperature,
        max_tokens,
    })
}

// ─── Block builders (shared between legacy and templated paths) ───────────

fn build_profile_block(profile: Option<&Profile>) -> String {
    let mut block = String::new();
    match profile {
        Some(p) => {
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
                    let _ = writeln!(block, "  - `{}` @ `{}` (role: {})", g.name, g.pci, g.role);
                }
            }
        }
        None => {
            let _ = writeln!(block, "- (no profile.json)");
        }
    }
    block.trim_end().to_string()
}

fn build_severity_block(reader: &JournalReader, window_start: i64) -> Result<String> {
    let counts = reader.severity_counts(window_start, i64::MAX)?;
    Ok(format!(
        "- info: {} | warn: {} | alert: {} | crit: {}",
        counts.info, counts.warn, counts.alert, counts.crit
    ))
}

fn build_samples_table(reader: &JournalReader, window_start: i64) -> Result<String> {
    let summaries = reader
        .host_samples_summary(window_start, i64::MAX)
        .unwrap_or_default();
    if summaries.is_empty() {
        return Ok("(no samples recorded)".to_string());
    }
    let baselines: std::collections::BTreeMap<String, (Option<f64>, Option<f64>)> = reader
        .read_baselines()
        .unwrap_or_default()
        .into_iter()
        .map(|b| (b.probe, (b.p95, b.ewma_mean)))
        .collect();
    let has_baselines = !baselines.is_empty();

    let mut table = String::new();
    if has_baselines {
        let _ = writeln!(
            table,
            "| probe | count | min | avg | max | last | p95 (30d) | ewma (7d) | unit |"
        );
        let _ = writeln!(table, "|---|---|---|---|---|---|---|---|---|");
    } else {
        let _ = writeln!(table, "| probe | count | min | avg | max | last | unit |");
        let _ = writeln!(table, "|---|---|---|---|---|---|---|");
    }
    for s in &summaries {
        let unit = s.unit.as_deref().unwrap_or("");
        if has_baselines {
            let (p95, ewma) = baselines
                .get(&s.probe)
                .map(|(p, e)| (*p, *e))
                .unwrap_or((None, None));
            let p95_str = p95.map(fmt_f64_baseline).unwrap_or_else(|| "—".to_string());
            let ewma_str = ewma
                .map(fmt_f64_baseline)
                .unwrap_or_else(|| "—".to_string());
            let _ = writeln!(
                table,
                "| {} | {} | {} | {} | {} | {} | {} | {} | {} |",
                s.probe,
                s.count,
                fmt_opt_f64(s.min),
                fmt_opt_f64(s.avg),
                fmt_opt_f64(s.max),
                fmt_opt_f64(s.last),
                p95_str,
                ewma_str,
                unit
            );
        } else {
            let _ = writeln!(
                table,
                "| {} | {} | {} | {} | {} | {} | {} |",
                s.probe,
                s.count,
                fmt_opt_f64(s.min),
                fmt_opt_f64(s.avg),
                fmt_opt_f64(s.max),
                fmt_opt_f64(s.last),
                unit
            );
        }
    }
    Ok(table.trim_end().to_string())
}

fn build_freshness_block(reader: &JournalReader) -> String {
    let age = last_sample_age(reader).unwrap_or(-1);
    if age >= 0 {
        format!("- Last sample {} seconds ago.", age)
    } else {
        "- No samples recorded.".to_string()
    }
}

fn build_events_table(reader: &JournalReader) -> Result<String> {
    let rows = reader.recent(20)?;
    if rows.is_empty() {
        return Ok("- (no events recorded)".to_string());
    }
    let mut table = String::new();
    let _ = writeln!(
        table,
        "| ts | severity | scope | module | action | summary |"
    );
    let _ = writeln!(table, "|---|---|---|---|---|---|");
    for r in rows {
        let _ = writeln!(
            table,
            "| {} | {} | {} | {} | {} | {} |",
            r.ts,
            r.severity.as_str(),
            r.scope.as_str(),
            r.module.as_deref().unwrap_or("-"),
            r.action,
            r.summary.as_deref().unwrap_or("")
        );
    }
    Ok(table.trim_end().to_string())
}

fn build_reflex_block(reader: &JournalReader) -> Result<String> {
    let now = russell_core::time::now_unix();
    let since = now - 7 * 86_400;
    let rows = reader.list_reflex_events(since, now)?;
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
    let _ = writeln!(
        block,
        "- If any reflex arc above is within the risk cap, propose it via ACTION: <intervention>."
    );
    Ok(block.trim_end().to_string())
}

/// Derive active symptom signals from recent journal events and probe data.
///
/// Bridges the vocabulary gap between sentinel probe names
/// (e.g., "gpu_vram_used_pct") and symptom catalog entries
/// (e.g., "vram_oom") using keyword extraction.
///
/// Sources:
/// 1. Module paths from elevated-severity events (contains probe names)
/// 2. Summary text keyword extraction
/// 3. Direct symptom signals from event action/tier fields
///
/// The resulting list is used for fuzzy matching against skill symptoms:
/// a skill declaring `symptoms: [vram_oom]` will match if "vram" appears
/// in the active symptoms.
fn derive_active_symptoms(events: &[russell_core::journal::EventRow]) -> Vec<String> {
    let mut symptoms = Vec::new();

    for ev in events {
        // Only consider elevated-severity events.
        let sev = ev.severity.as_str();
        if sev != "warn" && sev != "alert" && sev != "crit" {
            continue;
        }

        // Source 1: module paths contain probe names.
        // "sentinel/threshold/gpu_vram_used_pct" → "gpu_vram_used_pct"
        // "sentinel/rate/loadavg_1m" → "loadavg_1m"
        // "skill/okapi-watcher" → "okapi-watcher"
        if let Some(module) = &ev.module {
            if let Some(probe) = module.strip_prefix("sentinel/threshold/") {
                symptoms.push(probe.to_string());
                // Extract keywords from probe name.
                extract_keywords(probe, &mut symptoms);
            } else if let Some(probe) = module.strip_prefix("sentinel/rate/") {
                symptoms.push(probe.to_string());
                extract_keywords(probe, &mut symptoms);
            } else if let Some(skill_id) = module.strip_prefix("skill/") {
                symptoms.push(skill_id.to_string());
            }
        }

        // Source 2: summary text — extract known symptom indicator keywords.
        if let Some(summary) = &ev.summary {
            let lower = summary.to_lowercase();
            for keyword in [
                "oom",
                "swap",
                "gpu",
                "vram",
                "timeout",
                "stall",
                "degraded",
                "slow",
                "zombie",
                "pressure",
                "exhaustion",
                "drift",
                "skew",
                "bloat",
                "corruption",
            ] {
                if lower.contains(keyword) {
                    symptoms.push(keyword.to_string());
                }
            }
        }

        // Source 3: tier field — "sentinel" events about self-vitals.
        if ev.tier.as_deref() == Some("self_vital") {
            if let Some(module) = &ev.module {
                // "proprio/llm_p95_latency_ms" → keywords
                if let Some(vital) = module.strip_prefix("proprio/") {
                    extract_keywords(vital, &mut symptoms);
                }
            }
        }
    }

    symptoms.sort();
    symptoms.dedup();
    symptoms
}

/// Extract meaningful keywords from a probe/vital name (split on `_`, skip noise).
fn extract_keywords(name: &str, out: &mut Vec<String>) {
    for keyword in name.split('_') {
        if keyword.len() >= 3
            && !matches!(
                keyword,
                "used"
                    | "pct"
                    | "mib"
                    | "avg"
                    | "max"
                    | "min"
                    | "total"
                    | "count"
                    | "the"
                    | "last"
                    | "run"
            )
        {
            out.push(keyword.to_string());
        }
    }
}

/// Append KNOWLEDGE.md with relevance scoring and token budgeting.
///
/// Uses the prompt registry's relevance scoring to select which
/// knowledge skills to include based on current alert state.
///
/// **Inter-session feedback loop:** When `skill_registry` is provided,
/// execution telemetry (success/failure rates) modulates relevance —
/// skills that have been reliable get boosted, skills that have been
/// failing get deprioritized. This closes the loop between action
/// outcomes and future attention allocation.
fn append_skill_knowledge_scored(
    system: &mut String,
    skills: &[Skill],
    skills_base_dir: &Path,
    reader: &JournalReader,
    _window_start: i64,
    skill_registry: Option<&russell_skills::registry::RegistryCache>,
) {
    // Derive active symptoms from two sources:
    //
    // 1. Probe names from recent threshold breaches — sentinel events
    //    record `outputs["probe"]` with values like "gpu_vram_used_pct",
    //    "loadavg_1m", "swap_used_mib".
    //
    // 2. Skill IDs from recent skill-related events — skill execution
    //    events record module as "skill/<id>".
    //
    // We synthesize "active symptoms" by extracting these signals and
    // matching them against the symptom catalog using keyword overlap.
    // A symptom like "vram_oom" matches a probe like "gpu_vram_used_pct"
    // because they share the "vram" keyword.
    let recent_events = reader.recent(20).unwrap_or_default();
    let active_symptoms = derive_active_symptoms(&recent_events);

    // Budget: ~3000 tokens for knowledge injection.
    const KNOWLEDGE_BUDGET_TOKENS: usize = 3000;

    let mut slots: Vec<KnowledgeSlot> = Vec::new();
    for skill in skills {
        let applies = skill.applies_when.iter().any(|clause| {
            matches!(clause, russell_skills::AppliesWhen::Scalar { os_family: Some(os), .. } if os == "linux")
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
            // Gap 4: Structural relevance — since the applies_when filter above
            // already ensures only host-relevant skills reach this point,
            // always pass applies_when_match=true for the structural relevance floor.
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

    let selected = select_knowledge(&mut slots, KNOWLEDGE_BUDGET_TOKENS);
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
            "appended knowledge (relevance-scored, telemetry-modulated)",
        );
    }
}

/// Sanitize skill knowledge content before injection into system prompt.
///
/// Task 3.4: Security hardening against prompt injection.
/// Performs the following sanitization:
/// 1. Strip markdown code blocks (``` ... ```) — potential shell injection
/// 2. Remove URLs (http://, https://) — potential exfiltration targets
/// 3. Strip ACTION: patterns — prevent nested action injection
/// 4. Limit to 4KB max — prevent prompt bloat
///
/// Returns sanitized content, or `None` if content is empty after sanitization.
fn sanitize_knowledge(content: &str) -> Option<String> {
    let mut sanitized = content.to_string();

    // 1. Strip markdown code blocks (fence-style: ``` ... ```).
    // Use a simple state machine to remove fenced blocks.
    let mut result = String::with_capacity(sanitized.len());
    let mut in_fence = false;
    let mut lines = sanitized.lines();

    while let Some(line) = lines.next() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            in_fence = !in_fence;
            continue; // skip the fence line itself
        }
        if !in_fence {
            result.push_str(line);
            result.push('\n');
        }
    }
    sanitized = result;

    // 2. Remove URLs — potential exfiltration targets.
    // Simple regex-free approach: find and remove http:// and https:// URLs.
    let mut url_filtered = String::with_capacity(sanitized.len());
    let mut chars = sanitized.chars().peekable();
    while let Some(c) = chars.next() {
        // Check for http:// or https://
        if c == 'h' {
            let rest: String = chars.clone().take(7).collect();
            if rest.starts_with("ttp://") || rest.starts_with("ttps://") {
                // Skip until whitespace or end
                loop {
                    match chars.next() {
                        Some(ch) if !ch.is_whitespace() && !matches!(ch, ')' | ']' | '>') => {}
                        _ => break,
                    }
                }
                continue;
            }
        }
        url_filtered.push(c);
    }
    sanitized = url_filtered;

    // 3. Strip ACTION: patterns — prevent nested action injection.
    // Remove any line starting with ACTION:
    sanitized = sanitized
        .lines()
        .filter(|line| !line.trim().starts_with("ACTION:"))
        .collect::<Vec<_>>()
        .join("\n");

    // 4. Limit to 4KB max.
    sanitized.truncate(4096);

    // Return None if empty after sanitization.
    let trimmed = sanitized.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Append KNOWLEDGE.md content from any loaded skill that has one.
///
/// Knowledge files give Jack domain expertise (Ubuntu conventions,
/// ROCm troubleshooting, etc.) without bloating the base persona.
/// Only skills whose `applies_when` matches the machine profile
/// (currently: Linux) are included.
///
/// Task 3.2: All knowledge content is sanitized via [`sanitize_knowledge`]
/// before injection to prevent prompt injection, shell injection via
/// code blocks, and URL-based exfiltration.
fn append_skill_knowledge(system: &mut String, skills: &[Skill], skills_base_dir: &Path) {
    for skill in skills {
        // Skip skills with no applies_when or that don't match Linux.
        let applies = skill.applies_when.iter().any(|clause| {
            matches!(clause, russell_skills::AppliesWhen::Scalar {
                os_family: Some(os),
                ..
            } if os == "linux")
        });
        if !applies && !skill.applies_when.is_empty() {
            continue;
        }

        let knowledge_path = skills_base_dir.join(&skill.id).join("KNOWLEDGE.md");
        if !knowledge_path.exists() {
            continue;
        }

        match std::fs::read_to_string(&knowledge_path) {
            Ok(content) => {
                if content.trim().is_empty() {
                    continue;
                }

                // Task 3.2: Sanitize before injection.
                if let Some(sanitized) = sanitize_knowledge(&content) {
                    system.push_str("\n\n---\n\n");
                    system.push_str("# Knowledge: ");
                    system.push_str(&skill.id);
                    system.push_str("\n\n");
                    system.push_str(&sanitized);
                    tracing::debug!(
                        skill = %skill.id,
                        original_chars = content.len(),
                        sanitized_chars = sanitized.len(),
                        "appended sanitized skill knowledge to system prompt",
                    );
                } else {
                    tracing::warn!(
                        skill = %skill.id,
                        path = %knowledge_path.display(),
                        "skill knowledge was empty after sanitization (potential injection blocked)",
                    );
                }
            }
            Err(e) => {
                tracing::warn!(
                    skill = %skill.id,
                    path = %knowledge_path.display(),
                    error = %e,
                    "failed to read skill knowledge file",
                );
            }
        }
    }
}

fn last_sample_age(reader: &JournalReader) -> Option<i64> {
    let ts = reader.last_host_sample_ts().ok().flatten()?;
    let now = russell_core::time::now_unix();
    Some(now - ts)
}

/// Format an `Option<f64>` for a Markdown table cell.
fn fmt_opt_f64(v: Option<f64>) -> String {
    match v {
        Some(x) => {
            if x.fract() == 0.0 && x.abs() < 1_000_000.0 {
                format!("{x:.0}")
            } else if x.abs() < 100.0 {
                format!("{x:.2}")
            } else {
                format!("{x:.1}")
            }
        }
        None => "—".into(),
    }
}

/// Build the reflex actions section: list reflex_proposed events from
/// the last 7 days so Jack can see and propose the interventions.
fn build_reflex_section(reader: &JournalReader, objective: &mut String) -> Result<()> {
    let now = russell_core::time::now_unix();
    let since = now - 7 * 86_400;

    let rows = reader.list_reflex_events(since, now)?;
    if rows.is_empty() {
        return Ok(());
    }

    writeln!(objective, "\n### Reflex arcs — proposed interventions")?;
    writeln!(objective, "| ts | severity | intervention | summary |")?;
    writeln!(objective, "|---|---|---|---|")?;
    for (sev, intervention, summary, ts) in &rows {
        writeln!(objective, "| {ts} | {sev} | `{intervention}` | {summary} |")?;
    }

    writeln!(
        objective,
        "- If any reflex arc above is within the risk cap, propose it via ACTION: <intervention>."
    )?;
    Ok(())
}

/// Format a baseline f64 value for the p95 column.
fn fmt_f64_baseline(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1_000_000.0 {
        format!("{v:.0}")
    } else if v.abs() < 100.0 {
        format!("{v:.2}")
    } else {
        format!("{v:.1}")
    }
}

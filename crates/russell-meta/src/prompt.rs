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
//! The `compose_with_hkask_templated` function uses the prompt registry
//! to render templates from `.md.j2` files. The legacy `compose_with_hkask`
//! function retains the original `writeln!()` approach for backward
//! compatibility while callers migrate.

use std::fmt::Write as _;
use std::path::Path;

use russell_core::Profile;
#[cfg(test)]
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
pub fn compose(
    reader: &JournalReader,
    profile: Option<&Profile>,
    note: Option<&str>,
    loaded_skills: &[Skill],
    skills_base_dir: &Path,
) -> Result<SoapPrompt> {
    let registry = PromptRegistry::with_defaults()?;
    compose_templated(
        &registry,
        reader,
        profile,
        note,
        loaded_skills,
        skills_base_dir,
        &[],
        None,
    )
}

/// Compose a SOAP prompt with HKask MCP tools available.
///
/// Loads all templates, gathers journal data, and renders the SOAP
/// prompt with HKask tool names included in the prompt context.
pub fn compose_with_hkask(
    reader: &JournalReader,
    profile: Option<&Profile>,
    note: Option<&str>,
    loaded_skills: &[Skill],
    skills_base_dir: &Path,
    hkask_tool_names: &[(String, Option<String>)],
) -> Result<SoapPrompt> {
    let registry = PromptRegistry::with_defaults()?;
    compose_templated(
        &registry,
        reader,
        profile,
        note,
        loaded_skills,
        skills_base_dir,
        hkask_tool_names,
        None,
    )
}
/// Compose a SOAP prompt using MiniJinja templates.
///
/// Gathers journal data (profile, severity counts, samples, events),
/// renders the template with the given context, and returns the
/// complete prompt with inference hints.
#[allow(clippy::too_many_arguments)]
pub fn compose_templated(
    registry: &PromptRegistry,
    reader: &JournalReader,
    profile: Option<&Profile>,
    note: Option<&str>,
    loaded_skills: &[Skill],
    skills_base_dir: &Path,
    hkask_tool_names: &[(String, Option<String>)],
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

    let hkask_tools: Vec<serde_json::Value> = hkask_tool_names
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
    if !hkask_tools.is_empty() {
        ctx.insert("hkask_tools".to_string(), serde_json::json!(hkask_tools));
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
    let baseline_rows: Vec<russell_core::journal::BaselineRow> =
        reader.read_baselines().unwrap_or_default();

    // Task F3: Check baseline freshness (max age = 7 days = 168 hours)
    const BASELINE_FRESHNESS_HOURS: u32 = 168;
    let stale_count = baseline_rows
        .iter()
        .filter(|b| b.is_stale(BASELINE_FRESHNESS_HOURS))
        .count();
    let has_stale_baselines = stale_count > 0;

    let baselines: std::collections::BTreeMap<String, (Option<f64>, Option<f64>)> = baseline_rows
        .into_iter()
        .map(|b| (b.probe, (b.p95, b.ewma_mean)))
        .collect();
    let has_baselines = !baselines.is_empty();

    let mut table = String::new();

    // Task F3: Add freshness warning if baselines are stale
    if has_stale_baselines {
        let _ = writeln!(
            table,
            "> ⚠️  **Baseline freshness warning:** {} probe(s) have stale baselines (last updated >{} days ago). Interpret p95/ewma columns with caution.",
            stale_count,
            BASELINE_FRESHNESS_HOURS / 24
        );
        let _ = writeln!(table);
    }

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
        if ev.tier.as_deref() == Some("self_vital")
            && let Some(module) = &ev.module
        {
            // "proprio/llm_p95_latency_ms" → keywords
            if let Some(vital) = module.strip_prefix("proprio/") {
                extract_keywords(vital, &mut symptoms);
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

    let selected = select_knowledge(&slots, KNOWLEDGE_BUDGET_TOKENS);
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

#[cfg(test)]
mod tests {
    use super::*;
    use russell_core::event::{Event, Severity};
    use russell_core::journal::JournalWriter;

    #[test]
    fn compose_handles_empty_journal() {
        let tmp = tempfile::tempdir().unwrap();
        let db = tmp.path().join("journal.db");
        let w = JournalWriter::open(&db).unwrap();
        let reader = w.reader();
        let skills_dir = tmp.path().join("skills");
        std::fs::create_dir_all(&skills_dir).unwrap();
        let prompt = compose(&reader, None, None, &[], Path::new("/nonexistent")).unwrap();
        assert!(prompt.rendered.contains("## Subjective"));
        assert!(prompt.rendered.contains("(no operator note)"));
        assert!(prompt.rendered.contains("(no events recorded)"));
        assert!(prompt.system.contains("You are Jack"));
        // F-2: empty sample summary should show placeholder.
        assert!(prompt.rendered.contains("(no samples recorded)"));
    }

    #[test]
    fn compose_includes_note_and_events() {
        let tmp = tempfile::tempdir().unwrap();
        let db = tmp.path().join("journal.db");
        let w = JournalWriter::open(&db).unwrap();
        let mut e = Event::new("observe", Severity::Warn);
        e.module = Some("daily/gpu-sanity".into());
        e.summary = Some("one vm fault".into());
        w.append(&e).unwrap();
        let reader = w.reader();
        let skills_dir = tmp.path().join("skills");
        std::fs::create_dir_all(&skills_dir).unwrap();
        let prompt = compose(
            &reader,
            None,
            Some("ollama is slow"),
            &[],
            Path::new("/nonexistent"),
        )
        .unwrap();
        assert!(prompt.rendered.contains("ollama is slow"));
        assert!(prompt.rendered.contains("daily/gpu-sanity"));
        assert!(prompt.rendered.contains("warn"));
    }

    #[test]
    fn compose_includes_sample_summary() {
        let tmp = tempfile::tempdir().unwrap();
        let db = tmp.path().join("journal.db");
        let w = JournalWriter::open(&db).unwrap();
        let now = russell_core::time::now_unix();

        // Write a few host-scope samples across multiple probes.
        w.append_sample(
            now - 3600,
            Scope::Host,
            "mem_available_mib",
            Some(91000.0),
            None,
            Some("MiB"),
        )
        .unwrap();
        w.append_sample(
            now - 1800,
            Scope::Host,
            "mem_available_mib",
            Some(90500.0),
            None,
            Some("MiB"),
        )
        .unwrap();
        w.append_sample(
            now - 600,
            Scope::Host,
            "mem_available_mib",
            Some(90200.0),
            None,
            Some("MiB"),
        )
        .unwrap();
        w.append_sample(
            now - 3600,
            Scope::Host,
            "loadavg_1m",
            Some(0.45),
            None,
            None,
        )
        .unwrap();
        w.append_sample(now - 600, Scope::Host, "loadavg_1m", Some(1.2), None, None)
            .unwrap();
        w.append_sample(
            now - 3600,
            Scope::Host,
            "swap_used_mib",
            Some(3200.0),
            None,
            Some("MiB"),
        )
        .unwrap();
        w.append_sample(
            now - 600,
            Scope::Host,
            "swap_used_mib",
            Some(3500.0),
            None,
            Some("MiB"),
        )
        .unwrap();

        let reader = w.reader();
        let skills_dir = tmp.path().join("skills");
        std::fs::create_dir_all(&skills_dir).unwrap();
        let prompt = compose(
            &reader,
            None,
            Some("checking trends"),
            &[],
            Path::new("/nonexistent"),
        )
        .unwrap();

        // The sample summary table should appear with all three probes.
        assert!(
            prompt
                .rendered
                .contains("### Host probe samples — last 24h")
        );
        assert!(prompt.rendered.contains("mem_available_mib"));
        assert!(prompt.rendered.contains("loadavg_1m"));
        assert!(prompt.rendered.contains("swap_used_mib"));

        // Count column should reflect the number of data points.
        assert!(prompt.rendered.contains("| mem_available_mib | 3 |"));

        // Should see the MiB unit for mem/swap probes.
        assert!(prompt.rendered.contains("| MiB |"));

        // F-2: operator note still present.
        assert!(prompt.rendered.contains("checking trends"));
    }

    #[test]
    fn compose_with_hkask_includes_hkask_tools_section() {
        let tmp = tempfile::tempdir().unwrap();
        let db = tmp.path().join("journal.db");
        let w = JournalWriter::open(&db).unwrap();
        let reader = w.reader();
        let skills_dir = tmp.path().join("skills");
        std::fs::create_dir_all(&skills_dir).unwrap();

        let hkask_tools = vec![
            (
                "paradigm_shift_query".to_string(),
                Some("medium".to_string()),
            ),
            (
                "russell_host_snapshot".to_string(),
                Some("none".to_string()),
            ),
        ];

        let prompt = compose_with_hkask(
            &reader,
            None,
            Some("test hkask tools"),
            &[],
            Path::new("/nonexistent"),
            &hkask_tools,
        )
        .unwrap();

        assert!(
            prompt.rendered.contains("### HKask MCP tools"),
            "should include HKask MCP tools section"
        );
        assert!(
            prompt.rendered.contains("paradigm_shift_query"),
            "should list paradigm_shift_query"
        );
        assert!(
            prompt.rendered.contains("russell_host_snapshot"),
            "should list russell_host_snapshot"
        );
        assert!(
            prompt.rendered.contains("ACTION: hkask/"),
            "should include HKask ACTION syntax"
        );
    }

    #[test]
    fn compose_with_hkask_empty_tools_no_section() {
        let tmp = tempfile::tempdir().unwrap();
        let db = tmp.path().join("journal.db");
        let w = JournalWriter::open(&db).unwrap();
        let reader = w.reader();

        let hkask_tools: Vec<(String, Option<String>)> = vec![];

        let prompt = compose_with_hkask(
            &reader,
            None,
            Some("test empty hkask"),
            &[],
            Path::new("/nonexistent"),
            &hkask_tools,
        )
        .unwrap();

        assert!(
            !prompt.rendered.contains("### HKask MCP tools"),
            "should NOT include HKask section when no tools available"
        );
    }
}

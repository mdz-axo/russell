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

use std::fmt::Write as _;
use std::path::Path;

use russell_core::journal::JournalReader;
use russell_core::{Profile, event::Scope};
use russell_skills::Skill;

use crate::client::SoapPrompt;
use crate::error::Result;

/// Build the SOAP prompt. The system prompt is always the
/// embedded Jack persona.
///
/// If `loaded_skills` is provided, Jack is also told what
/// probes and interventions are available so he can recommend
/// specific manifest IDs per ADR-0008 (LLM ranks IDs, does not
/// emit shell).
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
        let baselines: std::collections::BTreeMap<String, f64> = reader
            .read_baselines()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|b| b.p95.map(|v| (b.probe, v)))
            .collect();
        let has_baselines = !baselines.is_empty();

        if has_baselines {
            writeln!(
                objective,
                "| probe | count | min | avg | max | last | p95 (30d) | unit |"
            )?;
            writeln!(objective, "|---|---|---|---|---|---|---|---|")?;
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
                let p95 = baselines
                    .get(&s.probe)
                    .map(|v| fmt_f64_baseline(*v))
                    .unwrap_or_else(|| "—".to_string());
                writeln!(
                    objective,
                    "| {} | {} | {} | {} | {} | {} | {} | {} |",
                    s.probe,
                    s.count,
                    fmt_opt_f64(s.min),
                    fmt_opt_f64(s.avg),
                    fmt_opt_f64(s.max),
                    fmt_opt_f64(s.last),
                    p95,
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
                "\nWhen you identify an intervention and a skill is loaded, \
                 propose it on the final line using:\n\n\
                 ACTION: <skill-id>/<intervention-id>\n\n\
                 (e.g. ACTION: okapi-watcher/restart-okapi). \
                 Only propose interventions, not probes. \
                 The operator must consent before execution."
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
            let sev = match r.severity {
                russell_core::event::Severity::Info => "info",
                russell_core::event::Severity::Warn => "warn",
                russell_core::event::Severity::Alert => "alert",
                russell_core::event::Severity::Crit => "crit",
            };
            let scope = match r.scope {
                Scope::Host => "host",
                Scope::Self_ => "self",
            };
            writeln!(
                objective,
                "| {} | {} | {} | {} | {} | {} |",
                r.ts,
                sev,
                scope,
                r.module.as_deref().unwrap_or("-"),
                r.action,
                r.summary.as_deref().unwrap_or("")
            )?;
        }
    }

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
    })
}

/// Append KNOWLEDGE.md content from any loaded skill that has one.
///
/// Knowledge files give Jack domain expertise (Ubuntu conventions,
/// ROCm troubleshooting, etc.) without bloating the base persona.
/// Only skills whose `applies_when` matches the machine profile
/// (currently: Linux) are included.
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
                system.push_str("\n\n---\n\n");
                system.push_str("# Knowledge: ");
                system.push_str(&skill.id);
                system.push_str("\n\n");
                system.push_str(&content);
                tracing::debug!(
                    skill = %skill.id,
                    chars = content.len(),
                    "appended skill knowledge to system prompt",
                );
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
    let ts = reader.last_sample_ts().ok().flatten()?;
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
}

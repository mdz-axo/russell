// SPDX-License-Identifier: MIT OR Apache-2.0
//! SOAP objective builder for the chat REPL.
//!
//! Formats the current journal state into a Markdown block
//! that Jack can consume each turn. This must provide the same
//! signals as the `russell chat` SOAP prompt — including
//! baselines, reflex arcs, self-vitals, and skill telemetry —
//! so Jack has full metacognitive awareness of the system and
//! his own capabilities regardless of interface mode.

use russell_core::journal::JournalReader;
use russell_skills::Skill;
use russell_skills::registry::RegistryCache;
use std::fmt::Write as _;

/// Build the SOAP objective Markdown from journal state and skills.
///
/// This is Jack's window into reality each turn. Every signal that
/// could inform his assessment or action must be present here.
pub fn build_objective(
    reader: &JournalReader,
    skills: &[Skill],
    profile: Option<&russell_core::Profile>,
    registry: &RegistryCache,
) -> String {
    let now = russell_core::time::now_unix();
    let window_start = now - 24 * 3600;
    let mut obj = String::new();

    // ── Profile ─────────────────────────────────────────────────────
    if let Some(p) = profile {
        let _ = writeln!(obj, "### Machine");
        let _ = writeln!(
            obj,
            "- os: `{}/{}` kernel `{}`",
            p.host.os.distro, p.host.os.version, p.host.os.kernel
        );
        if !p.host.cpu.model.is_empty() {
            let _ = writeln!(
                obj,
                "- cpu: `{}` ({} cores / {} threads)",
                p.host.cpu.model, p.host.cpu.cores, p.host.cpu.threads
            );
        }
        if !p.gpus.is_empty() {
            let _ = writeln!(obj, "- gpus:");
            for g in &p.gpus {
                let _ = writeln!(obj, "  - `{}` @ `{}` (role: {})", g.name, g.pci, g.role);
            }
        }
    }

    // ── Severity counts ─────────────────────────────────────────────
    let _ = writeln!(obj, "\n### Severity — last 24h");
    if let Ok(counts) = reader.severity_counts(window_start, i64::MAX) {
        let _ = writeln!(
            obj,
            "- info: {} | warn: {} | alert: {} | crit: {}",
            counts.info, counts.warn, counts.alert, counts.crit
        );
    }

    // ── Sample summary with baselines ───────────────────────────────
    if let Ok(summaries) = reader.host_samples_summary(window_start, i64::MAX)
        && !summaries.is_empty()
    {
        // Load baselines for p95/EWMA columns.
        let baselines: std::collections::BTreeMap<String, (Option<f64>, Option<f64>)> = reader
            .read_baselines()
            .unwrap_or_default()
            .into_iter()
            .map(|b| (b.probe, (b.p95, b.ewma_mean)))
            .collect();
        let has_baselines = !baselines.is_empty();

        let _ = writeln!(obj, "\n### Host samples — last 24h");
        if has_baselines {
            let _ = writeln!(
                obj,
                "| probe | count | min | avg | max | last | p95 (30d) | ewma (7d) | unit |"
            );
            let _ = writeln!(obj, "|---|---|---|---|---|---|---|---|---|");
        } else {
            let _ = writeln!(obj, "| probe | count | min | avg | max | last | unit |");
            let _ = writeln!(obj, "|---|---|---|---|---|---|---|");
        }
        for s in &summaries {
            let unit = s.unit.as_deref().unwrap_or("");
            if has_baselines {
                let (p95, ewma) = baselines
                    .get(&s.probe)
                    .map(|(p, e)| (*p, *e))
                    .unwrap_or((None, None));
                let p95_str = p95
                    .map(|v| fmt_f64(Some(v)))
                    .unwrap_or_else(|| "—".to_string());
                let ewma_str = ewma
                    .map(|v| fmt_f64(Some(v)))
                    .unwrap_or_else(|| "—".to_string());
                let _ = writeln!(
                    obj,
                    "| {} | {} | {} | {} | {} | {} | {} | {} | {} |",
                    s.probe,
                    s.count,
                    fmt_f64(s.min),
                    fmt_f64(s.avg),
                    fmt_f64(s.max),
                    fmt_f64(s.last),
                    p95_str,
                    ewma_str,
                    unit,
                );
            } else {
                let _ = writeln!(
                    obj,
                    "| {} | {} | {} | {} | {} | {} | {} |",
                    s.probe,
                    s.count,
                    fmt_f64(s.min),
                    fmt_f64(s.avg),
                    fmt_f64(s.max),
                    fmt_f64(s.last),
                    unit,
                );
            }
        }
    }

    // ── Sentinel freshness ──────────────────────────────────────────
    if let Ok(Some(ts)) = reader.last_host_sample_ts() {
        let age = now.saturating_sub(ts);
        let _ = writeln!(obj, "\n### Freshness\n- Last sample {} seconds ago.", age);
    }

    // ── Self-vitals (proprioception) ────────────────────────────────
    if let Ok(self_summaries) = reader.self_samples_summary(window_start, i64::MAX)
        && !self_summaries.is_empty()
    {
        let _ = writeln!(obj, "\n### Self-health (Russell's own vitals)");
        let _ = writeln!(obj, "| vital | last | avg | unit |");
        let _ = writeln!(obj, "|---|---|---|---|");
        for s in &self_summaries {
            let unit = s.unit.as_deref().unwrap_or("");
            let _ = writeln!(
                obj,
                "| {} | {} | {} | {} |",
                s.probe,
                fmt_f64(s.last),
                fmt_f64(s.avg),
                unit,
            );
        }
    }

    // ── Reflex arcs (proposed interventions from sentinel) ──────────
    if let Ok(rows) = reader.list_reflex_events(now - 7 * 86_400, now)
        && !rows.is_empty()
    {
        let _ = writeln!(obj, "\n### Reflex arcs — proposed interventions (last 7d)");
        let _ = writeln!(obj, "| severity | intervention | summary |");
        let _ = writeln!(obj, "|---|---|---|");
        for (sev, intervention, summary, _ts) in &rows {
            let _ = writeln!(obj, "| {} | `{}` | {} |", sev, intervention, summary);
        }
        let _ = writeln!(
            obj,
            "\nIf a reflex arc above is within the risk cap, you can propose it via ACTION syntax."
        );
    }

    // ── Recent events ───────────────────────────────────────────────
    if let Ok(rows) = reader.recent(5)
        && !rows.is_empty()
    {
        let _ = writeln!(obj, "\n### Recent events");
        for r in &rows {
            let _ = writeln!(
                obj,
                "- [{sev:?}] {action}: {summary}",
                summary = r.summary.as_deref().unwrap_or("(no summary)"),
                sev = r.severity,
                action = r.action,
            );
        }
    }

    // ── Skill telemetry (expanded) ──────────────────────────────────
    if !registry.skills.is_empty() {
        let _ = writeln!(obj, "\n### Skill Performance");
        let _ = writeln!(
            obj,
            "| skill | probes | p.fails | interventions | i.fails | success rate | last run |"
        );
        let _ = writeln!(obj, "|---|---|---|---|---|---|---|");
        for (id, entry) in &registry.skills {
            let last = entry.last_probe_run_at.as_deref().unwrap_or("never");
            let success = entry
                .ewma_success_rate
                .map(|r| format!("{:.0}%", r * 100.0))
                .unwrap_or_else(|| "—".to_string());
            let _ = writeln!(
                obj,
                "| {} | {} | {} | {} | {} | {} | {} |",
                id,
                entry.probe_runs,
                entry.recent_probe_failures,
                entry.intervention_runs,
                entry.recent_intervention_failures,
                success,
                last,
            );
        }
    }

    // ── Available skills ────────────────────────────────────────────
    if !skills.is_empty() {
        let _ = writeln!(obj, "\n### Available skills");
        for s in skills {
            for p in &s.probes {
                let _ = writeln!(obj, "- `{}`/`{}` (probe, risk: none)", s.id, p.id);
            }
            for iv in &s.interventions {
                let _ = writeln!(
                    obj,
                    "- `{}`/`{}` (intervention, risk: {:?})",
                    s.id, iv.id, iv.risk
                );
            }
        }
    }

    obj
}

/// Format an `Option<f64>` for display in SOAP tables.
pub fn fmt_f64(v: Option<f64>) -> String {
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

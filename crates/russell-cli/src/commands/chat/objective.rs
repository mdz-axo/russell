// SPDX-License-Identifier: MIT OR Apache-2.0
//! SOAP objective builder for the chat REPL.
//!
//! Formats the current journal state into a Markdown block
//! that Jack can consume each turn.

use russell_core::journal::JournalReader;
use russell_mcp::registry::ToolRegistry;
use russell_skills::Skill;
use russell_skills::registry::RegistryCache;
use std::fmt::Write as _;

/// Build the SOAP objective Markdown from journal state and skills.
pub fn build_objective(
    reader: &JournalReader,
    skills: &[Skill],
    profile: Option<&russell_core::Profile>,
    kask_registry: &ToolRegistry,
    registry: &RegistryCache,
) -> String {
    let now = russell_core::time::now_unix();
    let window_start = now - 24 * 3600;
    let mut obj = String::new();

    // Profile.
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

    // Severity counts.
    let _ = writeln!(obj, "\n### Severity — last 24h");
    if let Ok(counts) = reader.severity_counts(window_start, i64::MAX) {
        let _ = writeln!(
            obj,
            "- info: {} | warn: {} | alert: {} | crit: {}",
            counts.info, counts.warn, counts.alert, counts.crit
        );
    }

    // Sample summary.
    if let Ok(summaries) = reader.host_samples_summary(window_start, i64::MAX)
        && !summaries.is_empty()
    {
        let _ = writeln!(obj, "\n### Host samples — last 24h");
        let _ = writeln!(obj, "| probe | count | min | avg | max | last | unit |");
        let _ = writeln!(obj, "|---|---|---|---|---|---|---|");
        for s in &summaries {
            let unit = s.unit.as_deref().unwrap_or("");
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

    // Sentinel freshness.
    if let Ok(Some(ts)) = reader.last_host_sample_ts() {
        let age = now.saturating_sub(ts);
        let _ = writeln!(obj, "\n### Freshness\n- Last sample {} seconds ago.", age);
    }

    // Recent events (last 5).
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

    // Skill telemetry from registry cache.
    if !registry.skills.is_empty() {
        let _ = writeln!(obj, "\n### Skill Performance");
        let _ = writeln!(obj, "| skill | probes | fails | last run |");
        let _ = writeln!(obj, "|---|---|---|---|");
        for (id, entry) in &registry.skills {
            let last = entry.last_probe_run_at.as_deref().unwrap_or("never");
            let _ = writeln!(
                obj,
                "| {} | {} | {} | {} |",
                id, entry.probe_runs, entry.recent_probe_failures, last,
            );
        }
    }

    // Available skills.
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

    // Kask MCP tools (ADR-0025).
    if !kask_registry.is_empty() {
        let _ = writeln!(obj, "\n### Kask MCP tools");
        for tool in kask_registry.tools() {
            let risk = kask_registry
                .tool_risk_band(&tool.name)
                .unwrap_or_else(|| "medium".into());
            let desc = tool
                .description
                .as_deref()
                .and_then(|d| d.lines().next())
                .unwrap_or("");
            let desc_short = if desc.len() > 60 {
                format!("{}…", &desc[..57])
            } else {
                desc.to_owned()
            };
            let _ = writeln!(
                obj,
                "- `kask`/`{}` (tool, risk: {}) — {}",
                tool.name, risk, desc_short
            );
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

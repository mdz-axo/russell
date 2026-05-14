// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell jack` — Jack's cry-for-help channel.

use anyhow::{Context, Result};
use russell_core::journal::JournalWriter;
use russell_core::paths::Paths;
use russell_doctor::action::{self, KaskToolInfo, ResolvedAction};
use russell_mcp::client::KaskMcpClient;
use russell_mcp::config::KaskMcpConfig;
use russell_mcp::registry::ToolRegistry;
use russell_skills::RiskBand;
use tracing::debug;

pub async fn run(paths: &Paths, note: Option<&str>) -> Result<()> {
    let writer = JournalWriter::open(&paths.journal())
        .with_context(|| format!("opening journal {}", paths.journal().display()))?;

    // Resolve and correct the model name before the help flow starts.
    let cfg = russell_doctor::client::ClientConfig::from_env();
    let resolved = russell_doctor::oai_client::resolve_and_correct_model(&cfg, &paths.config).await;
    if resolved != cfg.model {
        println!(
            "  Corrected: model \"{}\" → \"{}\" (env file updated)",
            cfg.model, resolved
        );
    }

    // Collect Kask MCP tool names for the SOAP prompt (ADR-0025 §7).
    // Graceful degradation: empty list if Kask is unreachable.
    let kask_tool_names = collect_kask_tool_names().await;
    if !kask_tool_names.is_empty() {
        debug!(count = kask_tool_names.len(), "kask tools available for jack");
    }

    let outcome = russell_doctor::run_help(paths, &writer, note, &kask_tool_names)
        .await
        .context("running Doctor help flow")?;

    // Print the response without trailing blank lines.
    let response = outcome.response.trim_end();
    println!("{response}");
    println!();

    // If Jack proposed an action, resolve it.
    let skills = russell_skills::load_all(&paths.skills()).unwrap_or_default();
    let kask_tool_infos = build_kask_tool_infos_from_names(&kask_tool_names);
    if let Some(action_result) = action::resolve_with_kask(response, &skills, &kask_tool_infos) {
        match action_result {
            Ok(action) => match action {
                ResolvedAction::Probe { .. } => {
                    println!(
                        "  → Running probe: {}/{}…",
                        action.skill_id(),
                        action.action_id()
                    );
                    execute_probe(paths, &writer, &action).await;
                }
                ResolvedAction::Intervention {
                    risk, needs_sudo, ..
                } => {
                    let sudo_tag = if needs_sudo { " [needs sudo]" } else { "" };
                    println!(
                        "  → Jack proposes: {}/{} (risk: {:?}{})",
                        action.skill_id(),
                        action.action_id(),
                        risk,
                        sudo_tag,
                    );
                    println!(
                        "  → Switch to `russell chat` and I'll run it — just say 'ok' when I ask."
                    );
                    println!();
                }
                ResolvedAction::KaskTool { .. } => {
                    println!("  → Jack proposes kask tool: {}.", action.action_id(),);
                    println!("  → Switch to `russell chat` to execute Kask tools interactively.");
                    println!();
                }
            },
            Err(e) => {
                println!("  → {e}");
            }
        }
    }

    println!(
        "  [jack via {} · session {} · bundle {}]",
        outcome.backend,
        outcome.session_id,
        outcome.evidence_dir.display()
    );

    if let Some(sr) = outcome.skip_reason {
        let msg = match sr {
            russell_doctor::help::SkipReason::OfflineFallback => {
                "  [offline fallback engaged — Ollama unreachable or LLM call failed]"
            }
            russell_doctor::help::SkipReason::ThresholdSkip => {
                "  [below escalation threshold — rule-based summary returned]"
            }
        };
        println!("{msg}");
    }

    Ok(())
}

/// Execute a probe immediately (read-only, risk: none).
async fn execute_probe(paths: &Paths, journal: &JournalWriter, action: &ResolvedAction) {
    use russell_skills::dispatch::{Dispatcher, DryRun, StepType};
    use std::time::Duration;

    let skill_dir = paths.skills().join(action.skill_id());
    let evidence_base = paths.evidence();
    let timeout = Duration::from_secs(30);

    let mut dispatcher = Dispatcher::new(&skill_dir);
    dispatcher.probe_timeout = timeout;
    dispatcher.dry_run = DryRun::Disabled;
    dispatcher.max_auto_risk = match action {
        ResolvedAction::Probe { max_auto_risk, .. } => *max_auto_risk,
        _ => russell_skills::RiskBand::None,
    };

    let result = dispatcher
        .run_and_journal(
            journal,
            &evidence_base,
            action.cmd(),
            action.skill_id(),
            action.action_id(),
            StepType::Probe,
            "none",
            Some(timeout),
        )
        .await;

    match result {
        Ok(outcome) => {
            if outcome.exit_code == Some(0) {
                if !outcome.stdout.is_empty() {
                    println!("  {}", outcome.stdout.trim());
                }
                println!(
                    "  → Probe {}/{} complete.",
                    action.skill_id(),
                    action.action_id()
                );
            } else {
                println!(
                    "  → Probe {}/{} exited with code {:?}.",
                    action.skill_id(),
                    action.action_id(),
                    outcome.exit_code
                );
                if !outcome.stderr.is_empty() {
                    println!("  stderr: {}", outcome.stderr.trim());
                }
            }
        }
        Err(e) => {
            println!("  → Failed to run probe: {e}");
        }
    }
    println!();
}

/// Collect Kask MCP tool names and risk bands for the SOAP prompt.
/// Returns an empty list on any failure (graceful degradation per ADR-0025 §5).
async fn collect_kask_tool_names() -> Vec<(String, Option<String>)> {
    let kask_config = KaskMcpConfig::from_env();
    if kask_config.validate().is_err() {
        return vec![];
    }

    let mut client = match KaskMcpClient::new(kask_config.clone()) {
        Ok(c) => c,
        Err(e) => {
            debug!(error = %e, "kask MCP client construction failed");
            return vec![];
        }
    };

    if client.connect().await.is_err() {
        debug!("kask MCP connect failed — tools unavailable this session");
        return vec![];
    }

    let mut registry = ToolRegistry::new(kask_config.tool_ttl);
    if let Err(e) = registry.refresh(&client).await {
        debug!(error = %e, "kask tool registry refresh failed");
        return vec![];
    }

    registry
        .tools()
        .iter()
        .map(|t| {
            let risk = registry.tool_risk_band(&t.name);
            (t.name.clone(), risk)
        })
        .collect()
}

/// Build [`KaskToolInfo`] list from (name, risk) tuples for the action resolver.
fn build_kask_tool_infos_from_names(names: &[(String, Option<String>)]) -> Vec<KaskToolInfo> {
    names
        .iter()
        .map(|(name, risk)| {
            let risk_band = risk
                .as_deref()
                .map(|s| match s {
                    "none" => RiskBand::None,
                    "low" => RiskBand::Low,
                    "medium" => RiskBand::Medium,
                    "high" => RiskBand::High,
                    "critical" => RiskBand::Critical,
                    _ => RiskBand::Medium,
                })
                .unwrap_or(RiskBand::Medium);
            KaskToolInfo {
                name: name.clone(),
                risk_band,
            }
        })
        .collect()
}

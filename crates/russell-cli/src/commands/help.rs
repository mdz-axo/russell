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

    // Collect Kask MCP tool infos for the SOAP prompt and action resolver (ADR-0025 §7).
    // Graceful degradation: empty list if Kask is unreachable.
    let kask_tool_infos = collect_kask_tool_infos(paths).await;
    if !kask_tool_infos.is_empty() {
        debug!(
            count = kask_tool_infos.len(),
            "kask tools available for jack"
        );
    }
    let kask_tool_names: Vec<(String, Option<String>)> = kask_tool_infos
        .iter()
        .map(|t| {
            let risk = match t.risk_band {
                RiskBand::None => Some("none".to_string()),
                RiskBand::Low => Some("low".to_string()),
                RiskBand::Medium => None,
                RiskBand::High => Some("high".to_string()),
                RiskBand::Critical => Some("critical".to_string()),
            };
            (t.name.clone(), risk)
        })
        .collect();

    let outcome = russell_doctor::run_help(paths, &writer, note, &kask_tool_names)
        .await
        .context("running Doctor help flow")?;

    // Print the response without trailing blank lines.
    let response = outcome.response.trim_end();
    println!("{response}");
    println!();

    // If Jack proposed an action, resolve it.
    let skills = russell_skills::load_all(&paths.skills()).unwrap_or_default();
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

/// Collect Kask MCP tool infos (name, risk, input_schema) for the SOAP prompt
/// and action resolver (ADR-0025 §7).
/// Returns an empty list on any failure (graceful degradation per ADR-0025 §5).
/// Falls back to the disk cache if Kask is unreachable.
async fn collect_kask_tool_infos(paths: &Paths) -> Vec<KaskToolInfo> {
    let kask_config = KaskMcpConfig::from_env();
    if kask_config.validate().is_err() {
        return vec![];
    }

    let mut client = match KaskMcpClient::new(kask_config.clone()) {
        Ok(c) => c,
        Err(e) => {
            debug!(error = %e, "kask MCP client construction failed");
            return load_cached_tool_infos(paths);
        }
    };

    if client.connect().await.is_err() {
        debug!("kask MCP connect failed — tools unavailable this session");
        return load_cached_tool_infos(paths);
    }

    let cache_path = paths.memory_dir().join("kask-tools.cache.json");
    let mut registry = ToolRegistry::new(kask_config.tool_ttl);
    let _ = registry.load_from_disk(&cache_path);

    if let Err(e) = registry.refresh(&client).await {
        debug!(error = %e, "kask tool registry refresh failed");
        // Return cached if refresh failed but we have cached data.
        if !registry.is_empty() {
            return registry_to_kask_infos(&registry);
        }
        return vec![];
    }

    // Persist fresh tools to disk.
    let _ = registry.save_to_disk(&cache_path);

    registry_to_kask_infos(&registry)
}

/// Load cached tool infos from the disk cache as a fallback.
fn load_cached_tool_infos(paths: &Paths) -> Vec<KaskToolInfo> {
    let cache_path = paths.memory_dir().join("kask-tools.cache.json");
    let mut registry = ToolRegistry::new(KaskMcpConfig::from_env().tool_ttl);
    if registry.load_from_disk(&cache_path).is_ok() && !registry.is_empty() {
        debug!(
            count = registry.tool_count(),
            "loaded kask tools from disk cache"
        );
        return registry_to_kask_infos(&registry);
    }
    vec![]
}

/// Convert a [`ToolRegistry`] to a [`KaskToolInfo`] list, preserving
/// `input_schema` from the cached [`McpToolDefinition`].
fn registry_to_kask_infos(registry: &ToolRegistry) -> Vec<KaskToolInfo> {
    registry
        .tools()
        .iter()
        .map(|t| {
            let risk_band = registry
                .tool_risk_band(&t.name)
                .map(|s| match s.as_str() {
                    "none" => RiskBand::None,
                    "low" => RiskBand::Low,
                    "medium" => RiskBand::Medium,
                    "high" => RiskBand::High,
                    "critical" => RiskBand::Critical,
                    _ => RiskBand::Medium,
                })
                .unwrap_or(RiskBand::Medium);
            KaskToolInfo {
                name: t.name.clone(),
                risk_band,
                input_schema: t.input_schema.clone(),
            }
        })
        .collect()
}

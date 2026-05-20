// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell jack` — Jack's cry-for-help channel.

use anyhow::{Context, Result};
use russell_core::journal::JournalWriter;
use russell_core::paths::Paths;
use russell_mcp::client::HKaskMcpClient;
use russell_mcp::config::HKaskMcpConfig;
use russell_mcp::registry::ToolRegistry;
use russell_meta::action::{self, HKaskToolInfo, ResolvedAction};
use russell_skills::RiskBand;
use tracing::debug;

pub async fn run(paths: &Paths, note: Option<&str>) -> Result<()> {
    let writer = JournalWriter::open(&paths.journal())
        .with_context(|| format!("opening journal {}", paths.journal().display()))?;

    // Resolve and correct the model name before the help flow starts.
    let cfg = russell_meta::client::ClientConfig::from_env();
    let resolved = russell_meta::oai_client::resolve_and_correct_model(&cfg, &paths.config).await;
    if resolved != cfg.model {
        println!(
            "  Corrected: model \"{}\" → \"{}\" (env file updated)",
            cfg.model, resolved
        );
    }

    // Collect hKask MCP tool infos for the SOAP prompt and action resolver (ADR-0025 §7).
    // Graceful degradation: empty list if hKask is unreachable.
    let hkask_tool_infos = collect_hkask_tool_infos(paths).await;
    if !hkask_tool_infos.is_empty() {
        debug!(
            count = hkask_tool_infos.len(),
            "hkask tools available for jack"
        );
    }
    let hkask_tool_names: Vec<(String, Option<String>)> = hkask_tool_infos
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

    let skills = russell_skills::load_all(&paths.skills()).unwrap_or_default();

    // Reconcile registry against disk (fix stale/orphan entries).
    {
        let registry_path = paths.state.join("registry").join("local-cache.yaml");
        let mut registry =
            russell_skills::registry::RegistryCache::load(&registry_path).unwrap_or_default();
        if registry.reconcile(&skills) {
            let _ = registry.save(&registry_path);
        }
    }

    let outcome = russell_meta::run_help(paths, &writer, note, &hkask_tool_names)
        .await
        .context("running Doctor help flow")?;

    // If Jack proposed a probe, run it FIRST and feed results back for analysis.
    let final_response = if let Some(action_result) =
        action::resolve_with_hkask(&outcome.response, &skills, &hkask_tool_infos)
    {
        match action_result {
            Ok(action) => match &action {
                ResolvedAction::Probe { .. } => {
                    println!(
                        "  → Running probe: {}/{}…",
                        action.skill_id(),
                        action.action_id()
                    );
                    let probe_output = execute_probe_capture(paths, &writer, &action).await;
                    // Feed probe results back to Jack for analysis.
                    match analyze_probe_result(
                        paths,
                        &writer,
                        &outcome.response,
                        &probe_output,
                        &hkask_tool_names,
                    )
                    .await
                    {
                        Ok(analysis) => analysis,
                        Err(e) => {
                            debug!(error = %e, "probe analysis failed, using original response");
                            outcome.response.clone()
                        }
                    }
                }
                ResolvedAction::Intervention {
                    risk, needs_sudo, ..
                } => {
                    let sudo_tag = if *needs_sudo { " [needs sudo]" } else { "" };
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
                    outcome.response.clone()
                }
                ResolvedAction::HKaskTool { .. } => {
                    println!("  → Jack proposes hKask tool: {}.", action.action_id(),);
                    println!("  → Switch to `russell chat` to execute hKask tools interactively.");
                    println!();
                    outcome.response.clone()
                }
            },
            Err(e) => {
                println!("  → {e}");
                outcome.response.clone()
            }
        }
    } else {
        outcome.response.clone()
    };

    // Print the final response (Jack's analysis, possibly including probe findings).
    let response = final_response.trim_end();
    println!("{response}");
    println!();

    println!(
        "  [jack via {} · session {} · bundle {}]",
        outcome.backend,
        outcome.session_id,
        outcome.evidence_dir.display()
    );

    if let Some(sr) = outcome.skip_reason {
        let msg = match sr {
            russell_meta::help::SkipReason::OfflineFallback => {
                "  [offline fallback engaged — Ollama unreachable or LLM call failed]"
            }
            russell_meta::help::SkipReason::ThresholdSkip => {
                "  [below escalation threshold — rule-based summary returned]"
            }
        };
        println!("{msg}");
    }

    Ok(())
}

/// Execute a probe and capture output for analysis.
async fn execute_probe_capture(
    paths: &Paths,
    journal: &JournalWriter,
    action: &ResolvedAction,
) -> ProbeOutput {
    use russell_skills::dispatch::{Dispatcher, DryRun, StepType};
    use russell_skills::registry::RegistryCache;
    use std::time::Duration;

    let skill_dir = paths.skills().join(action.skill_id());
    let evidence_base = paths.evidence();
    let timeout = Duration::from_secs(30);

    let mut dispatcher = Dispatcher::new(&skill_dir);
    // Task 3.1: Load skill to get allowed_env_keys for capability attenuation.
    if let Ok(skill) = russell_skills::load_single(&skill_dir) {
        dispatcher.allowed_env_keys = skill.safety.allowed_env_keys.clone();
    }
    dispatcher.probe_timeout = timeout;
    dispatcher.dry_run = DryRun::Disabled;
    dispatcher.max_auto_risk = match action {
        ResolvedAction::Probe { max_auto_risk, .. } => *max_auto_risk,
        ResolvedAction::Intervention { max_auto_risk, .. } => *max_auto_risk,
        ResolvedAction::HKaskTool { .. } => russell_skills::RiskBand::None,
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

    // Update registry telemetry.
    let probe_success = result.as_ref().is_ok_and(|o| o.exit_code == Some(0));
    let probe_duration_ms = result
        .as_ref()
        .map(|o| o.duration.as_millis() as u64)
        .unwrap_or(0);
    let probe_error = result.as_ref().err().map(|e| e.to_string());
    let registry_path = paths.state.join("registry").join("local-cache.yaml");
    let _ = RegistryCache::with_update(&registry_path, |cache| {
        cache.record_execution(
            action.skill_id(),
            probe_success,
            probe_duration_ms,
            probe_error.as_deref(),
        );
    });

    match result {
        Ok(outcome) => ProbeOutput {
            success: outcome.exit_code == Some(0),
            stdout: outcome.stdout,
            stderr: outcome.stderr,
            exit_code: outcome.exit_code,
        },
        Err(e) => ProbeOutput {
            success: false,
            stdout: String::new(),
            stderr: format!("Dispatcher error: {e}"),
            exit_code: None,
        },
    }
}

/// Feed probe results back to Jack for analysis.
async fn analyze_probe_result(
    _paths: &Paths,
    _writer: &JournalWriter,
    _original_response: &str,
    probe_output: &ProbeOutput,
    _hkask_tool_names: &[(String, Option<String>)],
) -> Result<String> {
    use russell_meta::client::{ClientConfig, LlmClient, SoapPrompt};
    use russell_meta::oai_client;

    let cfg = ClientConfig::from_env();
    let client = match cfg.backend {
        russell_meta::client::Backend::Okapi => {
            let mut okapi_cfg = cfg.clone();
            if okapi_cfg.base_url.is_none() {
                okapi_cfg.base_url = Some("http://127.0.0.1:11435/v1".into());
            }
            if okapi_cfg.api_key.is_none() {
                okapi_cfg.api_key = Some("okapi".into());
            }
            oai_client::OkapiClient::new(&okapi_cfg).await?
        }
        russell_meta::client::Backend::Mock => {
            return Ok(format!(
                "[mock backend] Probe result: {}\nAnalysis would happen here.",
                probe_output.stdout.trim()
            ));
        }
        russell_meta::client::Backend::Offline => {
            return Ok(format!(
                "[offline] Probe returned: {}",
                probe_output.stdout.trim()
            ));
        }
    };

    let probe_summary = if probe_output.success {
        format!("Probe output:\n{}", probe_output.stdout.trim())
    } else {
        format!(
            "Probe failed with exit code {:?}.\nStdout: {}\nStderr: {}",
            probe_output.exit_code,
            probe_output.stdout.trim(),
            probe_output.stderr.trim()
        )
    };

    let response = client
        .chat(&SoapPrompt {
            system: "You are Jack, the Nurse. Analyze probe results and explain what they mean. \
                     You previously proposed running a probe. Here are the results.\n\n\
                     Analyze what this result means in the context of your previous analysis. \
                     What does this tell us about the system? What should we do next?\n\n\
                     If you identify another probe to run, propose it with ACTION: syntax."
                .into(),
            subjective: "Probe results".into(),
            objective: probe_summary.clone(),
            rendered: format!("Probe results:\n\n{}", probe_summary),
            temperature: None,
            max_tokens: None,
        })
        .await?;

    Ok(response.content)
}

/// Output from a probe execution.
#[derive(Debug, Clone)]
struct ProbeOutput {
    success: bool,
    stdout: String,
    stderr: String,
    exit_code: Option<i32>,
}

/// Collect hKask MCP tool infos (name, risk, input_schema) for the SOAP prompt
/// and action resolver (ADR-0025 §7).
/// Returns an empty list on any failure (graceful degradation per ADR-0025 §5).
/// Falls back to the disk cache if hKask is unreachable.
async fn collect_hkask_tool_infos(paths: &Paths) -> Vec<HKaskToolInfo> {
    let hkask_config = HKaskMcpConfig::from_env();
    if hkask_config.validate().is_err() {
        return vec![];
    }

    let mut client = match HKaskMcpClient::new(hkask_config.clone()) {
        Ok(c) => c,
        Err(e) => {
            debug!(error = %e, "hKask MCP client construction failed");
            return load_cached_tool_infos(paths);
        }
    };

    if client.connect().await.is_err() {
        debug!("hKask MCP connect failed — tools unavailable this session");
        return load_cached_tool_infos(paths);
    }

    let cache_path = paths.memory_dir().join("hkask-tools.cache.json");
    let mut registry = ToolRegistry::new(hkask_config.tool_ttl);
    let _ = registry.load_from_disk(&cache_path);

    if let Err(e) = registry.refresh(&client).await {
        debug!(error = %e, "hKask tool registry refresh failed");
        // Return cached if refresh failed but we have cached data.
        if !registry.is_empty() {
            return registry_to_hkask_infos(&registry);
        }
        return vec![];
    }

    // Persist fresh tools to disk.
    let _ = registry.save_to_disk(&cache_path);

    registry_to_hkask_infos(&registry)
}

/// Load cached tool infos from the disk cache as a fallback.
fn load_cached_tool_infos(paths: &Paths) -> Vec<HKaskToolInfo> {
    let cache_path = paths.memory_dir().join("hkask-tools.cache.json");
    let mut registry = ToolRegistry::new(HKaskMcpConfig::from_env().tool_ttl);
    if registry.load_from_disk(&cache_path).is_ok() && !registry.is_empty() {
        debug!(
            count = registry.tool_count(),
            "loaded hKask tools from disk cache"
        );
        return registry_to_hkask_infos(&registry);
    }
    vec![]
}

/// Convert a [`ToolRegistry`] to a [`HKaskToolInfo`] list, preserving
/// `input_schema` from the cached [`McpToolDefinition`].
fn registry_to_hkask_infos(registry: &ToolRegistry) -> Vec<HKaskToolInfo> {
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
            HKaskToolInfo {
                name: t.name.clone(),
                risk_band,
                input_schema: t.input_schema.clone(),
            }
        })
        .collect()
}

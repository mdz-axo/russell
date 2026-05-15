// SPDX-License-Identifier: MIT OR Apache-2.0
//! Kask MCP tool execution for the chat REPL (ADR-0025 §7).
//!
//! Handles calling remote Kask MCP tools, writing IDRS-structured
//! evidence bundles, and formatting results for LLM context injection.

use russell_core::event::{Event, Severity};
use russell_core::journal::JournalWriter;
use russell_core::paths::Paths;
use russell_doctor::action::{KaskToolInfo, ResolvedAction};
use russell_mcp::client::KaskMcpClient;
use russell_mcp::registry::ToolRegistry;
use russell_skills::RiskBand;
use tracing::{debug, warn};

use super::execute::journal_chat_turn;

/// Build a list of [`KaskToolInfo`] from the registry for the action resolver.
pub fn build_kask_tool_infos(registry: &ToolRegistry) -> Vec<KaskToolInfo> {
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

/// Execute a Kask MCP tool call. Returns a formatted result string
/// for injection into the LLM conversation history.
///
/// Writes an IDRS-structured evidence bundle to
/// `evidence/kask/<tool_name>/<ISO-ts>/` with `result.txt` and
/// `event.json`, matching the local skill dispatcher's evidence format.
///
/// If the tool has required fields declared in its `inputSchema` and
/// the LLM did not provide arguments, warns the operator and cancels.
pub async fn execute_kask_tool(
    journal: &JournalWriter,
    kask_client: &Option<KaskMcpClient>,
    action: &ResolvedAction,
    session_id: &str,
    model: &str,
    paths: &Paths,
) -> Option<String> {
    use std::time::Duration;

    let tool_name = action.action_id().to_string();
    let risk = action.risk_band();

    // Extract arguments from the resolved action.
    let (arguments, required_fields) = match action {
        ResolvedAction::KaskTool {
            arguments,
            required_fields,
            ..
        } => (arguments, required_fields),
        _ => {
            println!("  → Internal error: non-Kask action routed to execute_kask_tool.");
            return None;
        }
    };

    // If required fields exist but no arguments were provided, warn and cancel.
    if !required_fields.is_empty() && arguments.is_none() {
        println!("  → kask/{tool_name} requires arguments: {required_fields:?}",);
        println!("  → Jack didn't provide them. Ask Jack to include arguments in the response.");
        println!("  → Format: Arguments: {{\"field\": \"value\"}}");
        return None;
    }

    let client = match kask_client {
        Some(c) => c,
        None => {
            println!("  → Kask MCP client not connected. Cannot execute tool.");
            return None;
        }
    };

    println!("  → Calling kask/{tool_name}…");
    if arguments.is_some() {
        debug!(tool = %tool_name, "kask tool call with arguments");
    }
    let started = std::time::Instant::now();

    // Per-tool timeout: probes get 30s, interventions get 120s.
    let timeout = if risk == RiskBand::None {
        Duration::from_secs(30)
    } else {
        Duration::from_secs(120)
    };

    let tool_result =
        tokio::time::timeout(timeout, client.call_tool(&tool_name, arguments.clone())).await;

    let duration = started.elapsed();

    let (success, text, is_error, timed_out) = match tool_result {
        Ok(Ok(result)) => {
            let text: String = result
                .content
                .iter()
                .filter_map(|c| c.text.as_deref())
                .collect::<Vec<_>>()
                .join("\n");
            (true, text, result.is_error, false)
        }
        Ok(Err(e)) => {
            let error_msg = format!("{e}");
            (false, error_msg, true, false)
        }
        Err(_elapsed) => {
            let msg = format!("timed out after {timeout:?}");
            (false, msg, false, true)
        }
    };

    let truncated = if text.len() > 3000 {
        format!("{}… (truncated)", &text[..3000])
    } else {
        text.clone()
    };

    if timed_out {
        println!("  → kask/{tool_name} timed out after {timeout:?}.");
    } else if is_error {
        println!("  → kask/{tool_name} returned an error:");
        println!("  {truncated}");
    } else {
        println!("  → kask/{tool_name} complete.");
        if !truncated.is_empty() {
            for line in truncated.lines().take(10) {
                println!("  {line}");
            }
            let line_count = truncated.lines().count();
            if line_count > 10 {
                println!("  … ({} more lines)", line_count - 10);
            }
        }
    }

    // Write evidence bundle (IDRS-S) matching local skill dispatch format.
    write_kask_evidence(paths, &tool_name, risk, &text, success, timed_out, duration);

    // Journal as a skill-level event (not just a chat turn).
    let action_str = if success {
        "kask_tool"
    } else {
        "kask_tool_failure"
    };
    let severity = if success && !is_error {
        Severity::Info
    } else {
        Severity::Warn
    };
    let mut ev = Event::new(action_str, severity);
    ev.tier = Some("skill".into());
    ev.module = Some(format!("kask/{tool_name}"));
    ev.duration_ms = Some(duration.as_millis() as u64);
    ev.summary = Some(format!(
        "kask/{tool_name}: success={success}, is_error={is_error}, timed_out={timed_out}"
    ));
    ev.outputs.insert("risk".into(), risk.as_str().into());
    ev.outputs.insert("step_type".into(), "kask_tool".into());
    ev.outputs.insert("success".into(), success.into());
    ev.outputs.insert("is_error".into(), is_error.into());
    ev.outputs.insert("timed_out".into(), timed_out.into());
    ev.outputs.insert("content_len".into(), text.len().into());
    if let Err(e) = journal.append(&ev) {
        warn!(error = %e, "failed to journal kask tool event");
    }

    // Also journal as chat turn for session history.
    journal_chat_turn(
        journal,
        session_id,
        model,
        &format!("/kask-tool {tool_name}"),
        &format!(
            "kask/{tool_name}: success={success}, is_error={is_error}, timed_out={timed_out}, content_len={}",
            text.len()
        ),
    );

    // Build result for LLM context injection.
    let status = if timed_out {
        "timeout"
    } else if is_error {
        "error"
    } else {
        "ok"
    };
    Some(format!(
        "[kask tool result: {tool_name}, status={status}]\n{truncated}"
    ))
}

/// Write an IDRS-structured evidence bundle for a Kask MCP tool call.
///
/// Produces `evidence/kask/<tool_name>/<ISO-ts>/` with `result.txt`
/// and `event.json`, mirroring the local skill dispatcher's bundle format.
fn write_kask_evidence(
    paths: &Paths,
    tool_name: &str,
    risk: RiskBand,
    result_text: &str,
    success: bool,
    timed_out: bool,
    duration: std::time::Duration,
) {
    let ts = russell_core::time::now_rfc3339().replace(':', "-");
    let evidence_dir = paths.evidence().join("kask").join(tool_name).join(&ts);

    if let Err(e) = std::fs::create_dir_all(&evidence_dir) {
        warn!(dir = %evidence_dir.display(), error = %e, "failed to create kask evidence dir");
        return;
    }

    // Write result text (equivalent to stdout.txt for local skills).
    if let Err(e) = std::fs::write(evidence_dir.join("result.txt"), result_text) {
        warn!(dir = %evidence_dir.display(), error = %e, "failed to write kask result.txt");
    }

    // Write structured event record.
    let mut ev = Event::new(
        "kask_tool",
        if success {
            Severity::Info
        } else {
            Severity::Warn
        },
    );
    ev.tier = Some("skill".into());
    ev.module = Some(format!("kask/{tool_name}"));
    ev.duration_ms = Some(duration.as_millis() as u64);
    ev.summary = Some(format!(
        "kask/{tool_name}: success={success}, timed_out={timed_out}"
    ));
    ev.outputs.insert("risk".into(), risk.as_str().into());
    ev.outputs.insert("step_type".into(), "kask_tool".into());
    ev.outputs.insert("success".into(), success.into());
    ev.outputs.insert("timed_out".into(), timed_out.into());
    ev.evidence_ref = Some(evidence_dir.display().to_string());

    match serde_json::to_string_pretty(&ev) {
        Ok(json) => {
            if let Err(e) = std::fs::write(evidence_dir.join("event.json"), json) {
                warn!(dir = %evidence_dir.display(), error = %e, "failed to write kask event.json");
            }
        }
        Err(e) => {
            warn!(error = %e, "failed to serialize kask event json");
        }
    }
}

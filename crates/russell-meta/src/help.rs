// SPDX-License-Identifier: MIT OR Apache-2.0
//! `run_help` — Nurse pipeline via LLM inference.
//!
//! Russell collects telemetry and sends it to an LLM backend for inference.
//! Falls back to an offline summary when no backend is reachable.

use std::path::PathBuf;
use std::time::Duration;

use serde::Serialize;
use time::OffsetDateTime;
use tracing::{info, warn};

use russell_core::config::RuntimeConfig;
use russell_core::journal::{HelpSessionInput, HelpSessionStatus, JournalWriter};
use russell_core::paths::Paths;

use crate::error::{DoctorError, Result};

/// Outcome from a `run_help` call.
#[derive(Debug, Clone, Serialize)]
pub struct HelpOutcome {
    /// Unique session identifier.
    pub session_id: String,
    /// Backend used ("okapi" or "offline").
    pub backend: &'static str,
    /// Path to the evidence bundle directory.
    pub evidence_dir: PathBuf,
    /// LLM response text or offline fallback summary.
    pub response: String,
    /// Reason the LLM call was skipped (if applicable).
    pub skip_reason: Option<SkipReason>,
}

/// Reason the LLM inference was skipped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum SkipReason {
    /// Remote backend was unreachable; offline fallback was used.
    OfflineFallback,
    /// No crit/alert events; threshold gate prevented escalation.
    ThresholdSkip,
}

#[derive(Debug, Clone, serde::Serialize)]
struct ObjectiveData {
    severity_counts: SeverityCounts,
    recent_events: Vec<EventRecord>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct SeverityCounts {
    crit: u64,
    alert: u64,
    warn: u64,
    info: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
struct EventRecord {
    probe: String,
    severity: String,
    message: String,
    ts: String,
}

/// Run the Nurse pipeline using the Okapi endpoint from [`RuntimeConfig`].
///
/// Loads configuration from environment variables (see [`RuntimeConfig::from_env`]).
pub async fn run_help(
    paths: &Paths,
    writer: &JournalWriter,
    note: Option<&str>,
) -> Result<HelpOutcome> {
    let config = RuntimeConfig::from_env();
    run_help_with_endpoint(paths, writer, note, &config.okapi_endpoint).await
}

/// Run the Nurse pipeline with a configurable inference endpoint.
///
/// Gathers objective telemetry, checks the threshold gate, calls the
/// LLM backend for inference, and journals the help session. Falls back to an
/// offline summary if the backend is unreachable or the threshold gate skips.
pub async fn run_help_with_endpoint(
    paths: &Paths,
    writer: &JournalWriter,
    note: Option<&str>,
    endpoint: &str,
) -> Result<HelpOutcome> {
    let session_id = ulid::Ulid::new().to_string();
    let evidence_dir = paths.evidence().join("help").join(&session_id);
    std::fs::create_dir_all(&evidence_dir).map_err(|e| DoctorError::io(&evidence_dir, e))?;

    let objective = gather_objective(writer).await;

    // Verify recent evidence bundles (Task 13).
    verify_recent_evidence(&paths.evidence(), writer);

    if let Some(reason) = check_threshold(&objective.severity_counts) {
        let response = fallback_summary(&objective);
        let ts = OffsetDateTime::now_utc();
        let input = HelpSessionInput {
            id: &session_id,
            ts_unix: ts.unix_timestamp(),
            ts: &ts.to_string(),
            backend: "offline",
            model: None,
            note,
            prompt_chars: 0,
            response_chars: response.len() as i64,
            latency_ms: None,
            status: HelpSessionStatus::ThresholdSkip,
            error_kind: None,
            evidence_ref: &evidence_dir.to_string_lossy(),
        };
        writer.append_help_session(&input)?;
        return Ok(HelpOutcome {
            session_id,
            backend: "offline",
            evidence_dir,
            response,
            skip_reason: Some(reason),
        });
    }

    // Attempt inference via the configured backend.
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| DoctorError::Config(format!("HTTP client: {e}")))?;

    let subjective = note.unwrap_or("System health check requested");
    let body = serde_json::json!({
        "subjective": subjective,
        "objective": objective,
    });

    let response = match client
        .post(endpoint)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => resp,
        Ok(resp) => {
            let status = resp.status();
            warn!(status = %status, "inference endpoint returned non-success");
            let response = fallback_summary(&gather_objective(writer).await);
            let ts = OffsetDateTime::now_utc();
            let input = HelpSessionInput {
                id: &session_id,
                ts_unix: ts.unix_timestamp(),
                ts: &ts.to_string(),
                backend: "offline",
                model: None,
                note,
                prompt_chars: 0,
                response_chars: response.len() as i64,
                latency_ms: None,
                status: HelpSessionStatus::Fallback,
                error_kind: None,
                evidence_ref: &evidence_dir.to_string_lossy(),
            };
            writer.append_help_session(&input)?;
            return Ok(HelpOutcome {
                session_id,
                backend: "offline",
                evidence_dir,
                response,
                skip_reason: Some(SkipReason::OfflineFallback),
            });
        }
        Err(e) => {
            warn!(error = %e, "inference endpoint unreachable; using offline fallback");
            let response = fallback_summary(&gather_objective(writer).await);
            let ts = OffsetDateTime::now_utc();
            let input = HelpSessionInput {
                id: &session_id,
                ts_unix: ts.unix_timestamp(),
                ts: &ts.to_string(),
                backend: "offline",
                model: None,
                note,
                prompt_chars: 0,
                response_chars: response.len() as i64,
                latency_ms: None,
                status: HelpSessionStatus::Fallback,
                error_kind: None,
                evidence_ref: &evidence_dir.to_string_lossy(),
            };
            writer.append_help_session(&input)?;
            return Ok(HelpOutcome {
                session_id,
                backend: "offline",
                evidence_dir,
                response,
                skip_reason: Some(SkipReason::OfflineFallback),
            });
        }
    };

    let infer_response: serde_json::Value = response
        .json()
        .await
        .map_err(|e| DoctorError::Config(format!("failed to parse inference response: {e}")))?;

    let text = infer_response
        .get("response")
        .and_then(|v| v.as_str())
        .unwrap_or("No response from inference backend")
        .to_string();

    let model = infer_response
        .get("model")
        .and_then(|v| v.as_str())
        .map(String::from);

    let ts = OffsetDateTime::now_utc();
    let input = HelpSessionInput {
        id: &session_id,
        ts_unix: ts.unix_timestamp(),
        ts: &ts.to_string(),
        backend: "okapi",
        model: model.as_deref(),
        note,
        prompt_chars: 0,
        response_chars: text.len() as i64,
        latency_ms: None,
        status: HelpSessionStatus::Ok,
        error_kind: None,
        evidence_ref: &evidence_dir.to_string_lossy(),
    };
    writer.append_help_session(&input)?;

    let evidence_path = evidence_dir.join("response.json");
    std::fs::write(
        &evidence_path,
        serde_json::to_string_pretty(&infer_response)?,
    )
    .map_err(|e| DoctorError::io(&evidence_path, e))?;

    info!(session_id, "help session complete");

    Ok(HelpOutcome {
        session_id,
        backend: "okapi",
        evidence_dir,
        response: text,
        skip_reason: None,
    })
}

async fn gather_objective(writer: &JournalWriter) -> ObjectiveData {
    let reader = writer.reader();
    let now = OffsetDateTime::now_utc().unix_timestamp();
    let since = now - 86400; // Last 24 hours

    let severity_counts =
        reader
            .severity_counts(since, now)
            .unwrap_or(russell_core::journal::SeverityCounts {
                crit: 0,
                alert: 0,
                warn: 0,
                info: 0,
            });

    let events = reader.recent(20).unwrap_or_default();

    ObjectiveData {
        severity_counts: SeverityCounts {
            crit: severity_counts.crit as u64,
            alert: severity_counts.alert as u64,
            warn: severity_counts.warn as u64,
            info: severity_counts.info as u64,
        },
        recent_events: events
            .into_iter()
            .map(|e| EventRecord {
                probe: format!("{:?}", e.scope),
                severity: format!("{:?}", e.severity),
                message: e.summary.unwrap_or_default(),
                ts: e.ts,
            })
            .collect(),
    }
}

fn check_threshold(counts: &SeverityCounts) -> Option<SkipReason> {
    if counts.crit > 0 || counts.alert > 0 {
        None
    } else {
        Some(SkipReason::ThresholdSkip)
    }
}

fn fallback_summary(objective: &ObjectiveData) -> String {
    let mut lines = Vec::new();
    lines.push("## Russell Health Summary (Offline)".to_string());
    lines.push(String::new());
    lines.push(format!(
        "Severity: {} crit, {} alert, {} warn, {} info",
        objective.severity_counts.crit,
        objective.severity_counts.alert,
        objective.severity_counts.warn,
        objective.severity_counts.info
    ));
    lines.push(String::new());
    lines.push("Recent events:".to_string());
    for event in objective.recent_events.iter().take(10) {
        lines.push(format!(
            "  - [{}] {}: {}",
            event.severity, event.probe, event.message
        ));
    }
    lines.join("\n")
}

/// Verify the 5 most recent evidence bundles by re-hashing files
/// and comparing against their manifest.json seals.
///
/// Logs a warning for each bundle that fails verification.
/// Failures are non-fatal — they don't block the Nurse pipeline.
fn verify_recent_evidence(evidence_base: &std::path::Path, _writer: &JournalWriter) {
    let skills_dir = evidence_base.join("skills");
    if !skills_dir.is_dir() {
        return;
    }

    let mut checked = 0u32;
    if let Ok(entries) = std::fs::read_dir(&skills_dir) {
        for entry in entries.flatten() {
            if checked >= 5 {
                break;
            }
            let skill_dir = entry.path();
            if !skill_dir.is_dir() {
                continue;
            }
            if let Ok(steps) = std::fs::read_dir(&skill_dir) {
                for step in steps.flatten() {
                    if checked >= 5 {
                        break;
                    }
                    let step_dir = step.path();
                    if !step_dir.is_dir() {
                        continue;
                    }
                    if let Ok(bundles) = std::fs::read_dir(&step_dir) {
                        for bundle in bundles.flatten() {
                            if checked >= 5 {
                                break;
                            }
                            let bundle_dir = bundle.path();
                            if !bundle_dir.join("manifest.json").exists() {
                                continue;
                            }
                            checked += 1;
                            match russell_skills::dispatch::verify_evidence_bundle(&bundle_dir) {
                                Ok(true) => {}
                                Ok(false) => {
                                    warn!(
                                        dir = %bundle_dir.display(),
                                        "evidence bundle integrity check FAILED"
                                    );
                                }
                                Err(e) => {
                                    warn!(
                                        dir = %bundle_dir.display(),
                                        error = %e,
                                        "evidence bundle integrity check error"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

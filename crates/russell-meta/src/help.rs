// SPDX-License-Identifier: MIT OR Apache-2.0
//! `run_help` — Nurse pipeline via hKask.
//!
//! Russell collects telemetry and sends it to hKask for LLM inference.
//! hKask handles prompt composition, LLM calls, and returns the response.

use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use ulid::Ulid;
use time::OffsetDateTime;

use russell_core::journal::{HelpSessionInput, HelpSessionStatus, JournalWriter};
use russell_core::paths::Paths;

use crate::error::{DoctorError, Result};

/// Outcome from a `run_help` call.
#[derive(Debug, Clone, Serialize)]
pub struct HelpOutcome {
    pub session_id: String,
    pub backend: &'static str,
    pub evidence_dir: PathBuf,
    pub response: String,
    pub skip_reason: Option<SkipReason>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum SkipReason {
    OfflineFallback,
    ThresholdSkip,
}

#[derive(Serialize)]
struct HKaskInferRequest {
    subjective: Option<String>,
    objective: ObjectiveData,
    assessment: String,
    plan: String,
}

#[derive(Serialize)]
struct ObjectiveData {
    samples: Vec<SampleRecord>,
    severity_counts: SeverityCounts,
    recent_events: Vec<EventRecord>,
}

#[derive(Serialize)]
struct SampleRecord {
    probe: String,
    value: f64,
    ts: String,
}

#[derive(Serialize)]
struct SeverityCounts {
    crit: u64,
    alert: u64,
    warn: u64,
    info: u64,
}

#[derive(Serialize)]
struct EventRecord {
    probe: String,
    severity: String,
    message: String,
    ts: String,
}

#[derive(Deserialize, Serialize)]
struct HKaskInferResponse {
    response: String,
    model: String,
    latency_ms: u64,
}

const HKASK_ENDPOINT: &str = "http://127.0.0.1:8080/api/llm/infer";

pub async fn run_help(
    paths: &Paths,
    writer: &JournalWriter,
    note: Option<&str>,
    _hkask_tool_names: &[(String, Option<String>)],
) -> Result<HelpOutcome> {
    run_help_with_endpoint(paths, writer, note, HKASK_ENDPOINT).await
}

pub async fn run_help_with_endpoint(
    paths: &Paths,
    writer: &JournalWriter,
    note: Option<&str>,
    endpoint: &str,
) -> Result<HelpOutcome> {
    let session_id = Ulid::new().to_string();
    let evidence_dir = paths.evidence().join("help").join(&session_id);
    std::fs::create_dir_all(&evidence_dir).map_err(|e| DoctorError::io(&evidence_dir, e))?;

    let objective = gather_objective(writer).await;

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
            evidence_ref: &evidence_dir.to_string_lossy(),
            status: HelpSessionStatus::ThresholdSkip,
            error_kind: None,
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

    let request = HKaskInferRequest {
        subjective: note.map(String::from),
        objective,
        assessment: String::new(),
        plan: String::new(),
    };

    let response = match call_hkask(endpoint, &request).await {
        Ok(r) => r,
        Err(e) => {
            warn!(error = %e, "hKask unreachable; using offline fallback");
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
                evidence_ref: &evidence_dir.to_string_lossy(),
            status: HelpSessionStatus::Fallback,
                error_kind: None,
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

    let ts = OffsetDateTime::now_utc();
    let input = HelpSessionInput {
        id: &session_id,
        ts_unix: ts.unix_timestamp(),
        ts: &ts.to_string(),
        backend: "hkask",
        model: Some(&response.model),
        note,
        prompt_chars: 0,
        response_chars: response.response.len() as i64,
        latency_ms: Some(response.latency_ms as i64),
        evidence_ref: &evidence_dir.to_string_lossy(),
            status: HelpSessionStatus::Ok,
        error_kind: None,
    };
    writer.append_help_session(&input)?;

    let evidence_path = evidence_dir.join("response.json");
    std::fs::write(&evidence_path, serde_json::to_string_pretty(&response)?)?;

    info!(session_id, model = %response.model, "help session complete");

    Ok(HelpOutcome {
        session_id,
        backend: "hkask",
        evidence_dir,
        response: response.response,
        skip_reason: None,
    })
}

async fn gather_objective(writer: &JournalWriter) -> ObjectiveData {
    let samples = writer.get_recent_samples(50).unwrap_or_default();
    let severity_counts = writer.get_severity_counts().unwrap_or_default();
    let events = writer.get_recent_events(20).unwrap_or_default();

    ObjectiveData {
        samples: samples.into_iter().map(|s| SampleRecord { probe: s.probe, value: s.value, ts: s.ts }).collect(),
        severity_counts: SeverityCounts { crit: severity_counts.crit, alert: severity_counts.alert, warn: severity_counts.warn, info: severity_counts.info },
        recent_events: events.into_iter().map(|e| EventRecord { probe: e.probe, severity: format!("{:?}", e.severity), message: e.message, ts: e.ts }).collect(),
    }
}

fn check_threshold(counts: &SeverityCounts) -> Option<SkipReason> {
    if counts.crit > 0 || counts.alert > 0 { None } else { Some(SkipReason::ThresholdSkip) }
}

fn fallback_summary(objective: &ObjectiveData) -> String {
    let mut lines = Vec::new();
    lines.push("## Russell Health Summary (Offline)".to_string());
    lines.push(String::new());
    lines.push(format!("Severity: {} crit, {} alert, {} warn, {} info",
        objective.severity_counts.crit, objective.severity_counts.alert,
        objective.severity_counts.warn, objective.severity_counts.info));
    lines.push(String::new());
    lines.push("Recent probes:".to_string());
    for sample in objective.samples.iter().take(10) {
        lines.push(format!("  - {}: {:.2}", sample.probe, sample.value));
    }
    lines.join("\n")
}

async fn call_hkask(endpoint: &str, request: &HKaskInferRequest) -> Result<HKaskInferResponse> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| DoctorError::Config(format!("HTTP client: {e}")))?;

    let response = client.post(endpoint).json(request).send().await
        .map_err(|e| DoctorError::Config(format!("hKask request: {e}")))?;

    if !response.status().is_success() {
        return Err(DoctorError::Config(format!("hKask returned {}", response.status())));
    }

    response.json().await.map_err(|e| DoctorError::Config(format!("hKask response: {e}")))
}

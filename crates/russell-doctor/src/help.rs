// SPDX-License-Identifier: MIT OR Apache-2.0
//! `run_help` orchestrator — the Nurse pipeline.
//!
//! Implements the three-stage Nurse flow from
//! [`docs/architecture/CAPABILITY_GRAPH.md`](../../../docs/architecture/CAPABILITY_GRAPH.md) §1.2:
//!
//! 1. **Compose** — builds a SOAP bundle (Subjective, Objective,
//!    Assessment, Plan) from journal state, machine profile,
//!    loaded skills, and Kask tools. Augments with operator
//!    PERSONA.md / USER.md files per
//!    [ADR-0022](../../../docs/adr/0022-markdown-memory-layer.md).
//! 2. **Dispatch** — threshold-gated LLM call per
//!    [ADR-0020](../../../docs/adr/0020-threshold-gated-llm-escalation.md).
//!    Falls back to rule-based offline summariser on failure.
//! 3. **Persist** — writes request/response/transcript to evidence
//!    bundle, appends `harness.event.v1` to journal, records
//!    help session row, and generates daily memory note.
//!
//! The Nurse never emits shell (JR-3, ADR-0008). Action IDs are
//! selected from loaded manifests and rejected by a poka-yoke
//! dispatcher if unknown.

use std::path::PathBuf;

use serde::Serialize;
use serde_json::json;
use tracing::{info, warn};
use ulid::Ulid;

use russell_core::event::{Event, Severity};
use russell_core::journal::{HelpSessionInput, HelpSessionStatus, JournalWriter};
use russell_core::paths::Paths;

use crate::client::{Backend, ClientConfig, LlmClient, SoapPrompt};
use crate::error::{DoctorError, Result};
use crate::{fallback, mock, oai_client, prompt};

/// Why the LLM was not called.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum SkipReason {
    /// Network/key unavailable; offline fallback engaged.
    OfflineFallback,
    /// Severity below threshold; rule-based summary returned.
    ThresholdSkip,
}

/// Outcome from a `run_help` call.
#[derive(Debug, Clone, Serialize)]
pub struct HelpOutcome {
    /// Session ID (ULID).
    pub session_id: String,
    /// Backend used — may be `offline` if fallback kicked in.
    pub backend: &'static str,
    /// Path to the evidence bundle on disk.
    pub evidence_dir: PathBuf,
    /// The response text Jack printed.
    pub response: String,
    /// Why the LLM was skipped; `None` if the LLM was called.
    pub skip_reason: Option<SkipReason>,
}

/// Run the Nurse flow end to end: compose SOAP, call LLM (or fall
/// back), journal the session, return a print-ready response.
///
/// `paths` and `writer` come from the CLI. The CLI is the only caller.
///
/// `kask_tool_names` is a list of (tool_name, risk_band) pairs from
/// the Kask MCP tool registry (ADR-0025). Empty if Kask is unreachable.
///
/// # Errors
///
/// Returns `DoctorError` if the filesystem write or journal write fails.
/// Provider errors are *caught* — the fallback handles them and
/// the function returns success with `skip_reason` set.
pub async fn run_help(
    paths: &Paths,
    writer: &JournalWriter,
    note: Option<&str>,
    kask_tool_names: &[(String, Option<String>)],
) -> Result<HelpOutcome> {
    let cfg = ClientConfig::from_env();
    run_help_with_config(paths, writer, note, cfg, kask_tool_names).await
}

/// Dispatch result from calling the LLM backend.
struct DispatchResult {
    backend_label: &'static str,
    response: Option<String>,
    model: Option<String>,
    latency_ms: Option<i64>,
    error_kind: Option<String>,
    skip_reason: Option<SkipReason>,
}

// ---------------------------------------------------------------------------
// Stage 1: SOAP composition + augmentation
// ---------------------------------------------------------------------------

/// Compose the SOAP prompt from journal state, profile, skills, and
/// operator identity files. Also writes the raw SOAP to the evidence
/// directory.
fn compose_and_augment_soap(
    paths: &Paths,
    writer: &JournalWriter,
    note: Option<&str>,
    evidence_dir: &std::path::Path,
    kask_tool_names: &[(String, Option<String>)],
) -> Result<SoapPrompt> {
    let profile_path = paths.profile();
    let profile = if profile_path.exists() {
        russell_core::Profile::load(&profile_path).ok()
    } else {
        None
    };

    let loaded_skills = russell_skills::load_all(&paths.skills()).unwrap_or_default();
    tracing::debug!(
        count = loaded_skills.len(),
        kask_tools = kask_tool_names.len(),
        "loaded skills and kask tools for help session"
    );

    let soap = prompt::compose_with_kask(
        &writer.reader(),
        profile.as_ref(),
        note,
        &loaded_skills,
        &paths.skills(),
        kask_tool_names,
    )?;

    // ADR-0022: augment the system prompt with operator identity files.
    let soap = augment_system_prompt(paths, soap);

    let soap_path = evidence_dir.join("soap.md");
    std::fs::write(&soap_path, &soap.rendered).map_err(|e| DoctorError::io(&soap_path, e))?;

    Ok(soap)
}

// ---------------------------------------------------------------------------
// Stage 2: Threshold gate + backend dispatch
// ---------------------------------------------------------------------------

/// Check the escalation threshold and dispatch to the LLM backend.
/// Falls back to the offline summariser if the threshold is not met
/// or the backend call fails.
async fn dispatch_backend(
    writer: &JournalWriter,
    cfg: &ClientConfig,
    soap: &SoapPrompt,
    escalate: bool,
) -> Result<DispatchResult> {
    // Threshold gate.
    if !escalate {
        let text = fallback::summarise(&writer.reader(), None)?;
        return Ok(DispatchResult {
            backend_label: "offline",
            response: Some(text),
            model: None,
            latency_ms: None,
            error_kind: None,
            skip_reason: Some(SkipReason::ThresholdSkip),
        });
    }

    // Attempt the configured backend. Any failure → fall back to offline.
    let (backend_label, maybe_response, error_kind, skip_reason) = match cfg.backend {
        Backend::Okapi => {
            let mut okapi_cfg = cfg.clone();
            if okapi_cfg.base_url.is_none() {
                okapi_cfg.base_url = Some("http://127.0.0.1:11435/v1".into());
            }
            if okapi_cfg.api_key.is_none() {
                okapi_cfg.api_key = Some("okapi".into());
            }
            let base = okapi_cfg
                .base_url
                .as_deref()
                .unwrap_or(crate::health::DEFAULT_BASE_URL);
            crate::health::ensure_ready(base).await;

            let client = oai_client::OkapiClient::new(&okapi_cfg).await?;
            match client.chat(soap).await {
                Ok(resp) => ("okapi", Some(resp), None, None),
                Err(e) => {
                    warn!(error = %e, "okapi call failed — falling back");
                    (
                        "okapi",
                        None,
                        Some(error_kind_of(&e)),
                        Some(SkipReason::OfflineFallback),
                    )
                }
            }
        }
        Backend::Mock => {
            let client = mock::MockClient::jack_default();
            match client.chat(soap).await {
                Ok(resp) => ("mock", Some(resp), None, None),
                Err(e) => (
                    "mock",
                    None,
                    Some(error_kind_of(&e)),
                    Some(SkipReason::OfflineFallback),
                ),
            }
        }
        Backend::Offline => ("offline", None, None, Some(SkipReason::OfflineFallback)),
    };

    let (response_text, latency_ms, model) = match maybe_response {
        Some(resp) => (
            resp.content.clone(),
            Some(resp.latency_ms as i64),
            resp.model,
        ),
        None => {
            let text = fallback::summarise(&writer.reader(), None)?;
            (text, None, None)
        }
    };

    Ok(DispatchResult {
        backend_label,
        response: Some(response_text),
        model,
        latency_ms,
        error_kind,
        skip_reason,
    })
}

// ---------------------------------------------------------------------------
// Stage 3: Evidence persistence + journaling
// ---------------------------------------------------------------------------

/// Write evidence artefacts, journal the event, and insert the
/// help-session row. Returns the final `HelpOutcome`.
#[allow(clippy::too_many_arguments)]
fn persist_session(
    paths: &Paths,
    writer: &JournalWriter,
    session_id: &str,
    ts_unix: i64,
    ts: &str,
    evidence_dir: &std::path::Path,
    soap: &SoapPrompt,
    dispatch: &DispatchResult,
    cfg: &ClientConfig,
    note: Option<&str>,
) -> Result<HelpOutcome> {
    let response_text = dispatch.response.as_deref().unwrap_or("");
    let backend_used = dispatch.backend_label;
    let skip_reason = dispatch.skip_reason;
    let status: HelpSessionStatus = if let Some(sr) = skip_reason {
        match sr {
            SkipReason::OfflineFallback => HelpSessionStatus::Fallback,
            SkipReason::ThresholdSkip => HelpSessionStatus::ThresholdSkip,
        }
    } else {
        HelpSessionStatus::Ok
    };
    let status_str = status.as_str();

    // Evidence files.
    let request_path = evidence_dir.join("request.json");
    let request_rec = json!({
        "backend": backend_used,
        "model": &cfg.model,
        "base_url": &cfg.base_url,
        "note": note,
        "soap_chars": soap.rendered.len(),
    });
    std::fs::write(&request_path, serde_json::to_vec_pretty(&request_rec)?)
        .map_err(|e| DoctorError::io(&request_path, e))?;

    let response_path = evidence_dir.join("response.json");
    let response_rec = json!({
        "status": status_str,
        "error_kind": dispatch.error_kind,
        "latency_ms": dispatch.latency_ms,
        "model": dispatch.model,
        "content_chars": response_text.len(),
    });
    std::fs::write(&response_path, serde_json::to_vec_pretty(&response_rec)?)
        .map_err(|e| DoctorError::io(&response_path, e))?;

    let transcript_path = evidence_dir.join("transcript.jsonl");
    let line = json!({
        "schema": "harness.llm-transcript.v1",
        "ts": ts,
        "backend": backend_used,
        "model": &cfg.model,
        "fell_back": skip_reason.is_some(),
        "skip_reason": skip_reason.map(|s| match s {
            SkipReason::OfflineFallback => "offline_fallback",
            SkipReason::ThresholdSkip => "threshold_skip",
        }),
        "prompt_chars": soap.rendered.len(),
        "response_chars": response_text.len(),
        "response": response_text,
    });
    std::fs::write(&transcript_path, format!("{line}\n"))
        .map_err(|e| DoctorError::io(&transcript_path, e))?;

    // Event journal entry.
    let evidence_ref_str = evidence_dir.to_string_lossy().into_owned();
    let mut ev = Event::new("help", Severity::Info);
    ev.id = russell_core::event::EventId(Ulid::from_string(session_id).unwrap_or_default());
    ev.run_id = Some(session_id.to_string());
    ev.tier = Some("doctor".into());
    ev.module = Some("doctor/help".into());
    ev.summary = Some(format!(
        "backend={} status={} chars={}",
        backend_used,
        status_str,
        response_text.len()
    ));
    ev.evidence_ref = Some(evidence_ref_str.clone());
    ev.duration_ms = dispatch.latency_ms.map(|v| v as u64);
    if let Some(ref k) = dispatch.error_kind {
        ev.outputs
            .insert("error_kind".into(), serde_json::Value::from(k.as_str()));
    }
    if let Some(sr) = skip_reason {
        ev.outputs.insert(
            "skip_reason".into(),
            serde_json::Value::from(match sr {
                SkipReason::OfflineFallback => "offline_fallback",
                SkipReason::ThresholdSkip => "threshold_skip",
            }),
        );
    }
    writer.append(&ev)?;

    // Help-session row.
    let input = HelpSessionInput {
        id: session_id,
        ts_unix,
        ts,
        backend: backend_used,
        model: dispatch.model.as_deref(),
        note,
        prompt_chars: soap.rendered.len() as i64,
        response_chars: response_text.len() as i64,
        latency_ms: dispatch.latency_ms,
        status,
        error_kind: dispatch.error_kind.as_deref(),
        evidence_ref: &evidence_ref_str,
    };
    writer.append_help_session(&input)?;

    // ADR-0022: append a session note to today's daily log if it exists.
    append_session_note(paths, session_id, note, status_str);

    Ok(HelpOutcome {
        session_id: session_id.to_string(),
        backend: backend_used,
        evidence_dir: evidence_dir.to_path_buf(),
        response: response_text.to_string(),
        skip_reason,
    })
}

/// Same as [`run_help`] but with an explicit [`ClientConfig`]. Useful in
/// tests where mutating process env racily is undesirable.
pub async fn run_help_with_config(
    paths: &Paths,
    writer: &JournalWriter,
    note: Option<&str>,
    cfg: ClientConfig,
    kask_tool_names: &[(String, Option<String>)],
) -> Result<HelpOutcome> {
    let session_id = Ulid::new().to_string();
    let ts_unix = russell_core::time::now_unix();
    let ts = russell_core::time::now_rfc3339();

    let evidence_dir = paths.evidence().join("help").join(&session_id);
    std::fs::create_dir_all(&evidence_dir).map_err(|e| DoctorError::io(&evidence_dir, e))?;

    info!(backend = %cfg.backend.label(), model = %cfg.model, session = %session_id, "russell help starting");

    // Stage 1: compose + augment SOAP.
    let soap = compose_and_augment_soap(paths, writer, note, &evidence_dir, kask_tool_names)?;

    // Stage 2: threshold gate + backend dispatch.
    let counts = {
        let now = russell_core::time::now_unix();
        let window_start = now - 24 * 3600;
        writer
            .reader()
            .severity_counts(window_start, i64::MAX)
            .unwrap_or_default()
    };
    let escalate = cfg.escalate_min.satisfied_by(&counts);
    tracing::debug!(escalate = escalate, alert = %counts.alert, crit = %counts.crit, "threshold gate");

    let dispatch = dispatch_backend(writer, &cfg, &soap, escalate).await?;

    // Stage 3: persist evidence + journal.
    persist_session(
        paths,
        writer,
        &session_id,
        ts_unix,
        &ts,
        &evidence_dir,
        &soap,
        &dispatch,
        &cfg,
        note,
    )
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// ADR-0022: augment the compiled-in Jack persona with operator identity files.
fn augment_system_prompt(
    paths: &Paths,
    soap: crate::client::SoapPrompt,
) -> crate::client::SoapPrompt {
    let mut soap = soap;
    let mut extras = String::new();

    if let Ok(persona) = std::fs::read_to_string(paths.persona_md())
        && !persona.trim().is_empty()
    {
        tracing::debug!("loaded PERSONA.md for session context");
        extras.push_str("\n\n---\n\n");
        extras.push_str(&persona);
    }
    if let Ok(user) = std::fs::read_to_string(paths.user_md())
        && !user.trim().is_empty()
    {
        tracing::debug!("loaded USER.md for session context");
        extras.push_str("\n\n---\n\n");
        extras.push_str(&user);
    }

    if !extras.is_empty() {
        soap.system.push_str(&extras);
    }
    soap
}

/// ADR-0022: append a one-line session note to the current day's daily log.
/// Non-fatal — failures are logged but never returned as errors.
fn append_session_note(paths: &Paths, session_id: &str, note: Option<&str>, status: &str) {
    let now = russell_core::time::now_unix();
    let today = match time::OffsetDateTime::from_unix_timestamp(now) {
        Ok(dt) => dt,
        Err(_) => return,
    };
    let (year, month, day) = (today.year(), u8::from(today.month()), today.day());
    let filename = format!("{year:04}-{month:02}-{day:02}.md");
    let path = paths.memory_daily_dir().join(&filename);

    // Only append if the file already exists (lazy creation by digest).
    if !path.exists() {
        return;
    }

    let summary = match note {
        Some(n) if !n.trim().is_empty() => format!("{n} [{status}]"),
        _ => format!("(no note) [{status}]"),
    };
    let line = format!("- [{session_id}] — {summary}\n");

    if let Err(e) = std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .and_then(|mut f| std::io::Write::write_all(&mut f, line.as_bytes()))
    {
        tracing::warn!(error = %e, path = %path.display(), "failed to append session note to daily log");
    } else {
        tracing::debug!(session = %session_id, "appended session note to daily log");
    }
}

/// Short tag for the `error_kind` column.
fn error_kind_of(e: &DoctorError) -> String {
    match e {
        DoctorError::Io { .. } => "io".into(),
        DoctorError::Json(_) => "json".into(),
        DoctorError::Core(_) => "core".into(),
        DoctorError::Http {
            is_timeout: true, ..
        } => "http_timeout".into(),
        DoctorError::Http {
            is_connect: true, ..
        } => "http_connect".into(),
        DoctorError::Http {
            status: Some(s), ..
        } => format!("http_{s}"),
        DoctorError::Http { .. } => "http".into(),
        DoctorError::Authentication(_) => "auth".into(),
        DoctorError::ModelNotFound(_) => "model_not_found".into(),
        DoctorError::RateLimited { .. } => "rate_limited".into(),
        DoctorError::Config(_) => "config".into(),
        DoctorError::BadResponse(_) => "bad_response".into(),
        DoctorError::Fmt(_) => "fmt".into(),
        DoctorError::Other(_) => "other".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::EscalateMin;

    #[tokio::test]
    async fn offline_path_produces_fallback_outcome() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::rooted(tmp.path());
        paths.ensure_dirs().unwrap();
        let writer = JournalWriter::open(&paths.journal()).unwrap();

        let cfg = ClientConfig {
            backend: Backend::Offline,
            model: "test".into(),
            base_url: None,
            api_key: None,
            timeout: std::time::Duration::from_secs(5),
            escalate_min: EscalateMin::Alert,
        };
        let out = run_help_with_config(&paths, &writer, Some("unit test"), cfg, &[])
            .await
            .unwrap();

        assert!(out.skip_reason.is_some());
        assert_eq!(out.backend, "offline");
        assert!(out.response.contains("Offline"));
        assert!(out.evidence_dir.join("soap.md").exists());
        assert!(out.evidence_dir.join("transcript.jsonl").exists());

        // Verify the help_sessions row landed.
        let reader = russell_core::journal::JournalReader::new(paths.journal());
        assert!(reader.recent(1).unwrap().iter().any(|r| r.action == "help"));
    }

    #[tokio::test]
    async fn mock_path_produces_ok_outcome() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::rooted(tmp.path());
        paths.ensure_dirs().unwrap();
        let writer = JournalWriter::open(&paths.journal()).unwrap();

        let cfg = ClientConfig {
            backend: Backend::Mock,
            model: "test".into(),
            base_url: None,
            api_key: None,
            timeout: std::time::Duration::from_secs(5),
            escalate_min: EscalateMin::Always,
        };
        let out = run_help_with_config(&paths, &writer, None, cfg, &[])
            .await
            .unwrap();

        assert!(out.skip_reason.is_none());
        assert_eq!(out.backend, "mock");
        assert!(out.response.contains("Mock Jack"));
    }
}

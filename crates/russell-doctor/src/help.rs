// SPDX-License-Identifier: MIT OR Apache-2.0
//! The `run_help` orchestrator — the one public entry point.

use std::path::PathBuf;

use serde::Serialize;
use serde_json::json;
use tracing::{info, warn};
use ulid::Ulid;

use russell_core::event::{Event, Severity};
use russell_core::journal::JournalWriter;
use russell_core::paths::Paths;

use crate::client::{Backend, ClientConfig, LlmClient};
use crate::error::{DoctorError, Result};
use crate::{fallback, mock, openrouter, prompt};

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

/// Minimal session record mirrored into the `help_sessions` table.
#[derive(Debug, Clone, Serialize)]
pub struct HelpSession {
    /// ULID.
    pub id: String,
    /// Unix timestamp.
    pub ts_unix: i64,
    /// RFC3339 timestamp.
    pub ts: String,
    /// Backend label.
    pub backend: &'static str,
    /// Model, if any.
    pub model: Option<String>,
    /// Operator note.
    pub note: Option<String>,
    /// Prompt character count.
    pub prompt_chars: i64,
    /// Response character count.
    pub response_chars: i64,
    /// Round-trip latency (ms); `None` for offline.
    pub latency_ms: Option<i64>,
    /// `ok | error | fallback | threshold_skip`.
    pub status: &'static str,
    /// Short error kind, if status=error.
    pub error_kind: Option<String>,
    /// Path to evidence bundle.
    pub evidence_ref: String,
}

/// Run the help flow end to end: compose, call (or fall back), journal, print-ready.
///
/// `paths` and `writer` come from the CLI. The CLI is the only caller.
///
/// # Errors
///
/// Returns `DoctorError` if the filesystem write or journal write fails.
/// Provider errors are *caught* — the fallback handles them and the
/// function returns success with `fell_back = true`.
pub async fn run_help(
    paths: &Paths,
    writer: &JournalWriter,
    note: Option<&str>,
) -> Result<HelpOutcome> {
    let cfg = ClientConfig::from_env();
    run_help_with_config(paths, writer, note, cfg).await
}

/// Same as [`run_help`] but with an explicit [`ClientConfig`]. Useful in
/// tests where mutating process env racily is undesirable.
pub async fn run_help_with_config(
    paths: &Paths,
    writer: &JournalWriter,
    note: Option<&str>,
    cfg: ClientConfig,
) -> Result<HelpOutcome> {
    let session_id = Ulid::new().to_string();
    let ts_unix = russell_core::time::now_unix();
    let ts = russell_core::time::now_rfc3339();

    let evidence_dir = paths.evidence().join("help").join(&session_id);
    std::fs::create_dir_all(&evidence_dir).map_err(|e| DoctorError::io(&evidence_dir, e))?;

    let profile_path = paths.profile();
    let profile = if profile_path.exists() {
        russell_core::Profile::load(&profile_path).ok()
    } else {
        None
    };

    let soap = prompt::compose(&writer.reader(), profile.as_ref(), note)?;

    // ADR-0022: augment the system prompt with operator identity files if present.
    let soap = augment_system_prompt(paths, soap);

    let soap_path = evidence_dir.join("soap.md");
    std::fs::write(&soap_path, &soap.rendered).map_err(|e| DoctorError::io(&soap_path, e))?;

    info!(backend = %cfg.backend.label(), model = %cfg.model, session = %session_id, "russell help starting");

    // ADR-0020: threshold gate — check severity before waking the LLM.
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

    // Attempt the configured backend. Any failure → fall back to offline.
    let (backend_used, maybe_response, error_kind, skip_reason) = match cfg.backend {
        Backend::OpenRouter => {
            let client = openrouter::OpenRouterClient::new(&cfg)?;
            match client.chat(&soap).await {
                Ok(resp) => ("openrouter", Some(resp), None, None),
                Err(e) => {
                    warn!(error = %e, "openrouter call failed — falling back");
                    (
                        "openrouter",
                        None,
                        Some(error_kind_of(&e)),
                        Some(SkipReason::OfflineFallback),
                    )
                }
            }
        }
        Backend::Ollama => {
            // Ollama speaks the same OpenAI-compatible API at :11434/v1
            let mut ollama_cfg = cfg.clone();
            if ollama_cfg.base_url.is_none() {
                ollama_cfg.base_url = Some("http://127.0.0.1:11434/v1".into());
            }
            if ollama_cfg.api_key.is_none() {
                ollama_cfg.api_key = Some("ollama".into());
            }
            let client = openrouter::OpenRouterClient::new(&ollama_cfg)?;
            match client.chat(&soap).await {
                Ok(resp) => ("ollama", Some(resp), None, None),
                Err(e) => {
                    warn!(error = %e, "ollama call failed — falling back");
                    (
                        "ollama",
                        None,
                        Some(error_kind_of(&e)),
                        Some(SkipReason::OfflineFallback),
                    )
                }
            }
        }
        Backend::Mock => {
            let client = mock::MockClient::jack_default();
            match client.chat(&soap).await {
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

    // If threshold not met, short-circuit to rule-based summary.
    let skip_reason = if !escalate {
        Some(SkipReason::ThresholdSkip)
    } else {
        skip_reason
    };

    let (response_text, latency_ms, model) = match maybe_response {
        Some(resp) => (
            resp.content.clone(),
            Some(resp.latency_ms as i64),
            resp.model,
        ),
        None => {
            let text = fallback::summarise(&writer.reader(), note)?;
            (text, None, None)
        }
    };

    // Persist request/response/transcript artefacts for inspection.
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
        "status": if let Some(sr) = skip_reason {
            match sr {
                SkipReason::OfflineFallback => "fallback",
                SkipReason::ThresholdSkip => "threshold_skip",
            }
        } else {
            "ok"
        },
        "error_kind": error_kind,
        "latency_ms": latency_ms,
        "model": model,
        "content_chars": response_text.len(),
    });
    std::fs::write(&response_path, serde_json::to_vec_pretty(&response_rec)?)
        .map_err(|e| DoctorError::io(&response_path, e))?;

    let transcript_path = evidence_dir.join("transcript.jsonl");
    let line = json!({
        "schema": "harness.llm-transcript.v1",
        "ts": &ts,
        "backend": backend_used,
        "model": &cfg.model,
        "fell_back": skip_reason.is_some(),
        "skip_reason": skip_reason.map(|s| match s {
            SkipReason::OfflineFallback => "offline_fallback",
            SkipReason::ThresholdSkip => "threshold_skip",
        }),
        "prompt_chars": soap.rendered.len(),
        "response_chars": response_text.len(),
        "response": &response_text,
    });
    std::fs::write(&transcript_path, format!("{line}\n"))
        .map_err(|e| DoctorError::io(&transcript_path, e))?;

    // Journal: events row + help_sessions row.
    let evidence_ref_str = evidence_dir.to_string_lossy().into_owned();
    let status: &'static str = if let Some(sr) = skip_reason {
        match sr {
            SkipReason::OfflineFallback => "fallback",
            SkipReason::ThresholdSkip => "threshold_skip",
        }
    } else {
        "ok"
    };

    let mut ev = Event::new("help", Severity::Info);
    ev.id = russell_core::event::EventId(Ulid::from_string(&session_id).unwrap_or_default());
    ev.run_id = Some(session_id.clone());
    ev.tier = Some("doctor".into());
    ev.module = Some("doctor/help".into());
    ev.summary = Some(format!(
        "backend={} status={} chars={}",
        backend_used,
        status,
        response_text.len()
    ));
    ev.evidence_ref = Some(evidence_ref_str.clone());
    ev.duration_ms = latency_ms.map(|v| v as u64);
    if let Some(ref k) = error_kind {
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

    let session = HelpSession {
        id: session_id.clone(),
        ts_unix,
        ts: ts.clone(),
        backend: backend_used,
        model,
        note: note.map(str::to_string),
        prompt_chars: soap.rendered.len() as i64,
        response_chars: response_text.len() as i64,
        latency_ms,
        status,
        error_kind,
        evidence_ref: evidence_ref_str,
    };
    insert_help_session(writer, &session)?;

    // ADR-0022: append a session note to today's daily log if it exists.
    append_session_note(paths, &session_id, note, status);

    Ok(HelpOutcome {
        session_id,
        backend: backend_used,
        evidence_dir,
        response: response_text,
        skip_reason,
    })
}

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
        DoctorError::ZdrRoutingFailed(_) => "zdr_failed".into(),
        DoctorError::Config(_) => "config".into(),
        DoctorError::BadResponse(_) => "bad_response".into(),
        DoctorError::Fmt(_) => "fmt".into(),
        DoctorError::Other(_) => "other".into(),
    }
}

fn insert_help_session(writer: &JournalWriter, s: &HelpSession) -> Result<()> {
    // russell-core owns DB access; expose a typed method on the writer.
    writer.append_help_session_row(
        &s.id,
        s.ts_unix,
        &s.ts,
        s.backend,
        s.model.as_deref(),
        s.note.as_deref(),
        s.prompt_chars,
        s.response_chars,
        s.latency_ms,
        s.status,
        s.error_kind.as_deref(),
        &s.evidence_ref,
    )?;
    Ok(())
}

// paths is used only for ::join; leave for future expansion.
#[allow(dead_code)]
fn _evidence_subdir(paths: &Paths, id: &str) -> PathBuf {
    paths.evidence().join("help").join(id)
}

/// Return the path to the most-recent evidence bundle under
/// `paths.evidence()/help/`, if any. Used by tests.
#[must_use]
pub fn last_evidence_dir(paths: &Paths) -> Option<PathBuf> {
    let root = paths.evidence().join("help");
    let mut entries: Vec<_> = std::fs::read_dir(&root)
        .ok()?
        .filter_map(std::result::Result::ok)
        .map(|e| e.path())
        .filter(|p: &PathBuf| p.is_dir())
        .collect();
    entries.sort();
    entries.into_iter().next_back()
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
        let out = run_help_with_config(&paths, &writer, Some("unit test"), cfg)
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
        let out = run_help_with_config(&paths, &writer, None, cfg)
            .await
            .unwrap();

        assert!(out.skip_reason.is_none());
        assert_eq!(out.backend, "mock");
        assert!(out.response.contains("Mock Jack"));
    }
}

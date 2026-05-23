// SPDX-License-Identifier: MIT OR Apache-2.0
//! `run_help` — Nurse pipeline via hKask.
//!
//! Russell collects telemetry and sends it to hKask for LLM inference.
//! hKask handles prompt composition, LLM calls, and returns the response.

use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tracing::{info, warn};
use ulid::Ulid;

use russell_core::config::RuntimeConfig;
use russell_core::journal::{HelpSessionInput, HelpSessionStatus, JournalWriter};
use russell_core::paths::Paths;

use crate::error::{DoctorError, Result};

/// Outcome from a `run_help` call.
#[derive(Debug, Clone, Serialize)]
pub struct HelpOutcome {
    /// Unique session identifier.
    pub session_id: String,
    /// Backend used ("hkask" or "offline").
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
    /// hKask was unreachable; offline fallback was used.
    OfflineFallback,
    /// No crit/alert events; threshold gate prevented escalation.
    ThresholdSkip,
}

#[derive(Serialize)]
struct HKaskInferRequest {
    /// Capability token (base64-encoded hKask CapabilityToken JSON).
    /// Required by hKask's `/api/llm/infer` endpoint for OCAP verification.
    capability_token: String,
    subjective: Option<String>,
    objective: ObjectiveData,
    assessment: String,
    plan: String,
}

#[derive(Serialize)]
struct ObjectiveData {
    severity_counts: SeverityCounts,
    recent_events: Vec<EventRecord>,
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

/// Run the Nurse pipeline using the hKask endpoint from [`RuntimeConfig`].
///
/// Loads configuration from environment variables (see [`RuntimeConfig::from_env`]).
pub async fn run_help(
    paths: &Paths,
    writer: &JournalWriter,
    note: Option<&str>,
    _hkask_tool_names: &[(String, Option<String>)],
) -> Result<HelpOutcome> {
    let config = RuntimeConfig::from_env();
    run_help_with_endpoint(paths, writer, note, &config.hkask_endpoint).await
}

/// Run the Nurse pipeline with a configurable hKask endpoint.
///
/// Gathers objective telemetry, checks the threshold gate, calls hKask
/// for LLM inference, and journals the help session. Falls back to an
/// offline summary if hKask is unreachable or the threshold gate skips.
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

    let capability_token = load_capability_token().unwrap_or_default();
    let request = HKaskInferRequest {
        capability_token,
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
        status: HelpSessionStatus::Ok,
        error_kind: None,
        evidence_ref: &evidence_dir.to_string_lossy(),
    };
    writer.append_help_session(&input)?;

    let evidence_path = evidence_dir.join("response.json");
    std::fs::write(&evidence_path, serde_json::to_string_pretty(&response)?)
        .map_err(|e| DoctorError::io(&evidence_path, e))?;

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

async fn call_hkask(endpoint: &str, request: &HKaskInferRequest) -> Result<HKaskInferResponse> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| DoctorError::Config(format!("HTTP client: {e}")))?;

    let mut req = client.post(endpoint).json(request);

    if let Some(token) = load_service_token() {
        req = req.bearer_auth(&token);
    }

    let response = req
        .send()
        .await
        .map_err(|e| DoctorError::Config(format!("hKask request: {e}")))?;

    if !response.status().is_success() {
        return Err(DoctorError::Config(format!(
            "hKask returned {}",
            response.status()
        )));
    }

    response
        .json()
        .await
        .map_err(|e| DoctorError::Config(format!("hKask response: {e}")))
}

fn load_service_token() -> Option<String> {
    let token_path = russell_core::paths::Paths::from_env()
        .ok()?
        .state
        .join("russell.token");

    if let Ok(token) = std::fs::read_to_string(&token_path) {
        let trimmed = token.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_owned());
        }
    }

    let token = generate_service_token();
    if let Some(parent) = token_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let _ = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&token_path)
            .and_then(|mut f| std::io::Write::write_all(&mut f, token.as_bytes()));
    }
    #[cfg(not(unix))]
    {
        let _ = std::fs::write(&token_path, &token);
    }
    Some(token)
}

fn generate_service_token() -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
            .to_le_bytes(),
    );
    hasher.update(std::process::id().to_le_bytes());
    hasher.update(b"russell-service-principal");
    hex::encode(hasher.finalize())
}

/// Load or generate a hKask-format capability token for SOAP inference.
///
/// The token is stored at `~/.local/state/harness/russell.capability_token`
/// as base64-encoded JSON matching hKask's `CapabilityToken` schema.
///
/// Returns `None` if the token cannot be loaded or generated.
fn load_capability_token() -> Option<String> {
    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
    use hmac::{Hmac, Mac};
    use serde_json::json;
    use sha2::{Digest, Sha256};

    type HmacSha256 = Hmac<Sha256>;

    let token_path = russell_core::paths::Paths::from_env()
        .ok()?
        .state
        .join("russell.capability_token");

    // Try to load existing token
    if let Ok(token) = std::fs::read_to_string(&token_path) {
        let trimmed = token.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_owned());
        }
    }

    // Generate new token matching hKask's CapabilityToken schema
    let secret = std::env::var("HKASK_CAPABILITY_KEY").ok()?;
    let russell_webid = "russell-agent";
    let hkask_webid = "hkask-inference";

    // Compute token ID: SHA-256(resource || resource_id || action || from || to)
    let mut id_hasher = Sha256::new();
    id_hasher.update(b"Tool");
    id_hasher.update(b"inference");
    id_hasher.update(b"Execute");
    id_hasher.update(russell_webid.as_bytes());
    id_hasher.update(hkask_webid.as_bytes());
    let token_id = hex::encode(id_hasher.finalize());

    // Compute signature: HMAC-SHA256(secret, id || resource || resource_id || action || from || to)
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC can take key of any size");
    mac.update(token_id.as_bytes());
    mac.update(b"Tool");
    mac.update(b"inference");
    mac.update(b"Execute");
    mac.update(russell_webid.as_bytes());
    mac.update(hkask_webid.as_bytes());
    let signature = hex::encode(mac.finalize().into_bytes());

    let token_json = json!({
        "id": token_id,
        "resource": "Tool",
        "resource_id": "inference",
        "action": "Execute",
        "delegated_from": russell_webid,
        "delegated_to": hkask_webid,
        "signature": signature,
        "expires_at": null,
        "attenuation_level": 0,
        "max_attenuation": 7,
        "context_nonce": format!("root-{}", russell_webid)
    });

    let token_b64 = BASE64.encode(serde_json::to_string(&token_json).ok()?.as_bytes());

    // Persist token
    if let Some(parent) = token_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let _ = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&token_path)
            .and_then(|mut f| std::io::Write::write_all(&mut f, token_b64.as_bytes()));
    }
    #[cfg(not(unix))]
    {
        let _ = std::fs::write(&token_path, &token_b64);
    }

    Some(token_b64)
}

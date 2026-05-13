// SPDX-License-Identifier: MIT OR Apache-2.0
//! OpenAI-compatible client targeting Okapi (local inference).
//!
//! Single round-trip POST to `/chat/completions`. No streaming,
//! no retry, no tool-calling.

use std::time::Instant;

use serde_json::json;
use tracing::{debug, warn};

use crate::client::{ClientConfig, LlmClient, LlmResponse, SoapPrompt};
use crate::error::{DoctorError, Result};

/// Model entry from Okapi's `/api/tags` (Ollama-compatible).
#[derive(Debug, Clone, serde::Deserialize)]
struct OkapiModel {
    name: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct OkapiTagsResponse {
    models: Vec<OkapiModel>,
}

/// OpenAI-compatible LLM client. Targets Okapi at localhost:11435.
///
/// Construction via [`OkapiClient::new`] **validates the model name**
/// against Okapi's actual loaded model list (`/api/tags`). If the
/// candidate model name does not match any loaded model exactly, the
/// constructor performs a fuzzy match and returns the closest real
/// model name. If Okapi is unreachable, the candidate is used as-is
/// with a warning logged.
#[derive(Debug, Clone)]
pub struct OkapiClient {
    base_url: String,
    model: String,
    api_key: Option<String>,
    http: reqwest::Client,
}

impl OkapiClient {
    /// Construct from a resolved [`ClientConfig`], **resolving the
    /// model name** against Okapi's `/api/tags` list.
    ///
    /// The model name in `cfg` is treated as a candidate. The
    /// constructor queries Okapi for loaded models, finds the best
    /// fuzzy match, and stores the **exact** model name reported by
    /// Okapi. This prevents stale or misspelled model names from
    /// reaching the chat-completions endpoint.
    ///
    /// # Errors
    /// Returns [`DoctorError::Config`] if the HTTP client cannot be built.
    pub async fn new(cfg: &ClientConfig) -> Result<Self> {
        let base_url = cfg
            .base_url
            .clone()
            .unwrap_or_else(|| "http://127.0.0.1:11435/v1".into());
        let http = reqwest::Client::builder()
            .timeout(cfg.timeout)
            .user_agent(concat!("russell/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| DoctorError::Config(format!("HTTP client: {e}")))?;

        let resolved = resolve_model_name(&base_url, &cfg.model, &http).await;

        if resolved != cfg.model {
            warn!(
                candidate = %cfg.model,
                resolved = %resolved,
                "model name resolved"
            );
        }

        Ok(Self {
            base_url,
            model: resolved,
            api_key: cfg.api_key.clone(),
            http,
        })
    }

    fn endpoint(&self) -> String {
        format!("{}/chat/completions", self.base_url.trim_end_matches('/'))
    }
}

impl LlmClient for OkapiClient {
    async fn chat(&self, prompt: &SoapPrompt) -> Result<LlmResponse> {
        let body = json!({
            "model": self.model,
            "messages": [
                { "role": "system", "content": prompt.system },
                { "role": "user",   "content": prompt.rendered },
            ],
            "temperature": 0.2,
        });

        let url = self.endpoint();
        let started = Instant::now();
        let mut req = self
            .http
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body);
        if let Some(k) = &self.api_key {
            req = req.bearer_auth(k);
        }

        let resp = req
            .send()
            .await
            .map_err(|e| map_reqwest_error("send", &e))?;
        let status = resp.status();
        let retry_after = resp
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .and_then(parse_retry_after);
        let body_text = resp
            .text()
            .await
            .map_err(|e| map_reqwest_error("body", &e))?;

        if !status.is_success() {
            return Err(map_http_status_error(
                status.as_u16(),
                body_text,
                retry_after,
                &url,
            ));
        }

        let latency_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let parsed = parse_completion(&body_text)?;

        debug!(
            model = %self.model,
            prompt_tokens = ?parsed.prompt_tokens,
            completion_tokens = ?parsed.completion_tokens,
            latency_ms,
            "Okapi chat completion OK"
        );

        Ok(LlmResponse {
            content: parsed.content,
            model: parsed.model,
            prompt_tokens: parsed.prompt_tokens,
            completion_tokens: parsed.completion_tokens,
            latency_ms,
        })
    }
}

/// Resolve a candidate model name against Okapi's actual model list.
///
/// Queries `/api/tags` to get the set of loaded models,
/// then finds the best fuzzy match (Jaro-Winkler ≥ 0.80).
/// Returns the exact model name from Okapi on match, or the
/// original candidate if Okapi is unreachable or has no models.
async fn resolve_model_name(
    base_url: &str,
    candidate: &str,
    http: &reqwest::Client,
) -> String {
    let tags_url = format!(
        "{}/api/tags",
        base_url.trim_end_matches("/v1").trim_end_matches('/')
    );

    let models: Vec<String> = match http
        .get(&tags_url)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => match resp.json::<OkapiTagsResponse>().await {
            Ok(body) => body.models.into_iter().map(|m| m.name).collect(),
            Err(_) => {
                warn!("failed to parse /api/tags response");
                return candidate.to_string();
            }
        },
        Ok(resp) => {
            warn!(status = %resp.status(), "non-success from /api/tags");
            return candidate.to_string();
        }
        Err(e) => {
            warn!(error = %e, "can't reach Okapi for model resolution");
            return candidate.to_string();
        }
    };

    if models.is_empty() {
        warn!("Okapi reports zero loaded models");
        return candidate.to_string();
    }

    // Exact match.
    if models.iter().any(|m| m == candidate) {
        return candidate.to_string();
    }

    // Fuzzy match: strip non-alphanumeric chars before scoring
    // so "nemotron3super" matches "nemotron3-super:cloud".
    let candidate_lower = candidate.to_lowercase();
    let candidate_clean: String = candidate_lower
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect();

    let mut scored: Vec<(&str, f64)> = models
        .iter()
        .map(|m| {
            let name_lower = m.to_lowercase();
            let name_clean: String =
                name_lower.chars().filter(|c| c.is_alphanumeric()).collect();
            let score = strsim::jaro_winkler(&candidate_clean, &name_clean);
            (m.as_str(), score)
        })
        .collect();

    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    if let Some((best_name, score)) = scored.first() {
        if *score >= 0.80 {
            debug!(
                candidate = %candidate,
                resolved = %best_name,
                score = %score,
                "fuzzy model resolution"
            );
            return best_name.to_string();
        }
    }

    warn!(
        candidate = %candidate,
        available = ?models,
        "no matching model found in Okapi"
    );
    candidate.to_string()
}

/// Resolve the model name against Okapi's actual model list, and if
/// the resolved name differs from the configured name, **correct the
/// env file** so the fix persists across restarts.
///
/// Searches Russell's env-file discovery order (config dir first,
/// then repo root, then cwd). If a `russell.env` or `.env` file
/// contains `RUSSELL_DOCTOR_MODEL=<old value>`, the line is
/// replaced with the resolved name. Also updates the process
/// environment so subsequent reads see the correction.
///
/// Returns the resolved (actual) model name.
pub async fn resolve_and_correct_model(
    cfg: &ClientConfig,
    config_harness_dir: &std::path::Path,
) -> String {
    let http = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(_) => return cfg.model.clone(),
    };
    let base_url = cfg.base_url.as_deref().unwrap_or(crate::health::DEFAULT_BASE_URL);
    let resolved = resolve_model_name(base_url, &cfg.model, &http).await;

    if resolved == cfg.model {
        return resolved;
    }

    // Correct the env file and process environment.
    let env_path = russell_core::env::find_env_file(config_harness_dir);
    let Some(env_path) = env_path else {
        return resolved;
    };

    let text = match std::fs::read_to_string(&env_path) {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!(error = %e, path = %env_path.display(), "can't read env file for correction");
            return resolved;
        }
    };

    let mut found = false;
    let mut updated = String::with_capacity(text.len() + 32);
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            updated.push_str(raw);
            updated.push('\n');
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            updated.push_str(raw);
            updated.push('\n');
            continue;
        };
        let value = strip_optional_quotes(v.trim());
        if k.trim() == "RUSSELL_DOCTOR_MODEL" && value == cfg.model && !found {
            updated.push_str(&format!("RUSSELL_DOCTOR_MODEL={resolved}\n"));
            found = true;
            continue;
        }
        updated.push_str(raw);
        updated.push('\n');
    }

    if found {
        if let Err(e) = std::fs::write(&env_path, &updated) {
            tracing::warn!(error = %e, path = %env_path.display(), "failed to write corrected env file");
        } else {
            tracing::info!(
                path = %env_path.display(),
                old = %cfg.model,
                new = %resolved,
                "env file corrected"
            );
            #[allow(unsafe_code)]
            unsafe {
                std::env::set_var("RUSSELL_DOCTOR_MODEL", &resolved);
            }
        }
    }

    resolved
}

fn strip_optional_quotes(s: &str) -> &str {
    let bytes = s.as_bytes();
    if bytes.len() >= 2
        && ((bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\''))
    {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

/// Convert a [`reqwest::Error`] into [`DoctorError::Http`].
fn map_reqwest_error(context: &str, e: &reqwest::Error) -> DoctorError {
    DoctorError::Http {
        message: format!("{context}: {e}"),
        is_timeout: e.is_timeout(),
        is_connect: e.is_connect(),
        status: e.status().map(|s| s.as_u16()),
    }
}

/// Map an HTTP non-success status to the appropriate variant.
fn map_http_status_error(
    status: u16,
    body: String,
    retry_after: Option<u64>,
    url: &str,
) -> DoctorError {
    match status {
        401 => DoctorError::Authentication(body),
        402 | 403 => DoctorError::Http {
            status: Some(status),
            message: body,
            is_connect: false,
            is_timeout: false,
        },
        404 => DoctorError::ModelNotFound(format!("{body} (url: {url})")),
        429 => DoctorError::RateLimited {
            retry_after_seconds: retry_after,
        },
        _ => DoctorError::Http {
            status: Some(status),
            message: body,
            is_connect: false,
            is_timeout: false,
        },
    }
}

fn parse_retry_after(value: &str) -> Option<u64> {
    value.trim().parse::<u64>().ok()
}

#[derive(Debug)]
struct ParsedCompletion {
    content: String,
    model: Option<String>,
    prompt_tokens: Option<u32>,
    completion_tokens: Option<u32>,
}

fn parse_completion(body: &str) -> Result<ParsedCompletion> {
    let v: serde_json::Value = serde_json::from_str(body)
        .map_err(|e| DoctorError::BadResponse(format!("json parse: {e}: {body}")))?;
    let choice0 = v
        .get("choices")
        .and_then(|c| c.get(0))
        .ok_or_else(|| DoctorError::BadResponse(format!("no choices in response: {body}")))?;
    let msg = choice0
        .get("message")
        .ok_or_else(|| DoctorError::BadResponse("no message in choice".into()))?;

    // Some models emit reasoning in `reasoning_details` or
    // `reasoning_content` instead of `content`. Promote when needed.
    let content = match msg.get("content") {
        Some(serde_json::Value::String(s)) if !s.is_empty() => s.clone(),
        _ => {
            if let Some(text) = msg
                .get("reasoning_details")
                .and_then(|rd| rd.as_array())
                .and_then(|arr| {
                    arr.iter()
                        .find_map(|d| d.get("text").and_then(|t| t.as_str()))
                })
            {
                warn!(
                    normalization = "reasoning_details",
                    "promoted reasoning to content"
                );
                text.to_string()
            } else if let Some(rc) = msg.get("reasoning_content").and_then(|v| v.as_str()) {
                warn!(
                    normalization = "reasoning_content",
                    "promoted reasoning to content"
                );
                rc.to_string()
            } else {
                return Err(DoctorError::BadResponse(
                    "response had no usable content field".into(),
                ));
            }
        }
    };

    let model = v.get("model").and_then(|m| m.as_str()).map(str::to_string);
    let usage = v.get("usage");
    let prompt_tokens = usage
        .and_then(|u| u.get("prompt_tokens"))
        .and_then(|x| x.as_u64())
        .map(|n| u32::try_from(n).unwrap_or(u32::MAX));
    let completion_tokens = usage
        .and_then(|u| u.get("completion_tokens"))
        .and_then(|x| x.as_u64())
        .map(|n| u32::try_from(n).unwrap_or(u32::MAX));

    Ok(ParsedCompletion {
        content,
        model,
        prompt_tokens,
        completion_tokens,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_standard_openai_response() {
        let body = r#"{
            "id":"x",
            "model":"deepseekv4pro",
            "choices":[{"message":{"role":"assistant","content":"hello"}}],
            "usage":{"prompt_tokens":10,"completion_tokens":2}
        }"#;
        let p = parse_completion(body).unwrap();
        assert_eq!(p.content, "hello");
        assert_eq!(p.model.as_deref(), Some("deepseekv4pro"));
        assert_eq!(p.prompt_tokens, Some(10));
    }

    #[test]
    fn promotes_reasoning_details_when_content_null() {
        let body = r#"{
            "choices":[{"message":{"role":"assistant","content":null,
              "reasoning_details":[{"text":"thought"}]}}]
        }"#;
        let p = parse_completion(body).unwrap();
        assert_eq!(p.content, "thought");
    }

    #[test]
    fn promotes_reasoning_content_when_content_missing() {
        let body = r#"{
            "choices":[{"message":{"role":"assistant",
              "reasoning_content":"thought2"}}]
        }"#;
        let p = parse_completion(body).unwrap();
        assert_eq!(p.content, "thought2");
    }

    #[test]
    fn rejects_response_with_no_content() {
        let body = r#"{"choices":[{"message":{"role":"assistant"}}]}"#;
        assert!(matches!(
            parse_completion(body),
            Err(DoctorError::BadResponse(_))
        ));
    }

    #[test]
    fn maps_404_to_model_not_found() {
        let err = map_http_status_error(
            404,
            "no such model".into(),
            None,
            "http://127.0.0.1:11435/v1/chat/completions",
        );
        assert!(matches!(err, DoctorError::ModelNotFound(_)));
    }

    #[test]
    fn maps_429_with_retry_after() {
        let err = map_http_status_error(429, "rate".into(), Some(30), "");
        assert!(matches!(
            err,
            DoctorError::RateLimited {
                retry_after_seconds: Some(30)
            }
        ));
    }
}

// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Reference-copied patterns from:
//   slate/stack/crates/stack-llm/src/openai.rs
//   slate/stack/crates/stack-llm/src/wire.rs
// Upstream commit (slate): 67a13834d8af4efa8c330ce10ef1031bf2cdeee2
// Russell changes:
//   - Uses DoctorError, not stack_types::LlmError
//   - Uses SoapPrompt / LlmResponse, not stack_types wire types
//   - No streaming, no tool-calling, no structured-output
//   - Enforces ZDR via provider preferences on every request
//   - Single round-trip, no retry
//   - Normalises DeepSeek / Kimi reasoning_details content fallback
// Sync policy: review on upstream bug fix; pull fixes, not features.
// See docs/operations/REUSE_MANIFEST.md row 1.

//! OpenRouter / OpenAI-compatible backend.
//!
//! Implements a single round-trip POST to `/chat/completions`.
//! No streaming, no retry, no tool-calling — see
//! [ADR-0016](../../docs/adr/0016-doctor-and-llm-router.md).

use std::time::Instant;

use serde_json::json;
use tracing::{debug, warn};

use crate::client::{ClientConfig, LlmClient, LlmResponse, SoapPrompt};
use crate::error::{DoctorError, Result};

/// OpenAI-compatible backend targeting OpenRouter by default.
#[derive(Debug, Clone)]
pub struct OpenRouterClient {
    base_url: String,
    model: String,
    api_key: Option<String>,
    http: reqwest::Client,
    referer: String,
    title: String,
}

impl OpenRouterClient {
    /// Construct from a resolved [`ClientConfig`].
    ///
    /// # Errors
    /// Returns [`DoctorError::Config`] if the HTTP client cannot be built.
    pub fn new(cfg: &ClientConfig) -> Result<Self> {
        let base_url = cfg
            .base_url
            .clone()
            .unwrap_or_else(|| "https://openrouter.ai/api/v1".into());
        let http = reqwest::Client::builder()
            .timeout(cfg.timeout)
            .user_agent(concat!("russell/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| DoctorError::Config(format!("HTTP client: {e}")))?;
        Ok(Self {
            base_url,
            model: cfg.model.clone(),
            api_key: cfg.api_key.clone(),
            http,
            referer: "https://russell.local/".into(),
            title: "Russell".into(),
        })
    }

    fn endpoint(&self) -> String {
        format!("{}/chat/completions", self.base_url.trim_end_matches('/'))
    }
}

impl LlmClient for OpenRouterClient {
    async fn chat(&self, prompt: &SoapPrompt) -> Result<LlmResponse> {
        let body = json!({
            "model": self.model,
            "messages": [
                { "role": "system", "content": prompt.system },
                { "role": "user",   "content": prompt.rendered },
            ],
            "temperature": 0.2,
            // Per-request ZDR enforcement — see OpenRouter docs
            // `guides/features/zdr`. Requests that cannot be
            // routed to a ZDR endpoint fail rather than fall back
            // to a retaining provider.
            "provider": {
                "zdr": true,
                "data_collection": "deny"
            }
        });

        let url = self.endpoint();
        let started = Instant::now();
        let mut req = self
            .http
            .post(&url)
            .header("Content-Type", "application/json")
            .header("HTTP-Referer", &self.referer)
            .header("X-Title", &self.title)
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
            "OpenRouter chat completion OK"
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

// --- wire-format helpers (pattern from stack-llm/src/wire.rs) ----------------

/// Convert a [`reqwest::Error`] into [`DoctorError::Http`] preserving
/// structured metadata. Called at every `.send().await` / `.text().await`.
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
    // OpenRouter surfaces ZDR routing failures with a specific body pattern
    // we recognise so the operator knows *why* the call failed.
    if status == 403 && body.to_ascii_lowercase().contains("zero data retention") {
        return DoctorError::ZdrRoutingFailed(body);
    }
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

// --- response parsing --------------------------------------------------------

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

    // Content-normalisation, pattern from stack-llm/src/wire.rs:
    // some models (DeepSeek, Kimi K2.5, GLM-5) emit reasoning in
    // `reasoning_details[].text` or `reasoning_content` rather than
    // `content`. Promote those when `content` is null/missing.
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
            "https://openrouter.ai/api/v1/chat/completions",
        );
        assert!(matches!(err, DoctorError::ModelNotFound(_)));
    }

    #[test]
    fn maps_zdr_failure() {
        let err = map_http_status_error(
            403,
            "Zero data retention required but no ZDR provider available".into(),
            None,
            "",
        );
        assert!(matches!(err, DoctorError::ZdrRoutingFailed(_)));
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

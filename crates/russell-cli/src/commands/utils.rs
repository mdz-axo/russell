// SPDX-License-Identifier: MIT OR Apache-2.0
//! Shared CLI utilities — YAML extraction, LLM helpers.
//!
//! Consolidates duplicated helpers from workshop.rs and chat/mod.rs.

use anyhow::Result;

/// Extract a scalar YAML field from a manifest string.
pub fn extract_yaml_field(manifest: &str, key: &str) -> Option<String> {
    for line in manifest.lines() {
        if let Some(rest) = line.trim().strip_prefix(&format!("{key}:")) {
            return Some(rest.trim().to_string());
        }
    }
    None
}

/// Extract a YAML list from a manifest string (simple line-based parser).
pub fn extract_yaml_list(manifest: &str, key: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut in_section = false;
    for line in manifest.lines() {
        if line.trim_start().starts_with(&format!("{key}:")) {
            in_section = true;
            continue;
        }
        if in_section {
            if let Some(item) = line.trim().strip_prefix("- ") {
                items.push(item.trim().to_string());
            } else if line.trim().starts_with(|c: char| c.is_alphabetic()) {
                break;
            }
        }
    }
    items
}

/// Extract the first ```yaml...``` or ---manifest...--- block from LLM output.
pub fn extract_manifest_block(response: &str) -> Option<String> {
    // Try ---manifest format first
    if let Some(start) = response.find("---manifest\n") {
        let content_start = start + "---manifest\n".len();
        let remainder = &response[content_start..];
        let end = if let Some(pos) = remainder.find("\n---\n") {
            pos + 1
        } else if remainder.starts_with("---\n") {
            0
        } else if remainder == "---" {
            remainder.len()
        } else if remainder.ends_with("\n---") {
            remainder.len() - 3
        } else {
            return None;
        };
        let content = remainder[..end].trim().to_string();
        if content.is_empty() {
            return None;
        }
        return Some(content);
    }
    
    // Try ```yaml format
    let start = response.find("```yaml");
    let body_start = match start {
        Some(s) => s + 7,
        None => return None,
    };
    let end = match response[body_start..].find("```") {
        Some(e) => body_start + e,
        None => return Some(response[body_start..].trim().to_string()),
    };
    Some(response[body_start..end].trim().to_string())
}

/// Call Okapi LLM with a prompt. Returns response text or empty on failure.
pub async fn call_llm(
    cfg: &russell_meta::client::ClientConfig,
    model: &str,
    system: &str,
    prompt: &str,
    temperature: Option<f64>,
) -> Result<String> {
    use russell_meta::client::SoapPrompt;
    use russell_meta::oai_client::OkapiClient;

    let mut chat_cfg = cfg.clone();
    chat_cfg.model = model.to_string();
    if chat_cfg.base_url.is_none() {
        chat_cfg.base_url = Some(russell_meta::health::DEFAULT_BASE_URL.to_string());
    }
    if chat_cfg.api_key.is_none() {
        chat_cfg.api_key = Some("okapi".into());
    }

    let base = chat_cfg.base_url.as_deref().unwrap_or(russell_meta::health::DEFAULT_BASE_URL);
    if !russell_meta::health::ensure_ready(base).await {
        return Err(anyhow::anyhow!("Okapi not reachable"));
    }

    let client = OkapiClient::new(&chat_cfg).await?;
    let soap = SoapPrompt {
        system: system.to_string(),
        subjective: String::new(),
        objective: String::new(),
        rendered: prompt.to_string(),
        temperature,
        max_tokens: None,
    };
    use russell_meta::client::LlmClient;
    Ok(client.chat(&soap).await?.content)
}
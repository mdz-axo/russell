// SPDX-License-Identifier: MIT OR Apache-2.0
//! LLM call with animated thinking spinner.

use rand::seq::SliceRandom;
use russell_doctor::client::LlmClient;
use russell_doctor::client::SoapPrompt;
use russell_doctor::oai_client::OkapiClient;
use std::io::Write;
use tokio::sync::oneshot;

/// Braille spinner frames for the thinking animation.
const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Jack's thinking expressions — a mix of terrier and McFarland energy.
const THINKING_EXPRESSIONS: &[&str] = &[
    "🐶 digging digging digging",
    "✨ hey hey! hold your kibble",
    "🐶 *sniff* *sniff* checking things",
    "💅 working it. just a moment.",
    "🔍 just checking on you",
];

/// Call the LLM via Okapi with an animated thinking spinner on stdout.
/// Routes through the [`LlmClient`] port rather than raw HTTP.
pub async fn call_okapi_with_spinner(
    cfg: &russell_doctor::client::ClientConfig,
    model: &str,
    messages: &[serde_json::Value],
) -> std::result::Result<String, String> {
    let expression = THINKING_EXPRESSIONS
        .choose(&mut rand::thread_rng())
        .unwrap_or(&"⏳");

    // Spawn the actual LLM call; receive result via oneshot.
    let (tx, rx) = oneshot::channel();
    let cfg = cfg.clone();
    let model = model.to_string();
    let messages = messages.to_vec();
    tokio::spawn(async move {
        let result = call_llm_via_port(&cfg, &model, &messages).await;
        let _ = tx.send(result);
    });

    let mut rx = rx;
    let mut interval = tokio::time::interval(std::time::Duration::from_millis(180));
    let mut frame_idx = 0usize;

    // Print initial spinner line.
    print!(
        "\r\x1b[KJack → \x1b[1;36m{expression}\x1b[0m {}",
        SPINNER_FRAMES[0]
    );
    std::io::stdout().flush().unwrap();

    loop {
        tokio::select! {
            result = &mut rx => {
                return result.unwrap_or(Err("internal error: channel closed".into()));
            }
            _ = interval.tick() => {
                frame_idx = (frame_idx + 1) % SPINNER_FRAMES.len();
                print!(
                    "\r\x1b[KJack → \x1b[1;36m{expression}\x1b[0m {}",
                    SPINNER_FRAMES[frame_idx]
                );
                std::io::stdout().flush().unwrap();
            }
        }
    }
}

/// Send chat messages through the [`LlmClient`] port.
///
/// Flattens the multi-message array into a [`SoapPrompt`] and calls
/// [`OkapiClient::chat`]. This replaces the old `call_okapi_direct`
/// which bypassed the hexagonal port with raw `reqwest` calls.
async fn call_llm_via_port(
    cfg: &russell_doctor::client::ClientConfig,
    model: &str,
    messages: &[serde_json::Value],
) -> std::result::Result<String, String> {
    let system = messages
        .iter()
        .find(|m| m["role"] == "system")
        .and_then(|m| m["content"].as_str())
        .unwrap_or("")
        .to_string();

    // Flatten conversation into a rendered Markdown block.
    let mut rendered = String::new();
    for msg in messages.iter().filter(|m| m["role"] != "system") {
        let role = msg["role"].as_str().unwrap_or("unknown");
        let content = msg["content"].as_str().unwrap_or("");
        let label = if role == "user" { "User" } else { "Jack" };
        rendered.push_str(&format!("**{label}:** {content}\n\n"));
    }

    let soap = SoapPrompt {
        system,
        subjective: String::new(),
        objective: String::new(),
        rendered: rendered.trim_end().to_string(),
    };

    let mut chat_cfg = cfg.clone();
    chat_cfg.model = model.to_string();
    // Ensure we always point at Okapi.
    if chat_cfg.base_url.is_none() {
        chat_cfg.base_url = Some(russell_doctor::health::DEFAULT_BASE_URL.to_string());
    }
    if chat_cfg.api_key.is_none() {
        chat_cfg.api_key = Some("okapi".into());
    }

    // Shared health pipeline: verify Okapi is reachable, auto-start if needed.
    let base = chat_cfg
        .base_url
        .as_deref()
        .unwrap_or(russell_doctor::health::DEFAULT_BASE_URL);
    if !russell_doctor::health::ensure_ready(base).await {
        return Err("can't reach Okapi (tried auto-start)".into());
    }

    let client = OkapiClient::new(&chat_cfg)
        .await
        .map_err(|e| format!("client error: {e}"))?;

    let resp = client.chat(&soap).await.map_err(|e| format!("{e}"))?;

    Ok(resp.content)
}

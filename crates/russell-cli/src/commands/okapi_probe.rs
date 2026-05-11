// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell okapi-probe` — probe the Okapi inference engine once.
//!
//! Calls Okapi's JSON metrics endpoint, extracts inference health
//! probes, writes them as samples to the journal, evaluates threshold
//! rules, and optionally auto-applies safe management actions.
//!
//! Run on a systemd timer (every 5 min, offset from sentinel-once)
//! for autonomous Okapi supervision.

use anyhow::{Context, Result};
use russell_core::RuleSet;
use russell_core::event::{Event, Scope, Severity};
use russell_core::journal::JournalWriter;
use russell_core::paths::Paths;
use serde::Deserialize;

/// Okapi metrics from `/api/metrics/json`.
#[derive(Debug, Deserialize)]
struct OkapiMetricsResponse {
    tokens_generated_total: i64,
    prompt_tokens_evaluated_total: i64,
    requests_total: i64,
    requests_active: i64,
    errors_total: i64,
    #[serde(default)]
    tokens_per_second: Option<f64>,
    loaded_adapters: i64,
    go_goroutines: i32,
    uptime_seconds: f64,
    model_loaded: bool,
    #[serde(default)]
    model_name: Option<String>,
    #[serde(default)]
    gpu_memory_used_bytes: Option<u64>,
    #[serde(default)]
    gpu_memory_total_bytes: Option<u64>,
}

/// Result of the okapi-probe execution.
#[derive(Debug)]
struct OkapiProbeResult {
    samples: Vec<OkapiSample>,
    metrics: OkapiMetricsResponse,
}

#[derive(Debug)]
struct OkapiSample {
    name: &'static str,
    value: f64,
    unit: &'static str,
}

impl OkapiSample {
    fn new(name: &'static str, value: f64, unit: &'static str) -> Self {
        Self { name, value, unit }
    }
}

fn okapi_base_url() -> String {
    std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "127.0.0.1:11435".to_string())
}

fn okapi_http_url() -> String {
    let base = okapi_base_url();
    if base.starts_with("http") {
        base
    } else {
        format!("http://{base}")
    }
}

fn fetch_metrics(client: &reqwest::blocking::Client, base_url: &str) -> Result<OkapiMetricsResponse> {
    let url = format!("{base_url}/api/metrics/json");
    let resp = client
        .get(&url)
        .send()
        .with_context(|| format!("fetching {url}"))?;

    let status = resp.status();
    let body = resp
        .text()
        .with_context(|| "reading metrics response")?;

    if !status.is_success() {
        anyhow::bail!("metrics endpoint returned {status}: {body}");
    }

    serde_json::from_str::<OkapiMetricsResponse>(&body)
        .with_context(|| "parsing metrics JSON")
}

fn extract_samples(m: &OkapiMetricsResponse) -> Vec<OkapiSample> {
    let mut samples = Vec::new();

    samples.push(OkapiSample::new("okapi_model_loaded", if m.model_loaded { 1.0 } else { 0.0 }, "bool"));

    if let Some(tps) = m.tokens_per_second {
        samples.push(OkapiSample::new("okapi_tokens_per_sec", tps, "t/s"));
    }

    samples.push(OkapiSample::new("okapi_goroutine_count", m.go_goroutines as f64, "count"));
    samples.push(OkapiSample::new("okapi_uptime_hours", m.uptime_seconds / 3600.0, "hours"));
    samples.push(OkapiSample::new("okapi_requests_active", m.requests_active as f64, "count"));
    samples.push(OkapiSample::new("okapi_errors_total", m.errors_total as f64, "count"));
    samples.push(OkapiSample::new("okapi_adapter_count", m.loaded_adapters as f64, "count"));

    if let (Some(used), Some(total)) = (m.gpu_memory_used_bytes, m.gpu_memory_total_bytes) {
        if total > 0 {
            let pct = (used as f64 / total as f64) * 100.0;
            samples.push(OkapiSample::new("okapi_gpu_memory_used_pct", pct, "%"));
            samples.push(OkapiSample::new("okapi_gpu_memory_used_mib", used as f64 / 1_048_576.0, "MiB"));
        }
    }

    samples
}

fn trigger_model_load(client: &reqwest::blocking::Client, base_url: &str, model: &str) -> Result<String> {
    let url = format!("{base_url}/api/generate");
    let body = serde_json::json!({
        "model": model,
        "prompt": "ok",
        "stream": false,
        "options": { "num_predict": 1 }
    });

    let resp = client
        .post(&url)
        .json(&body)
        .send()
        .with_context(|| "triggering model load")?;

    let status = resp.status();
    let resp_body = resp.text().unwrap_or_default();

    if status.is_success() {
        Ok(format!("model load triggered for {model}"))
    } else {
        Ok(format!("model load failed ({status}): {resp_body}"))
    }
}

pub fn run(paths: &Paths, auto_apply: bool, default_model: &str) -> Result<()> {
    let started = std::time::Instant::now();
    let base_url = okapi_http_url();

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .context("building HTTP client")?;

    let journal = JournalWriter::open(&paths.journal())
        .with_context(|| format!("opening journal {}", paths.journal().display()))?;

    let mut rules = RuleSet::with_defaults();
    rules.load_from_dir(&paths.rules());

    // 1. Fetch metrics from Okapi.
    let metrics = match fetch_metrics(&client, &base_url) {
        Ok(m) => m,
        Err(e) => {
            let mut ev = Event::new("observe", Severity::Warn);
            ev.tier = Some("okapi".into());
            ev.module = Some("okapi/connect".into());
            ev.summary = Some(format!("okapi unreachable: {e}"));
            journal.append(&ev)?;

            eprintln!("okapi-probe: {e}");
            return Ok(());
        }
    };

    // 2. Extract probe samples.
    let samples = extract_samples(&metrics);
    let now = russell_core::time::now_unix();

    for s in &samples {
        journal.upsert_sample(now, Scope::Host, s.name, Some(s.value), None, s.unit)?;
    }

    // 3. Evaluate rules and emit threshold events.
    let mut threshold_events: Vec<Event> = Vec::new();
    for s in &samples {
        let severity = rules.evaluate(s.name, s.value);
        if severity > Severity::Info {
            let mut ev = Event::new("threshold_breach", severity);
            ev.tier = Some("okapi".into());
            ev.module = Some(format!("okapi/threshold/{}", s.name));
            ev.summary = Some(format!(
                "{} = {:.1} {} raised {}",
                s.name, s.value, s.unit, severity
            ));
            threshold_events.push(ev);
        }
    }

    for ev in &threshold_events {
        journal.append(ev)?;
    }

    // 4. Auto-apply safe actions.
    let mut actions_taken: Vec<String> = Vec::new();

    if auto_apply {
        // Model not loaded? Trigger a load.
        if !metrics.model_loaded && !default_model.is_empty() {
            match trigger_model_load(&client, &base_url, default_model) {
                Ok(msg) => {
                    actions_taken.push(format!("model_load: {msg}"));
                }
                Err(e) => {
                    actions_taken.push(format!("model_load: failed — {e}"));
                }
            }
        }

        // Too many adapters? Unload them.
        if metrics.loaded_adapters > 4 {
            let mut unloaded = 0u32;
            if let Ok(resp) = client
                .get(format!("{base_url}/api/adapters"))
                .send()
            {
                if let Ok(body) = resp.text() {
                    if let Ok(wrapper) = serde_json::from_str::<serde_json::Value>(&body) {
                        if let Some(adapters) = wrapper["adapters"].as_array() {
                            let model = metrics.model_name.as_deref().unwrap_or("");
                            for adapter in adapters {
                                if let Some(name) = adapter["name"].as_str() {
                                    let unload_body = serde_json::json!({
                                        "model": model,
                                        "name": name
                                    });
                                    match client
                                        .post(format!("{base_url}/api/adapters/unload"))
                                        .json(&unload_body)
                                        .send()
                                    {
                                        Ok(r) if r.status().is_success() => {
                                            unloaded += 1;
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if unloaded > 0 {
                actions_taken.push(format!("unloaded {unloaded} adapter(s)"));
            }
        }
    }

    // 5. Cycle event.
    let mut ev = Event::new("observe", Severity::Info);
    ev.tier = Some("okapi".into());
    ev.module = Some("okapi/cycle".into());
    ev.duration_ms = Some(started.elapsed().as_millis() as u64);
    ev.summary = Some(format!(
        "okapi-probe: {} samples, model={}, {} threshold breaches, {} actions{}",
        samples.len(),
        metrics.model_name.as_deref().unwrap_or("none"),
        threshold_events.len(),
        actions_taken.len(),
        if auto_apply { " (auto-applied)" } else { "" },
    ));
    ev.outputs.insert(
        "sample_count".into(),
        serde_json::Value::from(samples.len() as u64),
    );
    ev.outputs.insert(
        "model_name".into(),
        serde_json::Value::from(metrics.model_name.clone().unwrap_or_default()),
    );
    ev.outputs.insert(
        "model_loaded".into(),
        serde_json::Value::from(metrics.model_loaded),
    );
    journal.append(&ev)?;

    println!(
        "okapi-probe: captured {} samples, {} threshold breaches in {} ms{}",
        samples.len(),
        threshold_events.len(),
        started.elapsed().as_millis(),
        if actions_taken.is_empty() {
            String::new()
        } else {
            format!(" — actions: {}", actions_taken.join(", "))
        },
    );

    Ok(())
}
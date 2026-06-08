// SPDX-License-Identifier: MIT OR Apache-2.0
//! Russell ACP Server binary entry point.

use std::path::PathBuf;
use tracing_subscriber::{fmt, prelude::*};

use russell_acp_server::{
    AcpCnsEmitter, AcpDispatch, AcpHandler, AcpServer, JackPersonaProjection, MacaroonAuth,
    RateLimiter,
};
use russell_core::journal::{JournalReader, JournalWriter};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging.
    tracing_subscriber::registry()
        .with(fmt::layer().with_target(false).with_thread_ids(false))
        .init();

    // Initialize Jack persona.
    let persona = JackPersonaProjection::new()?;

    // Load skills from Russell's skill registry.
    let skills_dir = PathBuf::from(std::env::var("HOME")?).join(".local/share/harness/skills");

    let skills = if skills_dir.exists() {
        russell_skills::load_all(&skills_dir)?.skills
    } else {
        tracing::warn!("skills directory not found: {}", skills_dir.display());
        Vec::new()
    };

    tracing::info!("Loaded {} skills", skills.len());

    // Initialize journal writer.
    let journal_path =
        PathBuf::from(std::env::var("HOME")?).join(".local/state/harness/journal.db");

    let journal = if journal_path.exists() {
        #[allow(clippy::arc_with_non_send_sync)]
        Some(std::sync::Arc::new(JournalWriter::open(&journal_path)?))
    } else {
        tracing::warn!("journal not found, evidence logging disabled");
        None
    };

    // Initialize journal reader for proprioception notifications (T2-2).
    let journal_reader = if journal_path.exists() {
        Some(JournalReader::new(&journal_path))
    } else {
        None
    };

    // Initialize ACP dispatch with loaded skills.
    let dispatch = AcpDispatch::new(skills, skills_dir);
    let dispatch = if let Some(ref journal) = journal {
        dispatch.with_journal(std::sync::Arc::clone(journal))
    } else {
        dispatch
    };

    // Initialize auth (no root key = dev mode, skip validation).
    let macaroon_root_key = std::env::var("RUSSELL_ACP_MACAROON_KEY").ok();
    let dev_mode_allowed = std::env::var("RUSSELL_ACP_DEV_MODE").is_ok();
    let require_auth = std::env::var("RUSSELL_ACP_REQUIRE_AUTH").is_ok();
    let mut auth = MacaroonAuth::new(macaroon_root_key, dev_mode_allowed);
    if let Some(ref journal) = journal {
        auth = auth.with_journal(std::sync::Arc::clone(journal));
    }

    // Initialize inference backend (Okapi local inference).
    let okapi_endpoint =
        std::env::var("OKAPI_ENDPOINT").unwrap_or_else(|_| "http://localhost:11434".to_string());
    let okapi_model = std::env::var("OKAPI_MODEL").unwrap_or_else(|_| "llama3".to_string());
    let okapi = russell_meta::OkapiInferenceAdapter::new(&okapi_endpoint).with_model(&okapi_model);
    let okapi = std::sync::Arc::new(okapi);

    let inference = okapi;

    // Initialize rate limiter.
    let rate_limiter = RateLimiter::default();

    // Initialize CNS emitter for observability (T2-3).
    let cns = AcpCnsEmitter::new("russell-acp-server");

    // Create handler and server.
    let mut handler = AcpHandler::new(persona, dispatch, auth, rate_limiter)
        .with_require_auth(require_auth)
        .with_inference(inference)
        .with_cns(cns);
    if let Some(reader) = journal_reader {
        handler = handler.with_journal_reader(reader);
    }
    let server = AcpServer::new(handler);

    // Serve over stdio.
    server.serve_stdio().await?;

    Ok(())
}

// SPDX-License-Identifier: MIT OR Apache-2.0
//! Russell ACP Server binary entry point.

use std::path::PathBuf;
use tracing_subscriber::{fmt, prelude::*};

use russell_acp_server::{
    AcpDispatch, AcpHandler, AcpServer, JackPersonaProjection, MacaroonAuth, RateLimiter,
};
use russell_core::journal::JournalWriter;

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
        russell_skills::load_all(&skills_dir)?
    } else {
        tracing::warn!("skills directory not found: {}", skills_dir.display());
        Vec::new()
    };

    tracing::info!("Loaded {} skills", skills.len());

    // Initialize journal writer.
    let journal_path =
        PathBuf::from(std::env::var("HOME")?).join(".local/state/harness/journal.db");

    let journal = if journal_path.exists() {
        Some(JournalWriter::open(&journal_path)?)
    } else {
        tracing::warn!("journal not found, evidence logging disabled");
        None
    };

    // Initialize ACP dispatch with loaded skills.
    let dispatch = AcpDispatch::new(skills, skills_dir);
    let dispatch = if let Some(journal) = journal {
        dispatch.with_journal(journal)
    } else {
        dispatch
    };

    // Initialize auth (no root key = dev mode, skip validation).
    let macaroon_root_key = std::env::var("RUSSELL_ACP_MACAROON_KEY").ok();
    let auth = MacaroonAuth::new(macaroon_root_key);

    // Initialize rate limiter.
    let rate_limiter = RateLimiter::default();

    // Create handler and server.
    let handler = AcpHandler::new(persona, dispatch, auth, rate_limiter);
    let server = AcpServer::new(handler);

    // Serve over stdio.
    server.serve_stdio().await?;

    Ok(())
}

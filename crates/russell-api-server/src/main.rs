// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell-api-server` binary entry point.

use anyhow::Result;
use tracing::info;

use russell_api_server::AppState;
use russell_meta::JACK_PERSONA;
use russell_session::SessionEngine;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("russell_api_server=info".parse()?),
        )
        .init();

    let system_prompt = format!(
        "You are Jack, Russell's nurse persona.\n\n\
         {}\n\n\
         API Context:\n\
         - You are interacting via the HTTP REST API\n\
         - Your conversation partner may be an operator or automated client\n\
         - You observe the host, run probes, and recommend actions\n\
         - You NEVER emit shell commands — you rank intervention IDs\n\
         - You propose interventions; the operator consents; the dispatcher executes",
        JACK_PERSONA
    );

    let engine = SessionEngine::new(&system_prompt);
    let state = AppState::new(engine);

    let app = russell_api_server::routes::build_router(state);

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 8421));
    info!("Russell API server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

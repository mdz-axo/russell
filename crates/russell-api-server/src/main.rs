// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell-api-server` binary entry point.

use anyhow::Result;
use tracing::info;

use russell_api_server::AppState;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("russell_api_server=info".parse()?),
        )
        .init();

    let state = AppState::new();
    let app = russell_api_server::routes::build_router(state);

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 8421));
    info!("Russell API server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

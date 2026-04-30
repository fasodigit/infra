// SPDX-License-Identifier: AGPL-3.0-or-later
// terroir-buyer — entry point (skeleton P0, implémentation P3).

use std::net::SocketAddr;

use axum::{Router, http::StatusCode, response::IntoResponse, routing::get};
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .json()
        .init();

    let app = Router::new()
        .route("/health/ready", get(health_ready))
        .route("/health/live", get(health_live));

    let addr = SocketAddr::from(([0, 0, 0, 0], terroir_buyer::HTTP_PORT));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!(
        service = "terroir-buyer",
        version = terroir_buyer::version(),
        bind = %addr,
        "starting HTTP server"
    );

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!(service = "terroir-buyer", "shutdown complete");
    Ok(())
}

async fn health_ready() -> impl IntoResponse {
    (StatusCode::OK, "ready")
}

async fn health_live() -> impl IntoResponse {
    (StatusCode::OK, "live")
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
}

// SPDX-License-Identifier: AGPL-3.0-or-later
// terroir-ussd-simulator — entry point (P0.F : routes mock + admin).
//
// Bind LOOPBACK only (cf. CLAUDE.md §8) — c'est un MOCK des providers
// cloud foreign Hub2/Africa's Talking/Twilio. Cela permet de retarder
// leur intégration jusqu'à P3+ (décision utilisateur Q7) sans bloquer
// les flows mobile / backend qui dépendent du canal USSD.

use std::net::SocketAddr;

use axum::{Router, http::StatusCode, response::IntoResponse, routing::get};
use tracing::info;

use terroir_ussd_simulator::admin;
use terroir_ussd_simulator::providers::{africastalking, hub2, twilio};
use terroir_ussd_simulator::state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .json()
        .init();

    let state = AppState::from_env().await;

    let app = Router::new()
        .nest("/hub2", hub2::router())
        .nest("/africastalking", africastalking::router())
        .nest("/twilio", twilio::router())
        .nest("/admin", admin::router())
        .route("/health/ready", get(health_ready))
        .route("/health/live", get(health_live))
        .with_state(state);

    // Loopback only — cf. CLAUDE.md §8 (admin/dev-tools binds 127.0.0.1).
    let addr = SocketAddr::from(([127, 0, 0, 1], terroir_ussd_simulator::HTTP_PORT));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!(
        service = "terroir-ussd-simulator",
        version = terroir_ussd_simulator::version(),
        bind = %addr,
        "starting HTTP server"
    );

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!(service = "terroir-ussd-simulator", "shutdown complete");
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

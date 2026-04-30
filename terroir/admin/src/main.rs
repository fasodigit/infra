// SPDX-License-Identifier: AGPL-3.0-or-later
// terroir-admin — entry point.
//
// Binds loopback :9904 (port-policy R-03, admin 9900-9999 range).
// P0.A skeleton extended by P0.C with tenant onboarding endpoints.
//
// Environment variables:
//   DATABASE_URL  — Postgres connection string.
//                   MUST include ?statement_cache_capacity=0 for pgbouncer
//                   transaction pooling (no prepared statements).
//                   Example: postgres://terroir_svc:pass@localhost:6432/postgres
//                             ?statement_cache_capacity=0
//   TERROIR_MIGRATIONS_DIR   — absolute path to migrations/tenant-template/
//   TERROIR_WORKSPACE_ROOT   — fallback workspace root (used if MIGRATIONS_DIR unset)

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Context;
use axum::Router;
use tracing::info;

use terroir_admin::{routes::AppState, tenant_template};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .json()
        .init();

    // Postgres connection — statement_cache_capacity=0 for pgbouncer transaction pooling.
    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://postgres:postgres@localhost:5432/postgres?statement_cache_capacity=0"
            .to_string()
    });

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(std::time::Duration::from_secs(5))
        .connect(&database_url)
        .await
        .context("connect to Postgres")?;

    info!(
        service = "terroir-admin",
        version = terroir_admin::version(),
        "connected to Postgres"
    );

    // Resolve tenant migration templates directory.
    let template_dir = tenant_template::resolve_template_dir();
    info!(
        template_dir = %template_dir.display(),
        "tenant migration template directory resolved"
    );

    let state = AppState {
        pool: Arc::new(pool),
        template_dir: Arc::new(template_dir),
    };

    let app: Router = terroir_admin::routes::build_router(state);

    // R-03 (port-policy.yaml): admin-api 9900-9999 = loopback only.
    let addr = SocketAddr::from(([127, 0, 0, 1], terroir_admin::HTTP_PORT));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .context("bind loopback :9904")?;

    info!(
        service = "terroir-admin",
        version = terroir_admin::version(),
        bind = %addr,
        "starting HTTP server"
    );

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!(service = "terroir-admin", "shutdown complete");
    Ok(())
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

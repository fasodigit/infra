// SPDX-License-Identifier: AGPL-3.0-or-later
//! terroir-mobile-bff — entry point.
//!
//! Bootstraps:
//!   - PostgreSQL pool (sqlx, pgbouncer transaction mode, no prepared stmts)
//!   - KAYA RESP3 connection manager (idempotency + rate-limit)
//!   - JWKS cache (auth-ms :8801)
//!   - gRPC client pool to terroir-core :8730 (5 channels by default)
//!   - WebSocket registry (in-process; cross-replica broadcast wired in P1.E)
//!   - Axum HTTP + WebSocket server :8833

use std::{net::SocketAddr, sync::Arc};

use anyhow::Context;
use sqlx::postgres::PgPoolOptions;
use tracing::info;

use terroir_mobile_bff::{
    HTTP_PORT,
    grpc_client::{CoreGrpcPool, DEFAULT_POOL_SIZE},
    routes::build_router,
    state::AppState,
    tenant_context::JwksCache,
    ws::WsRegistry,
};

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

struct Config {
    database_url: String,
    kaya_url: String,
    jwks_uri: String,
    core_grpc_endpoint: String,
    core_grpc_pool_size: usize,
}

impl Config {
    fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            database_url: std::env::var("DATABASE_URL").unwrap_or_else(|_| {
                "postgres://terroir_app@localhost:5432/postgres?statement_cache_capacity=0".into()
            }),
            kaya_url: std::env::var("KAYA_URL").unwrap_or_else(|_| "redis://localhost:6380".into()),
            jwks_uri: std::env::var("JWKS_URI")
                .unwrap_or_else(|_| "http://localhost:8801/.well-known/jwks.json".into()),
            core_grpc_endpoint: std::env::var("TERROIR_CORE_GRPC_ENDPOINT")
                .unwrap_or_else(|_| "http://localhost:8730".into()),
            core_grpc_pool_size: std::env::var("TERROIR_CORE_GRPC_POOL_SIZE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(DEFAULT_POOL_SIZE),
        })
    }
}

// ---------------------------------------------------------------------------
// Bootstrap
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .json()
        .init();

    let cfg = Config::from_env()?;

    // PostgreSQL — no prepared statements (pgbouncer transaction pooling).
    let pg = PgPoolOptions::new()
        .max_connections(10)
        .connect(&cfg.database_url)
        .await
        .context("connect to PostgreSQL")?;
    info!("PostgreSQL pool ready");

    // KAYA RESP3 connection manager.
    let kaya_client = redis::Client::open(cfg.kaya_url.as_str()).context("parse KAYA URL")?;
    let kaya = redis::aio::ConnectionManager::new(kaya_client)
        .await
        .context("connect to KAYA")?;
    info!("KAYA connection manager ready");

    // Shared HTTP client (JWKS fetches).
    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()?;

    // JWKS cache.
    let jwks_cache = JwksCache::new(cfg.jwks_uri.clone());

    // gRPC pool to terroir-core :8730.
    let core_grpc = CoreGrpcPool::new(&cfg.core_grpc_endpoint, cfg.core_grpc_pool_size).await?;

    // WebSocket registry.
    let ws_registry = WsRegistry::new();

    let state = Arc::new(AppState {
        pg: Arc::new(pg),
        kaya,
        jwks_cache,
        http_client,
        core_grpc,
        ws_registry,
    });

    // HTTP + WebSocket server :8833.
    let http_addr = SocketAddr::from(([0, 0, 0, 0], HTTP_PORT));
    let app = build_router(state);
    let listener = tokio::net::TcpListener::bind(http_addr).await?;

    info!(
        service = "terroir-mobile-bff",
        version = terroir_mobile_bff::version(),
        bind = %http_addr,
        "terroir-mobile-bff ready"
    );

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!(service = "terroir-mobile-bff", "shutdown complete");
    Ok(())
}

// ---------------------------------------------------------------------------
// Graceful shutdown
// ---------------------------------------------------------------------------

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

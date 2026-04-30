// SPDX-License-Identifier: AGPL-3.0-or-later
//! terroir-core — entry point.
//!
//! Bootstraps:
//!   - PostgreSQL pool (sqlx, pgbouncer transaction mode, no prepared stmts)
//!   - KAYA RESP3 connection manager
//!   - Vault Transit PII service
//!   - JWKS cache (auth-ms :8801)
//!   - Redpanda event producer (feature "kafka")
//!   - Axum HTTP server :8830
//!   - Tonic gRPC server :8730

use std::{net::SocketAddr, sync::Arc};

use anyhow::Context;
use sqlx::postgres::PgPoolOptions;
use tracing::info;

use terroir_core::{
    GRPC_PORT, HTTP_PORT, grpc::build_server, routes::build_router,
    service::vault_transit::VaultTransitService, state::AppState, tenant_context::JwksCache,
};

// ---------------------------------------------------------------------------
// Config from environment variables
// ---------------------------------------------------------------------------

struct Config {
    database_url: String,
    kaya_url: String,
    vault_addr: String,
    vault_token: String,
    jwks_uri: String,
    #[cfg_attr(not(feature = "kafka"), allow(dead_code))]
    redpanda_brokers: String,
}

impl Config {
    fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            database_url: std::env::var("DATABASE_URL").unwrap_or_else(|_| {
                "postgres://terroir_app@localhost:5432/postgres?statement_cache_capacity=0".into()
            }),
            kaya_url: std::env::var("KAYA_URL").unwrap_or_else(|_| "redis://localhost:6380".into()),
            vault_addr: std::env::var("VAULT_ADDR")
                .unwrap_or_else(|_| "http://localhost:8200".into()),
            vault_token: std::env::var("VAULT_TOKEN").context("VAULT_TOKEN env var required")?,
            jwks_uri: std::env::var("JWKS_URI")
                .unwrap_or_else(|_| "http://localhost:8801/.well-known/jwks.json".into()),
            redpanda_brokers: std::env::var("REDPANDA_BROKERS")
                .unwrap_or_else(|_| "localhost:9092".into()),
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
        .max_connections(20)
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

    // Vault Transit PII service.
    let vault = VaultTransitService::new(cfg.vault_addr.clone(), cfg.vault_token.clone());

    // Shared HTTP client.
    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()?;

    // JWKS cache.
    let jwks_cache = JwksCache::new(cfg.jwks_uri.clone());

    // Redpanda event producer.
    #[cfg(feature = "kafka")]
    let events = {
        let producer = terroir_core::events::EventProducer::new(&cfg.redpanda_brokers)
            .context("create Redpanda producer")?;
        Arc::new(producer)
    };

    let state = Arc::new(AppState {
        pg: Arc::new(pg),
        kaya,
        vault: Arc::new(vault),
        jwks_cache,
        http_client,
        #[cfg(feature = "kafka")]
        events,
    });

    // -- gRPC server :8730 --
    let grpc_addr = SocketAddr::from(([0, 0, 0, 0], GRPC_PORT));
    let grpc_server = build_server(state.clone());

    tokio::spawn(async move {
        info!(bind = %grpc_addr, "starting gRPC server");
        tonic::transport::Server::builder()
            .add_service(grpc_server)
            .serve(grpc_addr)
            .await
            .expect("gRPC server failed");
    });

    // -- HTTP server :8830 --
    let http_addr = SocketAddr::from(([0, 0, 0, 0], HTTP_PORT));
    let app = build_router(state.clone());
    let listener = tokio::net::TcpListener::bind(http_addr).await?;

    info!(
        service = "terroir-core",
        version = terroir_core::version(),
        http = %http_addr,
        grpc = %grpc_addr,
        "terroir-core ready"
    );

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!(service = "terroir-core", "shutdown complete");
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

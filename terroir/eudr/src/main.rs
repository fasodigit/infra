// SPDX-License-Identifier: AGPL-3.0-or-later
//! terroir-eudr — entry point.
//!
//! Bootstraps:
//!   - PostgreSQL pool
//!   - KAYA RESP3 connection manager (validation cache)
//!   - aws-sdk-s3 client pointed at MinIO `geo-mirror`
//!   - Hansen + JRC tile readers (LRU cache)
//!   - Vault PKI HTTP client
//!   - Redpanda producer (feature "kafka")
//!   - Axum :8831 + Tonic :8731

use std::{net::SocketAddr, sync::Arc};

use anyhow::Context;
use aws_sdk_s3::config::Region;
use aws_smithy_types::retry::RetryConfig;
use sqlx::postgres::PgPoolOptions;
use tracing::info;

use terroir_eudr::{
    GEO_MIRROR_BUCKET, GRPC_PORT, HTTP_PORT, grpc_server::build_server, routes::build_router,
    service::hansen_reader::HansenReader, service::jrc_reader::JrcReader, state::AppState,
    state::EudrSettings, tenant_context::JwksCache,
};

struct BootConfig {
    database_url: String,
    kaya_url: String,
    s3_endpoint: String,
    s3_region: String,
    s3_access_key: String,
    s3_secret_key: String,
    jwks_uri: String,
    #[cfg_attr(not(feature = "kafka"), allow(dead_code))]
    redpanda_brokers: String,
}

impl BootConfig {
    fn from_env() -> Self {
        Self {
            database_url: std::env::var("DATABASE_URL").unwrap_or_else(|_| {
                "postgres://terroir_app@localhost:5432/postgres?statement_cache_capacity=0".into()
            }),
            kaya_url: std::env::var("KAYA_URL").unwrap_or_else(|_| "redis://localhost:6380".into()),
            s3_endpoint: std::env::var("S3_ENDPOINT")
                .unwrap_or_else(|_| "http://localhost:9201".into()),
            s3_region: std::env::var("S3_REGION").unwrap_or_else(|_| "bf-ouaga-1".into()),
            s3_access_key: std::env::var("S3_ACCESS_KEY")
                .unwrap_or_else(|_| "faso-dev-access-key".into()),
            s3_secret_key: std::env::var("S3_SECRET_KEY")
                .unwrap_or_else(|_| "faso-dev-secret-key-change-me-32c".into()),
            jwks_uri: std::env::var("JWKS_URI")
                .unwrap_or_else(|_| "http://localhost:8801/.well-known/jwks.json".into()),
            redpanda_brokers: std::env::var("REDPANDA_BROKERS")
                .unwrap_or_else(|_| "localhost:9092".into()),
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .json()
        .init();

    let cfg = BootConfig::from_env();
    let settings = EudrSettings::from_env()?;

    // PostgreSQL
    let pg = PgPoolOptions::new()
        .max_connections(20)
        .connect(&cfg.database_url)
        .await
        .context("connect to PostgreSQL")?;
    info!("PostgreSQL pool ready");

    // KAYA
    let kaya_client = redis::Client::open(cfg.kaya_url.as_str()).context("parse KAYA URL")?;
    let kaya = redis::aio::ConnectionManager::new(kaya_client)
        .await
        .context("connect to KAYA")?;
    info!("KAYA connection manager ready");

    // S3 (MinIO)
    let creds = aws_credential_types::Credentials::new(
        cfg.s3_access_key.clone(),
        cfg.s3_secret_key.clone(),
        None,
        None,
        "terroir-eudr-static",
    );
    let s3_config = aws_sdk_s3::config::Builder::new()
        .endpoint_url(cfg.s3_endpoint.clone())
        .region(Region::new(cfg.s3_region.clone()))
        .credentials_provider(creds)
        .force_path_style(true)
        .retry_config(RetryConfig::standard())
        .build();
    let s3 = aws_sdk_s3::Client::from_conf(s3_config);
    info!(endpoint = %cfg.s3_endpoint, "S3/MinIO client ready");

    let hansen = Arc::new(HansenReader::new(s3.clone(), GEO_MIRROR_BUCKET.to_owned()));
    let jrc = Arc::new(JrcReader::new(s3.clone(), GEO_MIRROR_BUCKET.to_owned()));

    // HTTP client
    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()?;

    // JWKS
    let jwks_cache = JwksCache::new(cfg.jwks_uri.clone());

    // Redpanda
    #[cfg(feature = "kafka")]
    let events = {
        let producer = terroir_eudr::events::EventProducer::new(&cfg.redpanda_brokers)
            .context("create Redpanda producer")?;
        Arc::new(producer)
    };

    let state = Arc::new(AppState {
        pg: Arc::new(pg),
        kaya,
        hansen,
        jrc,
        jwks_cache,
        http_client,
        settings,
        #[cfg(feature = "kafka")]
        events,
    });

    // gRPC :8731
    let grpc_addr = SocketAddr::from(([0, 0, 0, 0], GRPC_PORT));
    let grpc_server = build_server(state.clone());
    tokio::spawn(async move {
        info!(bind = %grpc_addr, "starting gRPC server");
        if let Err(e) = tonic::transport::Server::builder()
            .add_service(grpc_server)
            .serve(grpc_addr)
            .await
        {
            tracing::error!(error = %e, "gRPC server failed");
        }
    });

    // HTTP :8831
    let http_addr = SocketAddr::from(([0, 0, 0, 0], HTTP_PORT));
    let app = build_router(state.clone());
    let listener = tokio::net::TcpListener::bind(http_addr).await?;

    info!(
        service = "terroir-eudr",
        version = terroir_eudr::version(),
        http = %http_addr,
        grpc = %grpc_addr,
        "terroir-eudr ready"
    );

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!(service = "terroir-eudr", "shutdown complete");
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

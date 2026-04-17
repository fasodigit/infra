//! outbox-relay — FASO DIGITALISATION v3.1 (souverain)
//!
//! Bootstrap : lit la configuration depuis les variables d'environnement, ouvre
//! les connexions YugabyteDB / KAYA / Redpanda, démarre N workers (1 task par
//! shard), expose /metrics pour Prometheus, gère l'arrêt gracieux (SIGTERM).
//!
//! Référence : SPEC-OUTBOX-RELAY-v3.1.md §3, §4.

use std::{net::SocketAddr, sync::Arc, time::Duration};

use anyhow::{Context, Result};
use serde::Deserialize;
use tokio::{signal, task::JoinSet};
use tracing::{error, info, warn};
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

mod metrics;
mod publisher;
mod worker;

// -----------------------------------------------------------------------------
// 1. Configuration (12-factor, via env vars)
// -----------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    /// Identifiant du module métier (etat-civil, hospital, e-ticket, ...).
    pub module_name: String,

    /// Identifiant d'instance (0 ou 1 pour HA x2). Combiné à WORKER_BASE pour
    /// déterminer quels shards cette instance prend.
    pub instance_id: u8,

    /// Nombre total de shards (figé à 6 = 3 workers × 2 instances).
    #[serde(default = "default_total_shards")]
    pub total_shards: u8,

    /// Nombre de workers (tasks Tokio) à lancer sur cette instance (3).
    #[serde(default = "default_workers_per_instance")]
    pub workers_per_instance: u8,

    /// DSN YugabyteDB : `postgres://outbox_reader_relay@yb-host:5433/db?sslmode=verify-full`
    pub yugabyte_dsn: String,

    /// Brokers Redpanda : `redpanda-0:9092,redpanda-1:9092,redpanda-2:9092`
    pub redpanda_brokers: String,

    /// URL KAYA : `rediss://kaya-primary:6380`
    pub kaya_url: String,

    /// Port d'exposition Prometheus.
    #[serde(default = "default_metrics_port")]
    pub metrics_port: u16,

    /// Taille du batch SELECT FOR UPDATE SKIP LOCKED.
    #[serde(default = "default_batch_size")]
    pub batch_size: i32,

    /// Intervalle min entre deux polls lorsqu'aucune ligne PENDING.
    #[serde(default = "default_poll_idle_ms")]
    pub poll_idle_ms: u64,

    /// SPIFFE trust domain (ex: `faso.bf`).
    pub spiffe_trust_domain: String,
}

fn default_total_shards()          -> u8  { 6 }
fn default_workers_per_instance()  -> u8  { 3 }
fn default_metrics_port()          -> u16 { 9090 }
fn default_batch_size()            -> i32 { 100 }
fn default_poll_idle_ms()          -> u64 { 250 }

impl Config {
    pub fn from_env() -> Result<Self> {
        envy::prefixed("OUTBOX_")
            .from_env::<Self>()
            .context("failed to read Config from env (OUTBOX_*)")
    }

    /// Calcule les IDs de shards à traiter par cette instance.
    /// Instance 0 -> [0,1,2], instance 1 -> [3,4,5] (pour 6 shards, 2 instances).
    pub fn shard_ids(&self) -> Vec<u8> {
        let base = self.instance_id * self.workers_per_instance;
        (base..base + self.workers_per_instance).collect()
    }
}

// -----------------------------------------------------------------------------
// 2. Initialisation tracing / OTLP / Prometheus
// -----------------------------------------------------------------------------

fn init_telemetry(module: &str) -> Result<()> {
    let fmt_layer = fmt::layer().json();
    let filter_layer = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .init();

    info!(%module, "telemetry initialized");
    Ok(())
}

// -----------------------------------------------------------------------------
// 3. Entry point
// -----------------------------------------------------------------------------

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> Result<()> {
    let cfg = Config::from_env()?;
    init_telemetry(&cfg.module_name)?;

    info!(
        module = %cfg.module_name,
        instance = cfg.instance_id,
        shards = ?cfg.shard_ids(),
        "outbox-relay starting"
    );

    // 3.1 — Registre Prometheus + serveur HTTP /metrics
    let registry = metrics::build_registry()?;
    tokio::spawn(metrics::serve(
        SocketAddr::from(([0, 0, 0, 0], cfg.metrics_port)),
        registry.clone(),
    ));

    // 3.2 — Pool YugabyteDB (role: outbox_reader_relay)
    let db_pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(16)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&cfg.yugabyte_dsn)
        .await
        .context("YugabyteDB connect")?;

    // 3.3 — Client Redpanda (producer idempotent, acks=all)
    let producer = publisher::build_redpanda_producer(&cfg)
        .context("Redpanda producer")?;

    // 3.4 — Client KAYA (RESP3, connection manager)
    let kaya = publisher::build_kaya_client(&cfg).await
        .context("KAYA client")?;

    let cfg = Arc::new(cfg);
    let publisher = Arc::new(publisher::Publisher::new(
        producer,
        kaya,
        cfg.clone(),
    ));

    // 3.5 — Démarrage des N workers (un par shard géré par cette instance)
    let mut tasks = JoinSet::new();
    for shard_id in cfg.shard_ids() {
        let worker = worker::Worker::new(
            shard_id,
            db_pool.clone(),
            publisher.clone(),
            cfg.clone(),
        );
        tasks.spawn(async move {
            if let Err(e) = worker.run().await {
                error!(shard = shard_id, error = ?e, "worker terminated with error");
            }
        });
    }

    // 3.6 — Arrêt gracieux sur SIGTERM / SIGINT
    tokio::select! {
        _ = shutdown_signal() => {
            warn!("shutdown signal received, aborting workers");
            tasks.abort_all();
        }
        _ = tasks.join_next() => {
            warn!("a worker exited unexpectedly; shutting down");
            tasks.abort_all();
        }
    }

    info!("outbox-relay stopped");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async { let _ = signal::ctrl_c().await; };
    #[cfg(unix)]
    let sigterm = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let sigterm = std::future::pending::<()>();

    tokio::select! { _ = ctrl_c => {}, _ = sigterm => {} }
}

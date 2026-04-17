//! metrics.rs — Métriques Prometheus + endpoint HTTP /metrics.
//!
//! Noms figés par SPEC-OUTBOX-RELAY-v3.1 §12.1 :
//!   - outbox_relay_pending_count         (gauge,     scrapé par requête SQL externe)
//!   - outbox_relay_dead_letter_count     (gauge,     idem)
//!   - outbox_relay_lag_seconds           (histogram)
//!   - outbox_relay_publish_duration_ms   (histogram)
//!   - outbox_relay_xadd_duration_ms      (histogram)
//!   - outbox_relay_produce_duration_ms   (histogram)
//!   - outbox_relay_retry_count_total     (counter)
//!   - outbox_relay_worker_up             (gauge)

use std::{net::SocketAddr, sync::OnceLock, time::Duration};

use axum::{Router, extract::State, response::IntoResponse, routing::get};
use chrono::{DateTime, Utc};
use prometheus::{
    CounterVec, Encoder, GaugeVec, HistogramOpts, HistogramVec, Opts, Registry, TextEncoder,
};

struct Handles {
    publish_duration: HistogramVec,
    lag_seconds:      HistogramVec,
    retry_total:      CounterVec,
    dead_letter:      CounterVec,
    sent:             CounterVec,
    batch_size:       GaugeVec,
    batch_failure:    CounterVec,
    worker_up:        GaugeVec,
}

static HANDLES: OnceLock<Handles> = OnceLock::new();

pub fn build_registry() -> anyhow::Result<Registry> {
    let registry = Registry::new();

    let publish_duration = HistogramVec::new(
        HistogramOpts::new(
            "outbox_relay_publish_duration_ms",
            "Durée totale XADD + PRODUCE (ms)",
        )
        .buckets(vec![1.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1_000.0, 2_500.0]),
        &["shard"],
    )?;
    let lag_seconds = HistogramVec::new(
        HistogramOpts::new(
            "outbox_relay_lag_seconds",
            "Latence commit → publish (s)",
        )
        .buckets(vec![0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0, 60.0]),
        &["shard"],
    )?;
    let retry_total = CounterVec::new(
        Opts::new("outbox_relay_retry_count_total", "Nombre de retries"),
        &["shard"],
    )?;
    let dead_letter = CounterVec::new(
        Opts::new("outbox_relay_dead_letter_count", "Lignes passées en DEAD_LETTER"),
        &["shard"],
    )?;
    let sent = CounterVec::new(
        Opts::new("outbox_relay_sent_total", "Lignes publiées avec succès"),
        &["shard"],
    )?;
    let batch_size = GaugeVec::new(
        Opts::new("outbox_relay_batch_size", "Taille du dernier batch lu"),
        &["shard"],
    )?;
    let batch_failure = CounterVec::new(
        Opts::new("outbox_relay_batch_failure_total", "Échecs de batch (erreur SQL ou autre)"),
        &["shard"],
    )?;
    let worker_up = GaugeVec::new(
        Opts::new("outbox_relay_worker_up", "1 si worker vivant, 0 sinon"),
        &["shard"],
    )?;

    registry.register(Box::new(publish_duration.clone()))?;
    registry.register(Box::new(lag_seconds.clone()))?;
    registry.register(Box::new(retry_total.clone()))?;
    registry.register(Box::new(dead_letter.clone()))?;
    registry.register(Box::new(sent.clone()))?;
    registry.register(Box::new(batch_size.clone()))?;
    registry.register(Box::new(batch_failure.clone()))?;
    registry.register(Box::new(worker_up.clone()))?;

    HANDLES.set(Handles {
        publish_duration, lag_seconds, retry_total, dead_letter,
        sent, batch_size, batch_failure, worker_up,
    }).ok();

    Ok(registry)
}

// ----- API utilitaire utilisée par worker.rs -----

pub fn record_publish_duration(shard: u8, d: Duration) {
    if let Some(h) = HANDLES.get() {
        h.publish_duration.with_label_values(&[&shard.to_string()])
            .observe(d.as_secs_f64() * 1_000.0);
    }
}
pub fn record_lag(created_at: DateTime<Utc>) {
    if let Some(h) = HANDLES.get() {
        let lag = (Utc::now() - created_at).num_milliseconds().max(0) as f64 / 1_000.0;
        h.lag_seconds.with_label_values(&["*"]).observe(lag);
    }
}
pub fn inc_retry(shard: u8) {
    if let Some(h) = HANDLES.get() {
        h.retry_total.with_label_values(&[&shard.to_string()]).inc();
    }
}
pub fn inc_dead_letter(shard: u8) {
    if let Some(h) = HANDLES.get() {
        h.dead_letter.with_label_values(&[&shard.to_string()]).inc();
    }
}
pub fn inc_sent(shard: u8) {
    if let Some(h) = HANDLES.get() {
        h.sent.with_label_values(&[&shard.to_string()]).inc();
    }
}
pub fn set_batch_size(shard: u8, size: f64) {
    if let Some(h) = HANDLES.get() {
        h.batch_size.with_label_values(&[&shard.to_string()]).set(size);
    }
}
pub fn inc_batch_failure(shard: u8) {
    if let Some(h) = HANDLES.get() {
        h.batch_failure.with_label_values(&[&shard.to_string()]).inc();
    }
}
pub fn worker_up(shard: u8, up: bool) {
    if let Some(h) = HANDLES.get() {
        h.worker_up.with_label_values(&[&shard.to_string()])
            .set(if up { 1.0 } else { 0.0 });
    }
}

// ----- HTTP server -----

pub async fn serve(addr: SocketAddr, registry: Registry) {
    let app = Router::new()
        .route("/metrics", get(scrape))
        .route("/healthz", get(|| async { "ok" }))
        .with_state(registry);

    let listener = tokio::net::TcpListener::bind(addr).await.expect("bind metrics port");
    tracing::info!(?addr, "metrics server listening");
    let _ = axum::serve(listener, app).await;
}

async fn scrape(State(registry): State<Registry>) -> impl IntoResponse {
    let encoder = TextEncoder::new();
    let metric_families = registry.gather();
    let mut buffer = Vec::new();
    let _ = encoder.encode(&metric_families, &mut buffer);
    (
        [(axum::http::header::CONTENT_TYPE, encoder.format_type())],
        buffer,
    )
}

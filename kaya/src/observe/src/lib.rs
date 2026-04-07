//! KAYA Observability: Prometheus metrics, OpenTelemetry tracing, structured logging.

use thiserror::Error;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum ObserveError {
    #[error("metrics initialization failed: {0}")]
    MetricsInit(String),

    #[error("tracing initialization failed: {0}")]
    TracingInit(String),

    #[error("prometheus error: {0}")]
    Prometheus(#[from] prometheus::Error),
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ObserveConfig {
    pub metrics_enabled: bool,
    pub metrics_port: u16,
    pub tracing_enabled: bool,
    pub otlp_endpoint: String,
    pub log_level: String,
    pub json_logging: bool,
}

impl Default for ObserveConfig {
    fn default() -> Self {
        Self {
            metrics_enabled: true,
            metrics_port: 9100,
            tracing_enabled: false,
            otlp_endpoint: "http://localhost:4317".into(),
            log_level: "info".into(),
            json_logging: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Metrics registry
// ---------------------------------------------------------------------------

/// Core KAYA metrics exported to Prometheus.
pub struct KayaMetrics {
    pub commands_total: prometheus::IntCounterVec,
    pub commands_duration_seconds: prometheus::HistogramVec,
    pub connections_active: prometheus::IntGauge,
    pub connections_total: prometheus::IntCounter,
    pub memory_used_bytes: prometheus::IntGauge,
    pub keys_total: prometheus::IntGauge,
    pub evictions_total: prometheus::IntCounter,
    pub hit_count: prometheus::IntCounter,
    pub miss_count: prometheus::IntCounter,
    pub streams_messages_total: prometheus::IntCounter,
    pub replication_lag_seconds: prometheus::Gauge,
}

impl KayaMetrics {
    /// Create and register all metrics with the default Prometheus registry.
    pub fn new() -> Result<Self, ObserveError> {
        let commands_total = prometheus::register_int_counter_vec!(
            "kaya_commands_total",
            "Total number of commands processed",
            &["command"]
        )?;

        let commands_duration_seconds = prometheus::register_histogram_vec!(
            "kaya_commands_duration_seconds",
            "Command execution duration in seconds",
            &["command"],
            vec![0.00001, 0.00005, 0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05, 0.1]
        )?;

        let connections_active = prometheus::register_int_gauge!(
            "kaya_connections_active",
            "Number of active client connections"
        )?;

        let connections_total = prometheus::register_int_counter!(
            "kaya_connections_total",
            "Total number of client connections accepted"
        )?;

        let memory_used_bytes = prometheus::register_int_gauge!(
            "kaya_memory_used_bytes",
            "Approximate memory used by the store"
        )?;

        let keys_total = prometheus::register_int_gauge!(
            "kaya_keys_total",
            "Total number of keys in the store"
        )?;

        let evictions_total = prometheus::register_int_counter!(
            "kaya_evictions_total",
            "Total number of keys evicted"
        )?;

        let hit_count = prometheus::register_int_counter!(
            "kaya_hits_total",
            "Total number of cache hits"
        )?;

        let miss_count = prometheus::register_int_counter!(
            "kaya_misses_total",
            "Total number of cache misses"
        )?;

        let streams_messages_total = prometheus::register_int_counter!(
            "kaya_streams_messages_total",
            "Total number of stream messages ingested"
        )?;

        let replication_lag_seconds = prometheus::register_gauge!(
            "kaya_replication_lag_seconds",
            "Replication lag in seconds"
        )?;

        Ok(Self {
            commands_total,
            commands_duration_seconds,
            connections_active,
            connections_total,
            memory_used_bytes,
            keys_total,
            evictions_total,
            hit_count,
            miss_count,
            streams_messages_total,
            replication_lag_seconds,
        })
    }
}

// ---------------------------------------------------------------------------
// Logging initialization
// ---------------------------------------------------------------------------

/// Initialize the tracing subscriber with the given configuration.
pub fn init_logging(config: &ObserveConfig) -> Result<(), ObserveError> {
    use tracing_subscriber::prelude::*;
    use tracing_subscriber::EnvFilter;

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&config.log_level));

    let fmt_layer = tracing_subscriber::fmt::layer();

    if config.json_logging {
        let fmt_layer = fmt_layer.json();
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .try_init()
            .map_err(|e| ObserveError::TracingInit(e.to_string()))?;
    } else {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .try_init()
            .map_err(|e| ObserveError::TracingInit(e.to_string()))?;
    }

    Ok(())
}

/// Render all registered Prometheus metrics as text for the `/metrics` endpoint.
pub fn render_metrics() -> String {
    use prometheus::Encoder;
    let encoder = prometheus::TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buf = Vec::new();
    encoder.encode(&metric_families, &mut buf).unwrap();
    String::from_utf8(buf).unwrap()
}

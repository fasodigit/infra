//! Prometheus metrics for the KAYA Pub/Sub protocol.
//!
//! All metrics are namespaced under `kaya_pubsub_`.  They are registered
//! lazily, so importing the module from tests or multiple broker instances
//! does not panic with "duplicate metric".

use std::sync::OnceLock;

use prometheus::{
    register_histogram, register_int_counter, register_int_counter_vec, register_int_gauge,
    register_int_gauge_vec, Histogram, IntCounter, IntCounterVec, IntGauge, IntGaugeVec,
};

/// Centralised handles for all Pub/Sub metrics.
pub struct PubSubMetrics {
    pub subscribers: IntGaugeVec,
    pub channels: IntGauge,
    pub messages_published: IntCounterVec,
    pub dropped_messages: IntCounter,
    pub publish_latency: Histogram,
}

static METRICS: OnceLock<PubSubMetrics> = OnceLock::new();

/// Return the global metrics handle, initialising it on first use.
pub fn metrics() -> &'static PubSubMetrics {
    METRICS.get_or_init(|| PubSubMetrics {
        subscribers: register_int_gauge_vec!(
            "kaya_pubsub_subscribers_gauge",
            "Number of active Pub/Sub subscribers by kind",
            &["kind"]
        )
        .expect("register kaya_pubsub_subscribers_gauge"),
        channels: register_int_gauge!(
            "kaya_pubsub_channels_gauge",
            "Number of active Pub/Sub channels"
        )
        .expect("register kaya_pubsub_channels_gauge"),
        messages_published: register_int_counter_vec!(
            "kaya_pubsub_messages_published_total",
            "Messages published through KAYA Pub/Sub",
            &["kind"]
        )
        .expect("register kaya_pubsub_messages_published_total"),
        dropped_messages: register_int_counter!(
            "kaya_pubsub_dropped_messages_total",
            "Messages dropped due to slow subscribers"
        )
        .expect("register kaya_pubsub_dropped_messages_total"),
        publish_latency: register_histogram!(
            "kaya_pubsub_publish_latency_ms",
            "Publish fan-out latency in milliseconds",
            vec![0.01, 0.05, 0.1, 0.5, 1.0, 5.0, 10.0, 50.0, 100.0]
        )
        .expect("register kaya_pubsub_publish_latency_ms"),
    })
}

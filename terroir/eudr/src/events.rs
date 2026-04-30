// SPDX-License-Identifier: AGPL-3.0-or-later
//! Redpanda event producer for terroir-eudr (feature "kafka").
//!
//! Topics published:
//!   - `terroir.parcel.eudr.validated`
//!   - `terroir.parcel.eudr.rejected`
//!   - `terroir.parcel.eudr.escalated`
//!   - `terroir.dds.generated`
//!   - `terroir.dds.submitted`
//!   - `terroir.dds.rejected`
//!   - `terroir.dds.submitted.dlq` (DLQ for failed TRACES NT submissions)
//!
//! All payloads are JSON. If Redpanda is unavailable, a warning is logged
//! and the operation continues (events are best-effort).

use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct ParcelEudrEvent {
    pub validation_id: Uuid,
    pub parcel_id: Uuid,
    pub tenant_slug: String,
    pub status: String,
    pub deforestation_overlap_ha: f64,
    pub dataset_version: String,
    pub polygon_hash: String,
}

#[derive(Debug, Serialize)]
pub struct DdsEvent {
    pub dds_id: Uuid,
    pub validation_id: Uuid,
    pub tenant_slug: String,
    pub status: String,
    pub payload_sha256: String,
}

#[derive(Debug, Serialize)]
pub struct DdsDlqEvent {
    pub dds_id: Uuid,
    pub tenant_slug: String,
    pub attempt_no: i32,
    pub reason: String,
}

#[cfg(feature = "kafka")]
pub use kafka_impl::EventProducer;

#[cfg(feature = "kafka")]
mod kafka_impl {
    use rdkafka::{
        config::ClientConfig,
        producer::{FutureProducer, FutureRecord},
    };
    use std::time::Duration;
    use tracing::{instrument, warn};

    pub struct EventProducer {
        inner: FutureProducer,
    }

    impl EventProducer {
        pub fn new(brokers: &str) -> anyhow::Result<Self> {
            let producer: FutureProducer = ClientConfig::new()
                .set("bootstrap.servers", brokers)
                .set("message.timeout.ms", "5000")
                .set("enable.idempotence", "true")
                .create()?;
            Ok(Self { inner: producer })
        }

        #[instrument(skip(self, payload), fields(topic = topic))]
        pub async fn publish<T: serde::Serialize>(&self, topic: &str, key: &str, payload: &T) {
            let body = match serde_json::to_vec(payload) {
                Ok(b) => b,
                Err(e) => {
                    warn!(topic = topic, error = %e, "failed to serialize event");
                    return;
                }
            };
            let record = FutureRecord::to(topic).key(key).payload(&body);
            if let Err((e, _)) = self.inner.send(record, Duration::from_secs(5)).await {
                warn!(topic = topic, key = key, error = %e, "Redpanda publish failed");
            }
        }
    }
}

#[cfg(not(feature = "kafka"))]
pub struct EventProducer;

#[cfg(not(feature = "kafka"))]
impl EventProducer {
    pub fn new_noop() -> Self {
        Self
    }
    pub async fn publish<T: serde::Serialize>(&self, topic: &str, key: &str, _payload: &T) {
        tracing::warn!(
            topic = topic,
            key = key,
            "kafka feature disabled — event not published"
        );
    }
}

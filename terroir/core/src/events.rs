// SPDX-License-Identifier: AGPL-3.0-or-later
//! Redpanda event producer for terroir-core (feature "kafka").
//!
//! Topics published:
//!   - `terroir.member.created`
//!   - `terroir.member.updated`
//!   - `terroir.member.deleted`
//!   - `terroir.parcel.created`
//!   - `terroir.parcel.updated`
//!   - `terroir.audit.event`
//!
//! All payloads are JSON. If Redpanda is unavailable, a warning is logged
//! and the operation continues (events are best-effort).

use serde::Serialize;
use tracing::warn;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Event payloads
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct MemberCreatedEvent {
    pub producer_id: Uuid,
    pub cooperative_id: Uuid,
    pub tenant_slug: String,
    pub actor_id: String,
}

#[derive(Debug, Serialize)]
pub struct MemberUpdatedEvent {
    pub producer_id: Uuid,
    pub tenant_slug: String,
    pub actor_id: String,
    pub lww_version: i64,
}

#[derive(Debug, Serialize)]
pub struct MemberDeletedEvent {
    pub producer_id: Uuid,
    pub tenant_slug: String,
    pub actor_id: String,
}

#[derive(Debug, Serialize)]
pub struct ParcelCreatedEvent {
    pub parcel_id: Uuid,
    pub producer_id: Uuid,
    pub tenant_slug: String,
    pub actor_id: String,
}

#[derive(Debug, Serialize)]
pub struct ParcelUpdatedEvent {
    pub parcel_id: Uuid,
    pub tenant_slug: String,
    pub actor_id: String,
    pub lww_version: i64,
}

// ---------------------------------------------------------------------------
// EventProducer (kafka feature only)
// ---------------------------------------------------------------------------

#[cfg(feature = "kafka")]
pub use kafka_impl::EventProducer;

#[cfg(feature = "kafka")]
mod kafka_impl {
    use super::*;
    use rdkafka::{
        config::ClientConfig,
        producer::{FutureProducer, FutureRecord},
    };
    use std::time::Duration;

    /// Wraps a `FutureProducer` to publish typed events.
    pub struct EventProducer {
        inner: FutureProducer,
    }

    impl EventProducer {
        /// Create a producer connecting to the given brokers list.
        pub fn new(brokers: &str) -> anyhow::Result<Self> {
            let producer: FutureProducer = ClientConfig::new()
                .set("bootstrap.servers", brokers)
                .set("message.timeout.ms", "5000")
                .set("enable.idempotence", "true")
                .create()?;
            Ok(Self { inner: producer })
        }

        /// Publish a JSON-serializable event to a topic.
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
                warn!(topic = topic, key = key, error = %e, "Redpanda publish failed (best-effort)");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// No-op stub when kafka feature is disabled
// ---------------------------------------------------------------------------

#[cfg(not(feature = "kafka"))]
pub struct EventProducer;

#[cfg(not(feature = "kafka"))]
impl EventProducer {
    /// No-op constructor for dev without Redpanda.
    pub fn new_noop() -> Self {
        Self
    }

    pub async fn publish<T: serde::Serialize>(&self, topic: &str, key: &str, _payload: &T) {
        warn!(
            topic = topic,
            key = key,
            "kafka feature disabled — event not published"
        );
    }
}

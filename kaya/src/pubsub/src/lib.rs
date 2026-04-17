//! KAYA Pub/Sub protocol: exact, pattern and sharded subscriptions.
//!
//! This crate is part of the sovereign KAYA in-memory database. It provides
//! an async, bounded, drop-on-slow-subscriber broker that powers the
//! `SUBSCRIBE`, `PSUBSCRIBE`, `PUBLISH`, `SSUBSCRIBE` and `SPUBLISH`
//! commands in the RESP3 Pub/Sub surface.
//!
//! Key guarantees:
//! * Slow subscribers cannot stall the broker: message delivery uses
//!   `tokio::sync::mpsc::Sender::try_send` on a bounded channel and
//!   increments `kaya_pubsub_dropped_messages_total` on overflow.
//! * Exact and pattern subscriptions share a single broker; the sharded
//!   variant partitions traffic across independent brokers routed through
//!   an `xxh3_64` consistent-hash ring with virtual nodes.

pub mod broker;
pub mod error;
pub mod message;
pub mod metrics;
pub mod pattern;
pub mod sharded;

pub use broker::{
    Channel, PubSubBroker, PubSubStats, Subscriber, SubscriptionId,
    DEFAULT_SUBSCRIBER_CAPACITY,
};
pub use error::PubSubError;
pub use message::{ClientId, PubSubMessage};
pub use pattern::Pattern;
pub use sharded::ShardedPubSub;

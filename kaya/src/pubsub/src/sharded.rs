//! Sharded Pub/Sub: route channels to independent brokers via consistent hash.
//!
//! The KAYA Pub/Sub protocol exposes `SSUBSCRIBE` and `SPUBLISH` that — unlike
//! the global broker — deliberately partition traffic so that high-fan-in
//! channels do not block others. Each shard owns an independent
//! [`PubSubBroker`]; the routing key is the raw channel name hashed with
//! `xxh3_64` against a virtual-node ring.

use std::collections::BTreeMap;
use std::sync::Arc;

use bytes::Bytes;
use tokio::sync::mpsc;
use tracing::instrument;
use xxhash_rust::xxh3::xxh3_64;

use crate::broker::{PubSubBroker, Subscriber, SubscriptionId, DEFAULT_SUBSCRIBER_CAPACITY};
use crate::message::PubSubMessage;
use crate::metrics::metrics;

/// Number of virtual nodes per shard. 64 is a pragmatic default that yields
/// balanced distribution without bloating the ring for small cluster sizes.
const VIRTUAL_NODES_PER_SHARD: u32 = 64;

/// Consistent-hash ring mapping hashed keys to shard indices.
#[derive(Debug)]
struct HashRing {
    entries: BTreeMap<u64, usize>,
}

impl HashRing {
    fn new(num_shards: usize) -> Self {
        let mut entries = BTreeMap::new();
        for shard in 0..num_shards {
            for v in 0..VIRTUAL_NODES_PER_SHARD {
                let key = format!("kaya-pubsub-shard-{shard}-vnode-{v}");
                let h = xxh3_64(key.as_bytes());
                entries.insert(h, shard);
            }
        }
        Self { entries }
    }

    fn shard_for(&self, key: &[u8]) -> usize {
        if self.entries.is_empty() {
            return 0;
        }
        let h = xxh3_64(key);
        self.entries
            .range(h..)
            .next()
            .map(|(_, s)| *s)
            .unwrap_or_else(|| *self.entries.values().next().expect("non-empty ring"))
    }
}

/// Sharded Pub/Sub front-end.
pub struct ShardedPubSub {
    brokers: Vec<Arc<PubSubBroker>>,
    ring: HashRing,
}

impl ShardedPubSub {
    /// Create `num_shards` independent brokers.
    ///
    /// Panics if `num_shards == 0`.
    pub fn new(num_shards: usize) -> Self {
        assert!(num_shards > 0, "ShardedPubSub requires at least one shard");
        let brokers = (0..num_shards)
            .map(|_| Arc::new(PubSubBroker::new()))
            .collect();
        let ring = HashRing::new(num_shards);
        Self { brokers, ring }
    }

    /// Number of shards in the ring.
    pub fn shard_count(&self) -> usize {
        self.brokers.len()
    }

    /// Resolve the shard index for a channel.
    pub fn shard_index(&self, channel: &[u8]) -> usize {
        self.ring.shard_for(channel)
    }

    /// Resolve the broker responsible for a channel.
    pub fn broker_for(&self, channel: &[u8]) -> &Arc<PubSubBroker> {
        let idx = self.shard_index(channel);
        &self.brokers[idx]
    }

    /// Subscribe via the sharded API (`SSUBSCRIBE`).
    #[instrument(skip(self, sender))]
    pub fn ssubscribe(
        &self,
        channel: Bytes,
        sender: mpsc::Sender<PubSubMessage>,
    ) -> SubscriptionId {
        let broker = self.broker_for(&channel);
        let id = broker.subscribe(channel, sender);
        metrics().subscribers.with_label_values(&["sharded"]).inc();
        id
    }

    /// Convenience helper: allocate a bounded channel and subscribe.
    pub fn ssubscribe_bounded(&self, channel: Bytes, capacity: usize) -> Subscriber {
        let (tx, rx) = mpsc::channel(capacity);
        let id = self.ssubscribe(channel, tx);
        Subscriber { id, receiver: rx }
    }

    /// Publish through the sharded API (`SPUBLISH`).
    #[instrument(skip(self, payload))]
    pub async fn spublish(&self, channel: &[u8], payload: Bytes) -> u64 {
        let broker = self.broker_for(channel);
        let delivered = broker.publish(channel, payload).await;
        metrics()
            .messages_published
            .with_label_values(&["sharded"])
            .inc();
        delivered
    }

    /// Iterate over all shards (read-only).
    pub fn brokers(&self) -> &[Arc<PubSubBroker>] {
        &self.brokers
    }

    /// Default-capacity SSUBSCRIBE helper.
    pub fn ssubscribe_default(&self, channel: Bytes) -> Subscriber {
        self.ssubscribe_bounded(channel, DEFAULT_SUBSCRIBER_CAPACITY)
    }

    /// Aggregate channel names from all shards. Used by `PUBSUB SHARDCHANNELS`.
    pub fn channel_names(&self) -> Vec<bytes::Bytes> {
        let mut names = Vec::new();
        for broker in &self.brokers {
            names.extend(broker.channel_names());
        }
        names
    }
}

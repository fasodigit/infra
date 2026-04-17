//! Core Pub/Sub broker: exact and pattern subscriptions, fan-out publish.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use bytes::Bytes;
use dashmap::DashMap;
use parking_lot::RwLock;
use tokio::sync::mpsc;
use tracing::{debug, instrument, trace};

use crate::error::PubSubError;
use crate::message::PubSubMessage;
use crate::metrics::metrics;
use crate::pattern::Pattern;

/// Default bounded capacity for subscriber channels. Overflow triggers a
/// drop metric increment instead of blocking the broker.
pub const DEFAULT_SUBSCRIBER_CAPACITY: usize = 1024;

/// Opaque identifier returned by subscribe/psubscribe calls, used to
/// unsubscribe later.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SubscriptionId(pub u64);

/// Convenience alias for a channel identifier (raw bytes).
pub type Channel = Bytes;

/// Internal handle kept inside the broker's maps.
#[derive(Clone)]
struct SubscriberHandle {
    id: SubscriptionId,
    sender: mpsc::Sender<PubSubMessage>,
}

/// Subscriber wrapper exposed to callers so they can receive messages.
pub struct Subscriber {
    /// Unique subscription identifier (for `unsubscribe`).
    pub id: SubscriptionId,
    /// Receiving end of the bounded channel.
    pub receiver: mpsc::Receiver<PubSubMessage>,
}

/// Main broker. Cheap to share across threads via `Arc`.
pub struct PubSubBroker {
    channels: DashMap<Channel, Vec<SubscriberHandle>>,
    patterns: DashMap<Pattern, Vec<SubscriberHandle>>,
    /// Reverse index: SubscriptionId -> (is_pattern, key) for O(1) unsubscribe.
    index: RwLock<ahash::AHashMap<SubscriptionId, SubIndex>>,
    next_id: AtomicU64,
    stats: Arc<PubSubStats>,
}

#[derive(Clone)]
enum SubIndex {
    Exact(Channel),
    Pattern(Pattern),
}

/// Runtime counters exposed for `PUBSUB`-style introspection commands.
#[derive(Debug, Default)]
pub struct PubSubStats {
    pub exact_subscribers: AtomicU64,
    pub pattern_subscribers: AtomicU64,
    pub messages_published: AtomicU64,
    pub messages_dropped: AtomicU64,
}

impl Default for PubSubBroker {
    fn default() -> Self {
        Self::new()
    }
}

impl PubSubBroker {
    /// Create an empty broker.
    pub fn new() -> Self {
        Self {
            channels: DashMap::new(),
            patterns: DashMap::new(),
            index: RwLock::new(ahash::AHashMap::new()),
            next_id: AtomicU64::new(1),
            stats: Arc::new(PubSubStats::default()),
        }
    }

    /// Access the shared stats snapshot.
    pub fn stats(&self) -> Arc<PubSubStats> {
        self.stats.clone()
    }

    fn mint_id(&self) -> SubscriptionId {
        SubscriptionId(self.next_id.fetch_add(1, Ordering::Relaxed))
    }

    /// Subscribe to an exact channel. The caller supplies the sender half of
    /// a bounded channel; the broker keeps it until `unsubscribe`.
    #[instrument(skip(self, sender), fields(channel_len = channel.len()))]
    pub fn subscribe(
        &self,
        channel: Channel,
        sender: mpsc::Sender<PubSubMessage>,
    ) -> SubscriptionId {
        let id = self.mint_id();
        let handle = SubscriberHandle {
            id,
            sender,
        };
        let new_channel = {
            let mut entry = self.channels.entry(channel.clone()).or_default();
            let is_new = entry.is_empty();
            entry.push(handle);
            is_new
        };
        self.index.write().insert(id, SubIndex::Exact(channel));

        self.stats
            .exact_subscribers
            .fetch_add(1, Ordering::Relaxed);
        let m = metrics();
        m.subscribers.with_label_values(&["exact"]).inc();
        if new_channel {
            m.channels.inc();
        }
        debug!(?id, "exact subscribe");
        id
    }

    /// Subscribe to a pattern (`PSUBSCRIBE`).
    #[instrument(skip(self, sender))]
    pub fn psubscribe(
        &self,
        pattern: Pattern,
        sender: mpsc::Sender<PubSubMessage>,
    ) -> SubscriptionId {
        let id = self.mint_id();
        let handle = SubscriberHandle {
            id,
            sender,
        };
        {
            let mut entry = self.patterns.entry(pattern.clone()).or_default();
            entry.push(handle);
        }
        self.index.write().insert(id, SubIndex::Pattern(pattern));

        self.stats
            .pattern_subscribers
            .fetch_add(1, Ordering::Relaxed);
        metrics().subscribers.with_label_values(&["pattern"]).inc();
        debug!(?id, "pattern subscribe");
        id
    }

    /// Remove a previous subscription. No-op if `sub_id` is unknown.
    #[instrument(skip(self))]
    pub fn unsubscribe(&self, sub_id: SubscriptionId) {
        let entry = { self.index.write().remove(&sub_id) };
        let Some(entry) = entry else {
            return;
        };
        let m = metrics();
        match entry {
            SubIndex::Exact(channel) => {
                let removed_channel = if let Some(mut subs) = self.channels.get_mut(&channel) {
                    subs.retain(|h| h.id != sub_id);
                    subs.is_empty()
                } else {
                    false
                };
                if removed_channel {
                    self.channels.remove(&channel);
                    m.channels.dec();
                }
                self.stats
                    .exact_subscribers
                    .fetch_sub(1, Ordering::Relaxed);
                m.subscribers.with_label_values(&["exact"]).dec();
            }
            SubIndex::Pattern(pattern) => {
                let removed = if let Some(mut subs) = self.patterns.get_mut(&pattern) {
                    subs.retain(|h| h.id != sub_id);
                    subs.is_empty()
                } else {
                    false
                };
                if removed {
                    self.patterns.remove(&pattern);
                }
                self.stats
                    .pattern_subscribers
                    .fetch_sub(1, Ordering::Relaxed);
                m.subscribers.with_label_values(&["pattern"]).dec();
            }
        }
    }

    /// Publish a message to all exact subscribers of `channel` and all
    /// subscribers whose pattern matches it. Returns the number of receivers
    /// the message was **accepted** by (i.e. excluding drops).
    #[instrument(skip(self, payload), fields(channel_len = channel.len(), payload_len = payload.len()))]
    pub async fn publish(&self, channel: &[u8], payload: Bytes) -> u64 {
        let started = Instant::now();
        let mut delivered: u64 = 0;
        let m = metrics();

        // Exact subscribers.
        if let Some(subs) = self.channels.get(channel) {
            let msg = PubSubMessage::exact(Bytes::copy_from_slice(channel), payload.clone());
            for handle in subs.iter() {
                match handle.sender.try_send(msg.clone()) {
                    Ok(()) => delivered += 1,
                    Err(mpsc::error::TrySendError::Full(_)) => {
                        self.stats.messages_dropped.fetch_add(1, Ordering::Relaxed);
                        m.dropped_messages.inc();
                        trace!(?handle.id, "dropped (full)");
                    }
                    Err(mpsc::error::TrySendError::Closed(_)) => {
                        trace!(?handle.id, "receiver closed");
                    }
                }
            }
        }

        // Pattern subscribers.
        for entry in self.patterns.iter() {
            let pattern = entry.key();
            if !pattern.matches(channel) {
                continue;
            }
            let msg = PubSubMessage::pattern(
                Bytes::copy_from_slice(channel),
                payload.clone(),
                pattern.clone(),
            );
            for handle in entry.value().iter() {
                match handle.sender.try_send(msg.clone()) {
                    Ok(()) => delivered += 1,
                    Err(mpsc::error::TrySendError::Full(_)) => {
                        self.stats.messages_dropped.fetch_add(1, Ordering::Relaxed);
                        m.dropped_messages.inc();
                    }
                    Err(mpsc::error::TrySendError::Closed(_)) => {}
                }
            }
        }

        self.stats
            .messages_published
            .fetch_add(1, Ordering::Relaxed);
        m.messages_published.with_label_values(&["exact"]).inc();
        m.publish_latency
            .observe(started.elapsed().as_secs_f64() * 1_000.0);
        delivered
    }

    /// Number of distinct exact-subscribed channels.
    pub fn channel_count(&self) -> usize {
        self.channels.len()
    }

    /// Collect the names of all channels that currently have at least one
    /// exact subscriber. Used by `PUBSUB CHANNELS [pattern]`.
    pub fn channel_names(&self) -> Vec<bytes::Bytes> {
        self.channels
            .iter()
            .filter(|entry| !entry.value().is_empty())
            .map(|entry| entry.key().clone())
            .collect()
    }

    /// Number of exact subscribers on a given channel.
    pub fn subscriber_count(&self, channel: &[u8]) -> usize {
        self.channels.get(channel).map(|v| v.len()).unwrap_or(0)
    }

    /// Number of distinct pattern subscriptions.
    pub fn pattern_count(&self) -> usize {
        self.patterns.len()
    }

    /// Helper: subscribe with a freshly allocated bounded channel.
    pub fn subscribe_bounded(&self, channel: Channel, capacity: usize) -> Subscriber {
        let (tx, rx) = mpsc::channel(capacity);
        let id = self.subscribe(channel, tx);
        Subscriber {
            id,
            receiver: rx,
        }
    }

    /// Helper: psubscribe with a freshly allocated bounded channel.
    pub fn psubscribe_bounded(
        &self,
        pattern: Pattern,
        capacity: usize,
    ) -> Result<Subscriber, PubSubError> {
        let (tx, rx) = mpsc::channel(capacity);
        let id = self.psubscribe(pattern, tx);
        Ok(Subscriber {
            id,
            receiver: rx,
        })
    }
}

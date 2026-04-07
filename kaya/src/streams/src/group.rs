//! Consumer group: tracks pending entries and consumers.

use std::collections::{BTreeMap, HashMap};

use crate::consumer::Consumer;
use crate::StreamId;

/// A pending entry: tracked until XACK is received.
#[derive(Debug, Clone)]
pub struct PendingEntry {
    pub id: StreamId,
    pub consumer: String,
    pub delivery_count: u64,
    pub delivered_at_ms: u64,
}

/// A consumer group attached to a stream.
#[derive(Debug)]
pub struct ConsumerGroup {
    pub name: String,
    /// The last ID delivered to any consumer in this group.
    pub last_delivered_id: StreamId,
    /// Pending entries (not yet ACKed).
    pending: BTreeMap<StreamId, PendingEntry>,
    /// Registered consumers.
    consumers: HashMap<String, Consumer>,
}

impl ConsumerGroup {
    pub fn new(name: String, start_id: StreamId) -> Self {
        Self {
            name,
            last_delivered_id: start_id,
            pending: BTreeMap::new(),
            consumers: HashMap::new(),
        }
    }

    /// Register a consumer if not already present.
    pub fn ensure_consumer(&mut self, consumer_name: &str) {
        self.consumers
            .entry(consumer_name.to_string())
            .or_insert_with(|| Consumer::new(consumer_name.to_string()));
    }

    /// Mark an entry as pending for a consumer.
    pub fn add_pending(&mut self, id: StreamId, consumer_name: &str) {
        let now_ms = chrono::Utc::now().timestamp_millis() as u64;
        self.pending.insert(
            id,
            PendingEntry {
                id,
                consumer: consumer_name.to_string(),
                delivery_count: 1,
                delivered_at_ms: now_ms,
            },
        );
        if let Some(c) = self.consumers.get_mut(consumer_name) {
            c.pending_count += 1;
            c.touch();
        }
    }

    /// ACK an entry. Returns true if it was pending.
    pub fn ack(&mut self, id: StreamId) -> bool {
        if let Some(pe) = self.pending.remove(&id) {
            if let Some(c) = self.consumers.get_mut(&pe.consumer) {
                c.pending_count = c.pending_count.saturating_sub(1);
            }
            true
        } else {
            false
        }
    }

    /// Number of pending entries.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// List consumers.
    pub fn consumers(&self) -> Vec<&Consumer> {
        self.consumers.values().collect()
    }

    /// Remove a consumer and return the number of pending entries that belonged to it.
    pub fn del_consumer(&mut self, consumer_name: &str) -> u64 {
        let mut removed = 0u64;
        self.pending.retain(|_, pe| {
            if pe.consumer == consumer_name {
                removed += 1;
                false
            } else {
                true
            }
        });
        self.consumers.remove(consumer_name);
        removed
    }
}

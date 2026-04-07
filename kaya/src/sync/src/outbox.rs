//! Outbox pattern: track pending writes for reliable sync to YugabyteDB.

use std::collections::VecDeque;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

/// An outbox entry representing a pending write.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboxEntry {
    pub id: u64,
    pub key: String,
    pub operation: OutboxOp,
    pub payload: Option<Vec<u8>>,
    pub created_at_ms: u64,
    pub attempts: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutboxOp {
    Set,
    Delete,
}

/// Thread-safe outbox queue.
pub struct Outbox {
    queue: Mutex<VecDeque<OutboxEntry>>,
    next_id: Mutex<u64>,
}

impl Outbox {
    pub fn new() -> Self {
        Self {
            queue: Mutex::new(VecDeque::new()),
            next_id: Mutex::new(1),
        }
    }

    /// Enqueue a new outbox entry.
    pub fn push(&self, key: String, op: OutboxOp, payload: Option<Vec<u8>>) -> u64 {
        let mut next_id = self.next_id.lock();
        let id = *next_id;
        *next_id += 1;

        let entry = OutboxEntry {
            id,
            key,
            operation: op,
            payload,
            created_at_ms: chrono::Utc::now().timestamp_millis() as u64,
            attempts: 0,
        };

        self.queue.lock().push_back(entry);
        id
    }

    /// Take a batch of entries for processing.
    pub fn take_batch(&self, max: usize) -> Vec<OutboxEntry> {
        let mut queue = self.queue.lock();
        let n = max.min(queue.len());
        queue.drain(..n).collect()
    }

    /// Re-enqueue entries that failed processing.
    pub fn requeue(&self, mut entries: Vec<OutboxEntry>) {
        let mut queue = self.queue.lock();
        for entry in &mut entries {
            entry.attempts += 1;
        }
        for entry in entries.into_iter().rev() {
            queue.push_front(entry);
        }
    }

    /// Number of pending entries.
    pub fn len(&self) -> usize {
        self.queue.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.lock().is_empty()
    }
}

impl Default for Outbox {
    fn default() -> Self {
        Self::new()
    }
}

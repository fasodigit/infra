//! Consumer within a consumer group.

use std::time::Instant;


/// A consumer within a consumer group.
#[derive(Debug, Clone)]
pub struct Consumer {
    pub name: String,
    /// Number of pending (unacknowledged) entries.
    pub pending_count: u64,
    /// Last time this consumer was active.
    pub last_active: Instant,
}

impl Consumer {
    pub fn new(name: String) -> Self {
        Self {
            name,
            pending_count: 0,
            last_active: Instant::now(),
        }
    }

    pub fn touch(&mut self) {
        self.last_active = Instant::now();
    }

    /// Idle time in milliseconds.
    pub fn idle_ms(&self) -> u64 {
        self.last_active.elapsed().as_millis() as u64
    }
}

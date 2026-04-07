//! Entry and metadata types for stored key-value pairs.

use bytes::Bytes;
use std::time::Instant;

/// Metadata tracked per entry for eviction and expiry.
#[derive(Debug, Clone)]
pub struct EntryMetadata {
    pub created_at: Instant,
    pub last_accessed: Instant,
    pub expires_at: Option<Instant>,
    pub access_count: u64,
    pub size_bytes: usize,
}

/// A stored entry: compressed value + metadata.
#[derive(Debug, Clone)]
pub struct Entry {
    pub value: Bytes,
    pub metadata: EntryMetadata,
}

impl Entry {
    /// Check if this entry has expired.
    pub fn is_expired(&self) -> bool {
        self.metadata
            .expires_at
            .map(|exp| Instant::now() >= exp)
            .unwrap_or(false)
    }

    /// Touch this entry: update last_accessed and bump access_count.
    pub fn touch(&mut self) {
        self.metadata.last_accessed = Instant::now();
        self.metadata.access_count += 1;
    }

    /// Remaining TTL in seconds, or None if no expiry set.
    pub fn remaining_ttl_secs(&self) -> Option<u64> {
        self.metadata.expires_at.map(|exp| {
            let now = Instant::now();
            if exp > now {
                (exp - now).as_secs()
            } else {
                0
            }
        })
    }
}

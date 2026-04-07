//! KAYA Streams: XADD/XREAD/XREADGROUP/XACK/XTRIM implementation.
//!
//! Provides Redis-compatible stream semantics with consumer groups,
//! compaction, and typed entries. Used by event-bus-lib for event distribution.

pub mod consumer;
pub mod entry;
pub mod error;
pub mod group;
pub mod manager;

use std::collections::BTreeMap;

use bytes::Bytes;
use chrono::Utc;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

pub use consumer::Consumer;
pub use entry::StreamEntry;
pub use error::StreamError;
pub use group::ConsumerGroup;
pub use manager::StreamManager;

// ---------------------------------------------------------------------------
// Stream ID
// ---------------------------------------------------------------------------

/// A stream entry ID: `<milliseconds>-<sequence>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct StreamId {
    pub ms: u64,
    pub seq: u64,
}

impl StreamId {
    pub const ZERO: StreamId = StreamId { ms: 0, seq: 0 };

    pub fn new(ms: u64, seq: u64) -> Self {
        Self { ms, seq }
    }

    /// Auto-generate a new ID with the current timestamp.
    pub fn auto_generate(last: Option<StreamId>) -> Self {
        let ms = Utc::now().timestamp_millis() as u64;
        let seq = match last {
            Some(prev) if prev.ms == ms => prev.seq + 1,
            _ => 0,
        };
        Self { ms, seq }
    }

    /// Parse from string like "1234567890-0".
    pub fn parse(s: &str) -> Result<Self, StreamError> {
        if s == "*" {
            return Ok(Self::auto_generate(None));
        }
        let parts: Vec<&str> = s.splitn(2, '-').collect();
        let ms: u64 = parts
            .first()
            .ok_or_else(|| StreamError::InvalidId(s.to_string()))?
            .parse()
            .map_err(|_| StreamError::InvalidId(s.to_string()))?;
        let seq: u64 = if parts.len() > 1 {
            parts[1]
                .parse()
                .map_err(|_| StreamError::InvalidId(s.to_string()))?
        } else {
            0
        };
        Ok(Self { ms, seq })
    }
}

impl std::fmt::Display for StreamId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}", self.ms, self.seq)
    }
}

// ---------------------------------------------------------------------------
// Stream
// ---------------------------------------------------------------------------

/// A single stream: ordered log of entries with consumer groups.
pub struct Stream {
    /// Stream name/key.
    pub name: String,
    /// Entries ordered by ID.
    entries: RwLock<BTreeMap<StreamId, StreamEntry>>,
    /// Consumer groups.
    groups: RwLock<BTreeMap<String, ConsumerGroup>>,
    /// Last generated ID (for auto-ID generation).
    last_id: RwLock<StreamId>,
    /// Max entries (0 = unlimited). Used by XTRIM.
    max_entries: usize,
}

impl Stream {
    pub fn new(name: String, max_entries: usize) -> Self {
        Self {
            name,
            entries: RwLock::new(BTreeMap::new()),
            groups: RwLock::new(BTreeMap::new()),
            last_id: RwLock::new(StreamId::ZERO),
            max_entries,
        }
    }

    /// XADD: append an entry. Returns the generated stream ID.
    pub fn xadd(
        &self,
        id_hint: Option<&str>,
        fields: Vec<(Bytes, Bytes)>,
    ) -> Result<StreamId, StreamError> {
        let last = *self.last_id.read();

        let id = match id_hint {
            Some("*") | None => StreamId::auto_generate(Some(last)),
            Some(s) => {
                let parsed = StreamId::parse(s)?;
                if parsed <= last {
                    return Err(StreamError::IdTooSmall {
                        given: parsed,
                        last,
                    });
                }
                parsed
            }
        };

        let entry = StreamEntry { id, fields };

        let mut entries = self.entries.write();
        entries.insert(id, entry);
        *self.last_id.write() = id;

        // XTRIM if max_entries is set.
        if self.max_entries > 0 && entries.len() > self.max_entries {
            let excess = entries.len() - self.max_entries;
            let keys_to_remove: Vec<StreamId> =
                entries.keys().take(excess).copied().collect();
            for k in keys_to_remove {
                entries.remove(&k);
            }
        }

        Ok(id)
    }

    /// XLEN: number of entries.
    pub fn xlen(&self) -> usize {
        self.entries.read().len()
    }

    /// XREAD: read entries after the given ID.
    pub fn xread(&self, after: StreamId, count: Option<usize>) -> Vec<StreamEntry> {
        let entries = self.entries.read();
        let iter = entries
            .range((std::ops::Bound::Excluded(after), std::ops::Bound::Unbounded));

        match count {
            Some(n) => iter.take(n).map(|(_, e)| e.clone()).collect(),
            None => iter.map(|(_, e)| e.clone()).collect(),
        }
    }

    /// XRANGE: read entries in an ID range (inclusive).
    pub fn xrange(
        &self,
        start: StreamId,
        end: StreamId,
        count: Option<usize>,
    ) -> Vec<StreamEntry> {
        let entries = self.entries.read();
        let iter = entries.range(start..=end);

        match count {
            Some(n) => iter.take(n).map(|(_, e)| e.clone()).collect(),
            None => iter.map(|(_, e)| e.clone()).collect(),
        }
    }

    /// XTRIM: trim stream to at most `max_len` entries.
    pub fn xtrim(&self, max_len: usize) -> usize {
        let mut entries = self.entries.write();
        if entries.len() <= max_len {
            return 0;
        }
        let excess = entries.len() - max_len;
        let keys_to_remove: Vec<StreamId> =
            entries.keys().take(excess).copied().collect();
        for k in &keys_to_remove {
            entries.remove(k);
        }
        keys_to_remove.len()
    }

    // -- consumer group operations ------------------------------------------

    /// XGROUP CREATE: create a consumer group.
    pub fn xgroup_create(
        &self,
        group_name: &str,
        start_id: StreamId,
    ) -> Result<(), StreamError> {
        let mut groups = self.groups.write();
        if groups.contains_key(group_name) {
            return Err(StreamError::GroupExists(group_name.into()));
        }
        groups.insert(
            group_name.to_string(),
            ConsumerGroup::new(group_name.to_string(), start_id),
        );
        Ok(())
    }

    /// XREADGROUP: read entries for a consumer within a group.
    pub fn xreadgroup(
        &self,
        group_name: &str,
        consumer_name: &str,
        count: Option<usize>,
    ) -> Result<Vec<StreamEntry>, StreamError> {
        let mut groups = self.groups.write();
        let group = groups
            .get_mut(group_name)
            .ok_or_else(|| StreamError::GroupNotFound(group_name.into()))?;

        let entries_lock = self.entries.read();
        let pending_entries: Vec<StreamEntry> = entries_lock
            .range((
                std::ops::Bound::Excluded(group.last_delivered_id),
                std::ops::Bound::Unbounded,
            ))
            .take(count.unwrap_or(usize::MAX))
            .map(|(_, e)| e.clone())
            .collect();

        // Track which entries are now pending for this consumer.
        for entry in &pending_entries {
            group.add_pending(entry.id, consumer_name);
            group.last_delivered_id = entry.id;
        }

        // Register the consumer if new.
        group.ensure_consumer(consumer_name);

        Ok(pending_entries)
    }

    /// XACK: acknowledge entries for a consumer group.
    pub fn xack(&self, group_name: &str, ids: &[StreamId]) -> Result<u64, StreamError> {
        let mut groups = self.groups.write();
        let group = groups
            .get_mut(group_name)
            .ok_or_else(|| StreamError::GroupNotFound(group_name.into()))?;

        let mut count = 0u64;
        for id in ids {
            if group.ack(*id) {
                count += 1;
            }
        }
        Ok(count)
    }

    /// XGROUP DELCONSUMER: remove a consumer from a group.
    pub fn xgroup_delconsumer(
        &self,
        group_name: &str,
        consumer_name: &str,
    ) -> Result<u64, StreamError> {
        let mut groups = self.groups.write();
        let group = groups
            .get_mut(group_name)
            .ok_or_else(|| StreamError::GroupNotFound(group_name.into()))?;
        Ok(group.del_consumer(consumer_name))
    }

    /// Last ID in the stream.
    pub fn last_id(&self) -> StreamId {
        *self.last_id.read()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_id_ordering() {
        let a = StreamId::new(100, 0);
        let b = StreamId::new(100, 1);
        let c = StreamId::new(101, 0);
        assert!(a < b);
        assert!(b < c);
    }

    #[test]
    fn xadd_xread() {
        let stream = Stream::new("test".into(), 0);
        let id1 = stream
            .xadd(Some("1-0"), vec![(Bytes::from("k"), Bytes::from("v"))])
            .unwrap();
        let id2 = stream
            .xadd(Some("2-0"), vec![(Bytes::from("k2"), Bytes::from("v2"))])
            .unwrap();

        let entries = stream.xread(StreamId::ZERO, None);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].id, id1);
        assert_eq!(entries[1].id, id2);
    }

    #[test]
    fn consumer_group_basic() {
        let stream = Stream::new("events".into(), 0);
        stream
            .xadd(Some("1-0"), vec![(Bytes::from("a"), Bytes::from("1"))])
            .unwrap();
        stream
            .xadd(Some("2-0"), vec![(Bytes::from("b"), Bytes::from("2"))])
            .unwrap();

        stream
            .xgroup_create("mygroup", StreamId::ZERO)
            .unwrap();

        let entries = stream
            .xreadgroup("mygroup", "consumer-1", Some(10))
            .unwrap();
        assert_eq!(entries.len(), 2);

        let acked = stream
            .xack("mygroup", &[entries[0].id, entries[1].id])
            .unwrap();
        assert_eq!(acked, 2);
    }
}

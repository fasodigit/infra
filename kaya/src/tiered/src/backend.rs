//! Cold backend abstraction layer.
//!
//! Trait `ColdBackend` abstracts over any durable cold-tier storage.
//! Two implementations are provided:
//!
//! - `FjallBackend`: persists to NVMe via fjall LSM-tree.
//! - `MemBackend`: in-memory DashMap backend used in unit tests.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use dashmap::DashMap;

use crate::error::TieredError;

// ---------------------------------------------------------------------------
// Trait
// ---------------------------------------------------------------------------

/// Async-capable cold-tier storage backend.
#[async_trait::async_trait]
pub trait ColdBackend: Send + Sync {
    /// Retrieve a value by key. Returns `None` if the key does not exist.
    async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, TieredError>;

    /// Insert or update a key-value pair.
    async fn set(&self, key: &[u8], value: &[u8]) -> Result<(), TieredError>;

    /// Remove a key. Returns `true` if the key was present.
    async fn delete(&self, key: &[u8]) -> Result<bool, TieredError>;

    /// Check whether a key exists without fetching the value.
    async fn exists(&self, key: &[u8]) -> Result<bool, TieredError>;

    /// Approximate total data size in bytes (may include overhead).
    async fn size_bytes(&self) -> u64;
}

// ---------------------------------------------------------------------------
// FjallBackend
// ---------------------------------------------------------------------------

/// Cold backend backed by fjall (LSM-tree, NVMe-persisted).
///
/// Writes go directly to the partition; fjall journals them and flushes to
/// disk segments in the background.
pub struct FjallBackend {
    keyspace: fjall::Keyspace,
    partition: fjall::Partition,
    /// Running tally of approximate data size (key len + value len).
    approx_bytes: Arc<AtomicU64>,
}

impl FjallBackend {
    /// Open (or create) a fjall keyspace at `path` and use partition named
    /// `partition_name` for cold-tier storage.
    pub fn open(path: impl AsRef<std::path::Path>, partition_name: &str) -> Result<Self, TieredError> {
        let keyspace = fjall::Config::new(path)
            .open()
            .map_err(TieredError::Fjall)?;

        let partition = keyspace
            .open_partition(partition_name, fjall::PartitionCreateOptions::default())
            .map_err(TieredError::Fjall)?;

        // Compute initial approximate size by scanning all keys.
        let mut init_bytes: u64 = 0;
        for item in partition.iter() {
            if let Ok((k, v)) = item {
                init_bytes += k.len() as u64 + v.len() as u64;
            }
        }

        Ok(Self {
            keyspace,
            partition,
            approx_bytes: Arc::new(AtomicU64::new(init_bytes)),
        })
    }

    /// Expose the underlying keyspace (e.g. for persistence operations).
    pub fn keyspace(&self) -> &fjall::Keyspace {
        &self.keyspace
    }
}

#[async_trait::async_trait]
impl ColdBackend for FjallBackend {
    async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, TieredError> {
        let result = self.partition.get(key).map_err(TieredError::Fjall)?;
        Ok(result.map(|v| v.to_vec()))
    }

    async fn set(&self, key: &[u8], value: &[u8]) -> Result<(), TieredError> {
        let key_len = key.len() as u64;
        let val_len = value.len() as u64;

        // Deduct old value size if key already exists.
        if let Some(old) = self.partition.get(key).map_err(TieredError::Fjall)? {
            let old_len = key_len + old.len() as u64;
            let _ = self.approx_bytes.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |cur| {
                Some(cur.saturating_sub(old_len))
            });
        }

        self.partition.insert(key, value).map_err(TieredError::Fjall)?;
        self.approx_bytes.fetch_add(key_len + val_len, Ordering::Relaxed);
        Ok(())
    }

    async fn delete(&self, key: &[u8]) -> Result<bool, TieredError> {
        if let Some(old) = self.partition.get(key).map_err(TieredError::Fjall)? {
            let old_len = key.len() as u64 + old.len() as u64;
            let _ = self.approx_bytes.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |cur| {
                Some(cur.saturating_sub(old_len))
            });
            self.partition.remove(key).map_err(TieredError::Fjall)?;
            return Ok(true);
        }
        Ok(false)
    }

    async fn exists(&self, key: &[u8]) -> Result<bool, TieredError> {
        self.partition.contains_key(key).map_err(TieredError::Fjall)
    }

    async fn size_bytes(&self) -> u64 {
        self.approx_bytes.load(Ordering::Relaxed)
    }
}

// ---------------------------------------------------------------------------
// MemBackend (test / dev)
// ---------------------------------------------------------------------------

/// In-memory cold backend using DashMap. Not persistent; suitable for tests.
pub struct MemBackend {
    data: DashMap<Vec<u8>, Vec<u8>, ahash::RandomState>,
}

impl MemBackend {
    pub fn new() -> Self {
        Self {
            data: DashMap::with_hasher(ahash::RandomState::with_seeds(1, 2, 3, 4)),
        }
    }
}

impl Default for MemBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ColdBackend for MemBackend {
    async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, TieredError> {
        Ok(self.data.get(key).map(|v| v.clone()))
    }

    async fn set(&self, key: &[u8], value: &[u8]) -> Result<(), TieredError> {
        self.data.insert(key.to_vec(), value.to_vec());
        Ok(())
    }

    async fn delete(&self, key: &[u8]) -> Result<bool, TieredError> {
        Ok(self.data.remove(key).is_some())
    }

    async fn exists(&self, key: &[u8]) -> Result<bool, TieredError> {
        Ok(self.data.contains_key(key))
    }

    async fn size_bytes(&self) -> u64 {
        self.data.iter().map(|r| (r.key().len() + r.value().len()) as u64).sum()
    }
}

//! KAYA Store: Sharded in-memory KV engine.
//!
//! Core data store with DashMap per shard, eviction policies (LRU/LFU/TTL),
//! arena allocation, and compression via kaya-compress.

pub mod entry;
pub mod eviction;
pub mod shard;
pub mod bloom;
pub mod error;
pub mod types;

use std::time::{Duration, Instant};

use bytes::Bytes;
use serde::{Deserialize, Serialize};

pub use entry::{Entry, EntryMetadata};
pub use eviction::{EvictionPolicy, EvictionManager};
pub use shard::Shard;
pub use bloom::{BloomFilter, BloomManager};
pub use error::StoreError;
pub use types::KayaValue;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreConfig {
    pub num_shards: usize,
    pub eviction_policy: EvictionPolicyKind,
    pub max_memory_per_shard: usize,
    pub max_memory: usize,
    pub arena_block_size: usize,
    pub default_ttl: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EvictionPolicyKind {
    Lru,
    Lfu,
    Ttl,
    None,
}

impl Default for StoreConfig {
    fn default() -> Self {
        Self {
            num_shards: 64,
            eviction_policy: EvictionPolicyKind::Lru,
            max_memory_per_shard: 0,
            max_memory: 0,
            arena_block_size: 65536,
            default_ttl: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

/// The main key-value store. Thread-safe, sharded by key hash.
pub struct Store {
    shards: Vec<Shard>,
    config: StoreConfig,
    compressor: kaya_compress::Compressor,
    started_at: Instant,
}

impl Store {
    pub fn new(config: StoreConfig, compress_config: kaya_compress::CompressConfig) -> Self {
        let n = config.num_shards.max(1);
        let shards = (0..n)
            .map(|id| Shard::new(id, config.eviction_policy))
            .collect();
        Self {
            shards,
            config,
            compressor: kaya_compress::Compressor::new(compress_config),
            started_at: Instant::now(),
        }
    }

    // -- shard routing ------------------------------------------------------

    fn shard_index(&self, key: &[u8]) -> usize {
        let hash = ahash::RandomState::with_seeds(1, 2, 3, 4)
            .hash_one(key);
        (hash as usize) % self.shards.len()
    }

    fn shard(&self, key: &[u8]) -> &Shard {
        &self.shards[self.shard_index(key)]
    }

    // -- basic operations ---------------------------------------------------

    /// GET: retrieve a value by key.
    pub fn get(&self, key: &[u8]) -> Result<Option<Bytes>, StoreError> {
        let shard = self.shard(key);
        match shard.get(key) {
            Some(entry) => {
                if entry.is_expired() {
                    shard.remove(key);
                    return Ok(None);
                }
                // Decompress the stored value
                let raw = self.compressor.decompress(&entry.value)
                    .map_err(|e| StoreError::Compression(e.to_string()))?;
                Ok(Some(raw))
            }
            None => Ok(None),
        }
    }

    /// SET: store a key-value pair with optional TTL (seconds).
    pub fn set(&self, key: &[u8], value: &[u8], ttl: Option<u64>) -> Result<(), StoreError> {
        let compressed = self.compressor.compress(value)
            .map_err(|e| StoreError::Compression(e.to_string()))?;

        let expires_at = ttl
            .or(if self.config.default_ttl > 0 {
                Some(self.config.default_ttl)
            } else {
                None
            })
            .map(|secs| Instant::now() + Duration::from_secs(secs));

        let entry = Entry {
            value: compressed,
            metadata: EntryMetadata {
                created_at: Instant::now(),
                last_accessed: Instant::now(),
                expires_at,
                access_count: 0,
                size_bytes: value.len(),
            },
        };

        self.shard(key).insert(key, entry);
        Ok(())
    }

    /// DEL: remove one or more keys. Returns the number of keys deleted.
    pub fn del(&self, keys: &[&[u8]]) -> u64 {
        let mut count = 0u64;
        for key in keys {
            if self.shard(key).remove(key).is_some() {
                count += 1;
            }
        }
        count
    }

    /// EXISTS: returns how many of the given keys exist.
    pub fn exists(&self, keys: &[&[u8]]) -> u64 {
        keys.iter()
            .filter(|k| {
                self.shard(k)
                    .get(k)
                    .map(|e| !e.is_expired())
                    .unwrap_or(false)
            })
            .count() as u64
    }

    /// EXPIRE: set TTL on an existing key. Returns true if key exists.
    pub fn expire(&self, key: &[u8], seconds: u64) -> bool {
        self.shard(key).set_expiry(key, Duration::from_secs(seconds))
    }

    /// TTL: returns remaining time-to-live in seconds.
    /// -1 if no TTL, -2 if key does not exist.
    pub fn ttl(&self, key: &[u8]) -> i64 {
        match self.shard(key).get(key) {
            None => -2,
            Some(entry) => match entry.metadata.expires_at {
                None => -1,
                Some(exp) => {
                    let now = Instant::now();
                    if exp <= now {
                        -2
                    } else {
                        (exp - now).as_secs() as i64
                    }
                }
            },
        }
    }

    /// PERSIST: remove the TTL on a key. Returns true if the timeout was removed.
    pub fn persist(&self, key: &[u8]) -> bool {
        self.shard(key).remove_expiry(key)
    }

    /// INCR: increment integer value by 1. Returns new value.
    pub fn incr(&self, key: &[u8]) -> Result<i64, StoreError> {
        self.incr_by(key, 1)
    }

    /// DECR: decrement integer value by 1. Returns new value.
    pub fn decr(&self, key: &[u8]) -> Result<i64, StoreError> {
        self.incr_by(key, -1)
    }

    /// INCRBY: increment integer value by `delta`. Returns new value.
    pub fn incr_by(&self, key: &[u8], delta: i64) -> Result<i64, StoreError> {
        let shard = self.shard(key);
        shard.incr_by(key, delta, &self.compressor)
    }

    /// MGET: get multiple keys at once.
    pub fn mget(&self, keys: &[&[u8]]) -> Vec<Option<Bytes>> {
        keys.iter()
            .map(|k| self.get(k).ok().flatten())
            .collect()
    }

    /// MSET: set multiple key-value pairs.
    pub fn mset(&self, pairs: &[(&[u8], &[u8])]) -> Result<(), StoreError> {
        for (k, v) in pairs {
            self.set(k, v, None)?;
        }
        Ok(())
    }

    // -- set operations (SADD / SISMEMBER / SMEMBERS / SREM) ----------------

    /// SADD: add members to a set. Returns number of new members added.
    pub fn sadd(&self, key: &[u8], members: &[&[u8]]) -> Result<u64, StoreError> {
        self.shard(key).sadd(key, members)
    }

    /// SISMEMBER: check if a member exists in a set.
    pub fn sismember(&self, key: &[u8], member: &[u8]) -> bool {
        self.shard(key).sismember(key, member)
    }

    /// SMEMBERS: return all members of a set.
    pub fn smembers(&self, key: &[u8]) -> Vec<Bytes> {
        self.shard(key).smembers(key)
    }

    /// SREM: remove members from a set. Returns number of members removed.
    pub fn srem(&self, key: &[u8], members: &[&[u8]]) -> u64 {
        self.shard(key).srem(key, members)
    }

    /// SCARD: return cardinality of a set.
    pub fn scard(&self, key: &[u8]) -> usize {
        self.shard(key).scard(key)
    }

    // -- sorted set operations ------------------------------------------------

    /// ZADD: add members with scores to a sorted set. Returns count of new members.
    pub fn zadd(&self, key: &[u8], members: &[(f64, &[u8])]) -> u64 {
        self.shard(key).zadd(key, members)
    }

    /// ZREM: remove members from a sorted set. Returns count removed.
    pub fn zrem(&self, key: &[u8], members: &[&[u8]]) -> u64 {
        self.shard(key).zrem(key, members)
    }

    /// ZSCORE: get the score of a member.
    pub fn zscore(&self, key: &[u8], member: &[u8]) -> Option<f64> {
        self.shard(key).zscore(key, member)
    }

    /// ZCARD: number of members in a sorted set.
    pub fn zcard(&self, key: &[u8]) -> usize {
        self.shard(key).zcard(key)
    }

    /// ZRANGE: return members by index range (ascending by score).
    pub fn zrange(&self, key: &[u8], start: i64, stop: i64) -> Vec<(f64, Bytes)> {
        self.shard(key).zrange(key, start, stop)
    }

    /// ZRANGEBYSCORE: return members with score in [min, max].
    pub fn zrangebyscore(
        &self,
        key: &[u8],
        min: f64,
        max: f64,
        limit: Option<usize>,
    ) -> Vec<(f64, Bytes)> {
        self.shard(key).zrangebyscore(key, min, max, limit)
    }

    /// FLUSHDB: remove all keys from all shards.
    pub fn flush(&self) {
        for shard in &self.shards {
            shard.flush();
        }
    }

    // -- info ---------------------------------------------------------------

    /// Total number of keys across all shards.
    pub fn key_count(&self) -> usize {
        self.shards.iter().map(|s| s.len()).sum()
    }

    /// Uptime in seconds.
    pub fn uptime_secs(&self) -> u64 {
        self.started_at.elapsed().as_secs()
    }

    /// Number of shards.
    pub fn num_shards(&self) -> usize {
        self.shards.len()
    }

    pub fn config(&self) -> &StoreConfig {
        &self.config
    }

    /// Run eviction across all shards.
    pub fn run_eviction(&self) {
        for shard in &self.shards {
            shard.evict_expired();
        }
    }
}

impl Default for Store {
    fn default() -> Self {
        Self::new(StoreConfig::default(), kaya_compress::CompressConfig::default())
    }
}

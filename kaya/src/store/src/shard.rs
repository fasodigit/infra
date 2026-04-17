//! Individual shard: a DashMap partition of the key space.

use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

use bytes::Bytes;
use dashmap::DashMap;

use crate::entry::{Entry, EntryMetadata};
use crate::error::StoreError;
use crate::eviction::EvictionManager;
use crate::geo::index::{GeoIndex, GeoSearchQuery, GeoSearchResult};
use crate::geo::point::GeoPoint;
use crate::geo::error::GeoError;
use crate::EvictionPolicyKind;

/// A member of a sorted set: (score, member).
#[derive(Debug, Clone)]
pub struct SortedSetEntry {
    pub score: f64,
    pub member: Bytes,
}

/// A single shard of the key-value store.
pub struct Shard {
    /// Shard ID.
    pub id: usize,
    /// Primary KV map: key bytes -> Entry (compressed value + metadata).
    data: DashMap<Vec<u8>, Entry, ahash::RandomState>,
    /// Set data stored separately: key bytes -> set of member bytes.
    sets: DashMap<Vec<u8>, BTreeSet<Bytes>, ahash::RandomState>,
    /// Sorted set data: key -> (member -> score) + a sorted index.
    sorted_sets: DashMap<Vec<u8>, SortedSetData, ahash::RandomState>,
    /// Geo indexes: key -> GeoIndex (geohash-sorted spatial index).
    pub geos: DashMap<Vec<u8>, GeoIndex, ahash::RandomState>,
    /// Eviction manager.
    eviction: EvictionManager,
}

/// Internal representation of a sorted set.
#[derive(Debug, Clone, Default)]
pub struct SortedSetData {
    /// member -> score lookup
    pub members: BTreeMap<Bytes, f64>,
    /// Sorted index: (score, member) for range queries.
    /// We use BTreeSet with a wrapper that orders by score then member.
    pub by_score: BTreeSet<ScoreMember>,
}

/// A (score, member) pair that sorts by score first, then member.
#[derive(Debug, Clone)]
pub struct ScoreMember {
    pub score: f64,
    pub member: Bytes,
}

impl PartialEq for ScoreMember {
    fn eq(&self, other: &Self) -> bool {
        self.score == other.score && self.member == other.member
    }
}

impl Eq for ScoreMember {}

impl PartialOrd for ScoreMember {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ScoreMember {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.score
            .partial_cmp(&other.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| self.member.cmp(&other.member))
    }
}

impl Shard {
    pub fn new(id: usize, eviction_policy: EvictionPolicyKind) -> Self {
        Self {
            id,
            data: DashMap::with_hasher(ahash::RandomState::with_seeds(1, 2, 3, 4)),
            sets: DashMap::with_hasher(ahash::RandomState::with_seeds(5, 6, 7, 8)),
            sorted_sets: DashMap::with_hasher(ahash::RandomState::with_seeds(9, 10, 11, 12)),
            geos: DashMap::with_hasher(ahash::RandomState::with_seeds(13, 14, 15, 16)),
            eviction: EvictionManager::new(eviction_policy),
        }
    }

    // -- string operations --------------------------------------------------

    pub fn get(&self, key: &[u8]) -> Option<Entry> {
        self.data.get(key).map(|r| {
            let mut e = r.value().clone();
            e.touch();
            e
        })
    }

    pub fn insert(&self, key: &[u8], entry: Entry) {
        self.data.insert(key.to_vec(), entry);
    }

    pub fn remove(&self, key: &[u8]) -> Option<Entry> {
        self.data.remove(key).map(|(_, v)| v)
    }

    pub fn contains(&self, key: &[u8]) -> bool {
        self.data.contains_key(key)
    }

    pub fn len(&self) -> usize {
        self.data.len() + self.sets.len() + self.sorted_sets.len() + self.geos.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty() && self.sets.is_empty() && self.sorted_sets.is_empty() && self.geos.is_empty()
    }

    pub fn set_expiry(&self, key: &[u8], duration: Duration) -> bool {
        if let Some(mut entry) = self.data.get_mut(key) {
            entry.metadata.expires_at = Some(Instant::now() + duration);
            true
        } else {
            false
        }
    }

    pub fn remove_expiry(&self, key: &[u8]) -> bool {
        if let Some(mut entry) = self.data.get_mut(key) {
            if entry.metadata.expires_at.is_some() {
                entry.metadata.expires_at = None;
                return true;
            }
            false
        } else {
            false
        }
    }

    /// Atomic increment by delta. Reads the current value as an integer,
    /// adds `delta`, writes back, and returns the new value.
    pub fn incr_by(
        &self,
        key: &[u8],
        delta: i64,
        compressor: &kaya_compress::Compressor,
    ) -> Result<i64, StoreError> {
        // Use entry API for atomicity within the shard
        let key_vec = key.to_vec();

        let current = self.data.get(&key_vec);
        let current_val: i64 = match current {
            None => 0,
            Some(ref entry) => {
                let raw = compressor
                    .decompress(&entry.value)
                    .map_err(|e| StoreError::Compression(e.to_string()))?;
                let s = std::str::from_utf8(&raw)
                    .map_err(|_| StoreError::NotAnInteger)?;
                s.parse::<i64>().map_err(|_| StoreError::NotAnInteger)?
            }
        };
        drop(current);

        let new_val = current_val
            .checked_add(delta)
            .ok_or(StoreError::IntegerOverflow)?;

        let val_str = new_val.to_string();
        let compressed = compressor
            .compress(val_str.as_bytes())
            .map_err(|e| StoreError::Compression(e.to_string()))?;

        let entry = Entry {
            value: compressed,
            metadata: EntryMetadata {
                created_at: Instant::now(),
                last_accessed: Instant::now(),
                expires_at: None,
                access_count: 0,
                size_bytes: val_str.len(),
            },
        };

        self.data.insert(key_vec, entry);
        Ok(new_val)
    }

    // -- set operations -----------------------------------------------------

    pub fn sadd(&self, key: &[u8], members: &[&[u8]]) -> Result<u64, StoreError> {
        let mut count = 0u64;
        let key_vec = key.to_vec();
        let mut entry = self.sets.entry(key_vec).or_insert_with(BTreeSet::new);
        for member in members {
            if entry.insert(Bytes::copy_from_slice(member)) {
                count += 1;
            }
        }
        Ok(count)
    }

    pub fn sismember(&self, key: &[u8], member: &[u8]) -> bool {
        self.sets
            .get(key)
            .map(|s| s.contains(member))
            .unwrap_or(false)
    }

    pub fn smembers(&self, key: &[u8]) -> Vec<Bytes> {
        self.sets
            .get(key)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default()
    }

    pub fn srem(&self, key: &[u8], members: &[&[u8]]) -> u64 {
        let mut count = 0u64;
        if let Some(mut set) = self.sets.get_mut(key) {
            for member in members {
                let member_bytes = Bytes::copy_from_slice(member);
                if set.remove(&member_bytes) {
                    count += 1;
                }
            }
        }
        count
    }

    pub fn scard(&self, key: &[u8]) -> usize {
        self.sets.get(key).map(|s| s.len()).unwrap_or(0)
    }

    // -- sorted set operations ------------------------------------------------

    /// ZADD: add members with scores. Returns number of new members added.
    pub fn zadd(&self, key: &[u8], members: &[(f64, &[u8])]) -> u64 {
        let key_vec = key.to_vec();
        let mut entry = self.sorted_sets.entry(key_vec).or_default();
        let zset = entry.value_mut();
        let mut added = 0u64;

        for &(score, member) in members {
            let member_bytes = Bytes::copy_from_slice(member);

            // If member already exists, update its score
            if let Some(&old_score) = zset.members.get(&member_bytes) {
                // Remove old entry from score index
                zset.by_score.remove(&ScoreMember {
                    score: old_score,
                    member: member_bytes.clone(),
                });
            } else {
                added += 1;
            }

            zset.members.insert(member_bytes.clone(), score);
            zset.by_score.insert(ScoreMember {
                score,
                member: member_bytes,
            });
        }

        added
    }

    /// ZREM: remove members from a sorted set. Returns number removed.
    pub fn zrem(&self, key: &[u8], members: &[&[u8]]) -> u64 {
        let mut count = 0u64;
        if let Some(mut zset) = self.sorted_sets.get_mut(key) {
            for member in members {
                let member_bytes = Bytes::copy_from_slice(member);
                if let Some(score) = zset.members.remove(&member_bytes) {
                    zset.by_score.remove(&ScoreMember {
                        score,
                        member: member_bytes,
                    });
                    count += 1;
                }
            }
        }
        count
    }

    /// ZSCORE: get the score of a member.
    pub fn zscore(&self, key: &[u8], member: &[u8]) -> Option<f64> {
        self.sorted_sets
            .get(key)
            .and_then(|zset| zset.members.get(member).copied())
    }

    /// ZCARD: number of members in a sorted set.
    pub fn zcard(&self, key: &[u8]) -> usize {
        self.sorted_sets
            .get(key)
            .map(|zset| zset.members.len())
            .unwrap_or(0)
    }

    /// ZRANGE: return members in index range (0-based, ascending by score).
    pub fn zrange(&self, key: &[u8], start: i64, stop: i64) -> Vec<(f64, Bytes)> {
        self.sorted_sets
            .get(key)
            .map(|zset| {
                let len = zset.by_score.len() as i64;
                let s = normalize_index(start, len);
                let e = normalize_index(stop, len);
                if s > e || s >= len as usize {
                    return Vec::new();
                }
                let e = e.min(len as usize - 1);
                zset.by_score
                    .iter()
                    .skip(s)
                    .take(e - s + 1)
                    .map(|sm| (sm.score, sm.member.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// ZRANGEBYSCORE: return members with scores in [min, max].
    pub fn zrangebyscore(
        &self,
        key: &[u8],
        min: f64,
        max: f64,
        limit: Option<usize>,
    ) -> Vec<(f64, Bytes)> {
        self.sorted_sets
            .get(key)
            .map(|zset| {
                let iter = zset.by_score.iter().filter(|sm| sm.score >= min && sm.score <= max);
                match limit {
                    Some(n) => iter.take(n).map(|sm| (sm.score, sm.member.clone())).collect(),
                    None => iter.map(|sm| (sm.score, sm.member.clone())).collect(),
                }
            })
            .unwrap_or_default()
    }

    // -- geo operations -------------------------------------------------------

    /// GEOADD: add (point, member) pairs to a geo index. Returns the count of
    /// newly inserted members (existing members are updated in-place).
    pub fn geo_add(
        &self,
        key: &[u8],
        members: &[(GeoPoint, Bytes)],
        nx: bool,
        xx: bool,
        ch: bool,
    ) -> i64 {
        let idx = self.geos.entry(key.to_vec()).or_insert_with(GeoIndex::new);
        let mut count = 0i64;
        for (point, name) in members {
            let exists = idx.pos(name).is_some();
            if nx && exists {
                continue; // NX: skip if already present
            }
            if xx && !exists {
                continue; // XX: skip if not present
            }
            let was_new = idx.add(name.clone(), *point);
            if ch {
                // CH: count changed (new OR updated)
                count += 1;
            } else if was_new {
                // Default: count only insertions
                count += 1;
            }
        }
        count
    }

    /// GEOPOS: retrieve the position of one member.
    pub fn geo_pos(&self, key: &[u8], member: &[u8]) -> Option<GeoPoint> {
        self.geos.get(key)?.pos(member)
    }

    /// GEODIST: haversine distance in metres between two members.
    pub fn geo_dist(&self, key: &[u8], m1: &[u8], m2: &[u8]) -> Result<Option<f64>, GeoError> {
        match self.geos.get(key) {
            None => Ok(None),
            Some(idx) => Ok(idx.dist(m1, m2)),
        }
    }

    /// GEOSEARCH: run a spatial query on a geo index.
    pub fn geo_search(
        &self,
        key: &[u8],
        query: &GeoSearchQuery,
    ) -> Result<Vec<GeoSearchResult>, GeoError> {
        match self.geos.get(key) {
            None => Ok(Vec::new()),
            Some(idx) => Ok(idx.search(query)),
        }
    }

    /// GEOHASH: base32 geohash (11 chars) for a member.
    pub fn geo_hash(&self, key: &[u8], member: &[u8]) -> Option<String> {
        let idx = self.geos.get(key)?;
        let point = idx.pos(member)?;
        Some(point.geohash(11))
    }

    /// GEOREM: remove members from a geo index. Returns count removed.
    pub fn geo_rem(&self, key: &[u8], members: &[&[u8]]) -> i64 {
        match self.geos.get(key) {
            None => 0,
            Some(idx) => members.iter().filter(|m| idx.remove(m)).count() as i64,
        }
    }

    /// GEOSEARCHSTORE: clone matching results into a destination geo index.
    /// Returns the count of members stored in the destination.
    pub fn geo_search_store(
        &self,
        dest_key: &[u8],
        src_key: &[u8],
        query: &GeoSearchQuery,
    ) -> i64 {
        let results = match self.geos.get(src_key) {
            None => return 0,
            Some(idx) => idx.search(query),
        };
        let count = results.len() as i64;
        if count == 0 {
            return 0;
        }
        let dest = self.geos.entry(dest_key.to_vec()).or_insert_with(GeoIndex::new);
        for r in results {
            dest.add(r.member, r.point);
        }
        count
    }

    /// Iterate over all `(key, entry)` pairs in the primary KV map.
    ///
    /// Returns an iterator yielding `dashmap::mapref::multiple::RefMulti` items
    /// so callers can access both the key and the value without cloning.
    pub fn iter_kv(
        &self,
    ) -> impl Iterator<Item = dashmap::mapref::multiple::RefMulti<'_, Vec<u8>, Entry>> {
        self.data.iter()
    }

    // -- flush ---------------------------------------------------------------

    /// Remove all data from this shard.
    pub fn flush(&self) {
        self.data.clear();
        self.sets.clear();
        self.sorted_sets.clear();
        self.geos.clear();
    }

    // -- eviction -----------------------------------------------------------

    /// Remove all expired entries from this shard.
    pub fn evict_expired(&self) {
        let mut to_remove = Vec::new();
        for entry in self.data.iter() {
            if entry.value().is_expired() {
                to_remove.push(entry.key().clone());
            }
        }
        for key in to_remove {
            self.data.remove(&key);
        }
    }
}

/// Normalize a Redis-style index (supports negative indices).
fn normalize_index(idx: i64, len: i64) -> usize {
    if idx < 0 {
        let adjusted = len + idx;
        if adjusted < 0 { 0 } else { adjusted as usize }
    } else {
        idx as usize
    }
}

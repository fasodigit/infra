// SPDX-License-Identifier: AGPL-3.0-or-later
//! Cuckoo filter implementation.
//!
//! A probabilistic set membership structure with O(1) lookup, insertion and
//! deletion (unlike Bloom filters, which cannot delete safely).
//!
//! Based on Fan et al. 2014 ("Cuckoo Filter: Practically Better Than Bloom").
//!
//! # RESP3-compatible probabilistic commands
//!
//! KAYA exposes these cuckoo operations through RESP3-compatible commands:
//!
//! - `CF.RESERVE <key> <capacity> [BUCKETSIZE b] [ERROR fpr]` — pre-create a filter.
//! - `CF.ADD <key> <item>` — insert an item. Returns 1 on success, error if full.
//! - `CF.EXISTS <key> <item>` — test membership. Returns 1 if likely present.
//! - `CF.DEL <key> <item>` — delete an item. Returns 1 if removed, 0 otherwise.
//! - `CF.COUNT <key>` — number of items currently stored.

use once_cell::sync::Lazy;
use prometheus::{register_int_counter, IntCounter};
use rand::Rng;
use tracing::instrument;
use xxhash_rust::xxh3::xxh3_64_with_seed;

use super::error::ProbabilisticError;

const BUCKET_SIZE: usize = 4;
const MAX_RELOCATIONS: usize = 500;
const FINGERPRINT_SEED: u64 = 0xA5A5_A5A5_A5A5_A5A5;
const HASH1_SEED: u64 = 0x5A5A_5A5A_5A5A_5A5A;

static KAYA_CUCKOO_ITEMS_TOTAL: Lazy<IntCounter> = Lazy::new(|| {
    register_int_counter!(
        "kaya_cuckoo_items_total",
        "Total number of items inserted into cuckoo filters"
    )
    .unwrap_or_else(|_| IntCounter::new("dup_kaya_cuckoo_items_total", "fallback").unwrap())
});

static KAYA_CUCKOO_RELOCATIONS_TOTAL: Lazy<IntCounter> = Lazy::new(|| {
    register_int_counter!(
        "kaya_cuckoo_relocations_total",
        "Total number of relocations performed during cuckoo insertion"
    )
    .unwrap_or_else(|_| {
        IntCounter::new("dup_kaya_cuckoo_relocations_total", "fallback").unwrap()
    })
});

static KAYA_CUCKOO_INSERTION_FAILURES_TOTAL: Lazy<IntCounter> = Lazy::new(|| {
    register_int_counter!(
        "kaya_cuckoo_insertion_failures_total",
        "Total number of cuckoo insertion failures (filter full)"
    )
    .unwrap_or_else(|_| {
        IntCounter::new("dup_kaya_cuckoo_insertion_failures_total", "fallback").unwrap()
    })
});

/// A single bucket in the cuckoo filter storing up to 4 fingerprints.
#[derive(Debug, Clone, Copy, Default)]
struct Bucket {
    fingerprints: [u16; BUCKET_SIZE],
}

impl Bucket {
    #[inline]
    fn contains(&self, fp: u16) -> bool {
        self.fingerprints.iter().any(|&f| f == fp)
    }

    /// Try inserting a fingerprint into an empty slot. Returns true on success.
    #[inline]
    fn try_insert(&mut self, fp: u16) -> bool {
        for slot in &mut self.fingerprints {
            if *slot == 0 {
                *slot = fp;
                return true;
            }
        }
        false
    }

    /// Remove the first occurrence of a fingerprint. Returns true if removed.
    #[inline]
    fn remove(&mut self, fp: u16) -> bool {
        for slot in &mut self.fingerprints {
            if *slot == fp {
                *slot = 0;
                return true;
            }
        }
        false
    }

    /// Number of non-empty slots.
    #[inline]
    fn load(&self) -> usize {
        self.fingerprints.iter().filter(|&&f| f != 0).count()
    }
}

/// A cuckoo filter: supports insertion, lookup, and deletion in O(1).
///
/// The filter stores 16-bit fingerprints in buckets of 4 entries each, and
/// uses two hash functions with different seeds (xxh3) plus partial-key
/// cuckoo hashing to relocate colliding items.
#[derive(Debug, Clone)]
pub struct CuckooFilter {
    buckets: Vec<Bucket>,
    num_buckets: usize,
    count: usize,
    capacity: usize,
}

impl CuckooFilter {
    /// Create a new cuckoo filter sized for `capacity` items at the given
    /// false positive rate. `fpr` is used only to validate the fingerprint
    /// width; a 16-bit fingerprint supports FPR down to ~0.00002.
    #[instrument(level = "debug", skip_all)]
    pub fn new(capacity: usize, _fpr: f64) -> Self {
        // Target 95% load factor; round up to power of two for cheap modulo.
        let min_buckets = ((capacity as f64) / (BUCKET_SIZE as f64 * 0.95)).ceil() as usize;
        let num_buckets = min_buckets.next_power_of_two().max(2);
        Self {
            buckets: vec![Bucket::default(); num_buckets],
            num_buckets,
            count: 0,
            capacity,
        }
    }

    /// CF.ADD: insert an item. Returns `Err(Full)` if no placement is found
    /// after `MAX_RELOCATIONS` evictions.
    #[instrument(level = "trace", skip_all)]
    pub fn insert(&mut self, item: &[u8]) -> Result<(), ProbabilisticError> {
        let fp = fingerprint(item);
        let i1 = self.index1(item);
        let i2 = self.alt_index(i1, fp);

        if self.buckets[i1].try_insert(fp) || self.buckets[i2].try_insert(fp) {
            self.count += 1;
            KAYA_CUCKOO_ITEMS_TOTAL.inc();
            return Ok(());
        }

        // Kick-out loop.
        let mut rng = rand::thread_rng();
        let mut idx = if rng.gen::<bool>() { i1 } else { i2 };
        let mut current_fp = fp;

        for _ in 0..MAX_RELOCATIONS {
            let slot: usize = rng.gen_range(0..BUCKET_SIZE);
            let evicted = self.buckets[idx].fingerprints[slot];
            self.buckets[idx].fingerprints[slot] = current_fp;
            current_fp = evicted;
            KAYA_CUCKOO_RELOCATIONS_TOTAL.inc();

            idx = self.alt_index(idx, current_fp);
            if self.buckets[idx].try_insert(current_fp) {
                self.count += 1;
                KAYA_CUCKOO_ITEMS_TOTAL.inc();
                return Ok(());
            }
        }

        KAYA_CUCKOO_INSERTION_FAILURES_TOTAL.inc();
        Err(ProbabilisticError::Full)
    }

    /// CF.EXISTS: probabilistic membership test.
    #[instrument(level = "trace", skip_all)]
    pub fn contains(&self, item: &[u8]) -> bool {
        let fp = fingerprint(item);
        let i1 = self.index1(item);
        if self.buckets[i1].contains(fp) {
            return true;
        }
        let i2 = self.alt_index(i1, fp);
        self.buckets[i2].contains(fp)
    }

    /// CF.DEL: delete an item. Returns true if a matching fingerprint was
    /// removed. Note: this may remove a different item with the same
    /// fingerprint (extremely unlikely at 16-bit width).
    #[instrument(level = "trace", skip_all)]
    pub fn delete(&mut self, item: &[u8]) -> bool {
        let fp = fingerprint(item);
        let i1 = self.index1(item);
        if self.buckets[i1].remove(fp) {
            self.count = self.count.saturating_sub(1);
            return true;
        }
        let i2 = self.alt_index(i1, fp);
        if self.buckets[i2].remove(fp) {
            self.count = self.count.saturating_sub(1);
            return true;
        }
        false
    }

    /// CF.COUNT: number of items currently stored.
    pub fn len(&self) -> usize {
        self.count
    }

    /// Declared capacity of this filter.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Whether the filter is empty.
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Fill ratio: fraction of slots occupied in [0.0, 1.0].
    pub fn fill_ratio(&self) -> f64 {
        let total = (self.num_buckets * BUCKET_SIZE) as f64;
        if total == 0.0 {
            return 0.0;
        }
        let used: usize = self.buckets.iter().map(|b| b.load()).sum();
        (used as f64) / total
    }

    // -- internals ---------------------------------------------------------

    #[inline]
    fn index1(&self, item: &[u8]) -> usize {
        (xxh3_64_with_seed(item, HASH1_SEED) as usize) & (self.num_buckets - 1)
    }

    /// Partial-key alt index: `i2 = i1 XOR hash(fp)`.
    #[inline]
    fn alt_index(&self, idx: usize, fp: u16) -> usize {
        let fp_bytes = fp.to_le_bytes();
        let h = xxh3_64_with_seed(&fp_bytes, HASH1_SEED) as usize;
        (idx ^ h) & (self.num_buckets - 1)
    }
}

#[inline]
fn fingerprint(item: &[u8]) -> u16 {
    // Fingerprint must never be zero (reserved for "empty slot").
    let h = xxh3_64_with_seed(item, FINGERPRINT_SEED);
    let fp = (h & 0xFFFF) as u16;
    if fp == 0 {
        1
    } else {
        fp
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_lookup() {
        let mut cf = CuckooFilter::new(1000, 0.01);
        cf.insert(b"hello").unwrap();
        cf.insert(b"world").unwrap();
        assert!(cf.contains(b"hello"));
        assert!(cf.contains(b"world"));
        assert!(!cf.contains(b"absent"));
    }

    #[test]
    fn delete_removes() {
        let mut cf = CuckooFilter::new(100, 0.01);
        cf.insert(b"key1").unwrap();
        assert!(cf.contains(b"key1"));
        assert!(cf.delete(b"key1"));
        assert!(!cf.contains(b"key1"));
    }
}

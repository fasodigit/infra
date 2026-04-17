// SPDX-License-Identifier: AGPL-3.0-or-later
//! Probabilistic data structures for KAYA.
//!
//! Provides RESP3-compatible probabilistic commands:
//! - Cuckoo filter (`CF.*`) — set membership with deletions.
//! - HyperLogLog++ (`PF*`) — cardinality estimation.
//! - Count-Min Sketch (`CMS.*`) — approximate frequency counting.
//! - HeavyKeeper TopK (`TOPK.*`) — heavy-hitter tracking.

pub mod cms;
pub mod cuckoo;
pub mod error;
pub mod hyperloglog;
pub mod topk;

pub use cms::CountMinSketch;
pub use cuckoo::CuckooFilter;
pub use error::ProbabilisticError;
pub use hyperloglog::HyperLogLog;
pub use topk::TopK;

use dashmap::DashMap;
use parking_lot::RwLock;

// ---------------------------------------------------------------------------
// ProbabilisticStore — named-filter registry
// ---------------------------------------------------------------------------

/// Thread-safe registry for all probabilistic structures.
///
/// Holds named instances of Cuckoo filters, HyperLogLogs, Count-Min Sketches,
/// and TopK trackers.  Intended to be shared via `Arc<ProbabilisticStore>`
/// from `CommandContext`, following the same pattern as `BloomManager`.
pub struct ProbabilisticStore {
    /// Named cuckoo filters (`CF.*` commands).
    pub cuckoos: DashMap<Vec<u8>, CuckooFilter>,
    /// Named HyperLogLog estimators (`PF*` commands).
    pub hlls: DashMap<Vec<u8>, HyperLogLog>,
    /// Named Count-Min Sketches (`CMS.*` commands).
    pub cms_sketches: DashMap<Vec<u8>, CountMinSketch>,
    /// Named TopK trackers (`TOPK.*` commands).
    pub topks: DashMap<Vec<u8>, RwLock<TopK>>,
}

impl std::fmt::Debug for ProbabilisticStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProbabilisticStore")
            .field("cuckoos", &self.cuckoos.len())
            .field("hlls", &self.hlls.len())
            .field("cms_sketches", &self.cms_sketches.len())
            .field("topks", &self.topks.len())
            .finish()
    }
}

impl Default for ProbabilisticStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ProbabilisticStore {
    /// Create an empty probabilistic store.
    pub fn new() -> Self {
        Self {
            cuckoos: DashMap::new(),
            hlls: DashMap::new(),
            cms_sketches: DashMap::new(),
            topks: DashMap::new(),
        }
    }

    // -- Cuckoo filter API --------------------------------------------------

    /// CF.RESERVE: pre-create a cuckoo filter with the given capacity.
    pub fn cf_reserve(&self, key: &[u8], capacity: u64) {
        let cf = CuckooFilter::new(capacity as usize, 0.01);
        self.cuckoos.insert(key.to_vec(), cf);
    }

    /// CF.ADD: insert an item into the named cuckoo filter.
    /// Auto-creates with 10 000 capacity if the key does not exist.
    /// Returns `true` if the item was inserted, `false` on filter full.
    pub fn cf_add(&self, key: &[u8], item: &[u8]) -> bool {
        let mut entry = self
            .cuckoos
            .entry(key.to_vec())
            .or_insert_with(|| CuckooFilter::new(10_000, 0.01));
        entry.insert(item).is_ok()
    }

    /// CF.ADDNX: insert only if the item does NOT already exist.
    /// Returns `true` if the item was new and inserted.
    pub fn cf_addnx(&self, key: &[u8], item: &[u8]) -> bool {
        let mut entry = self
            .cuckoos
            .entry(key.to_vec())
            .or_insert_with(|| CuckooFilter::new(10_000, 0.01));
        if entry.contains(item) {
            return false;
        }
        entry.insert(item).is_ok()
    }

    /// CF.EXISTS: probabilistic membership test.
    pub fn cf_exists(&self, key: &[u8], item: &[u8]) -> bool {
        self.cuckoos
            .get(key)
            .map(|cf| cf.contains(item))
            .unwrap_or(false)
    }

    /// CF.DEL: delete an item. Returns `true` if a fingerprint was removed.
    pub fn cf_del(&self, key: &[u8], item: &[u8]) -> bool {
        self.cuckoos
            .get_mut(key)
            .map(|mut cf| cf.delete(item))
            .unwrap_or(false)
    }

    /// CF.COUNT: number of items stored in the filter.
    pub fn cf_count(&self, key: &[u8]) -> u64 {
        self.cuckoos
            .get(key)
            .map(|cf| cf.len() as u64)
            .unwrap_or(0)
    }

    /// CF.MEXISTS: membership test for multiple items.
    pub fn cf_mexists(&self, key: &[u8], items: &[&[u8]]) -> Vec<bool> {
        match self.cuckoos.get(key) {
            Some(cf) => items.iter().map(|item| cf.contains(item)).collect(),
            None => vec![false; items.len()],
        }
    }

    // -- HyperLogLog API ----------------------------------------------------

    /// PFADD: add items to the named HLL. Returns `true` if the estimated
    /// cardinality changed.
    pub fn pf_add(&self, key: &[u8], items: &[&[u8]]) -> bool {
        let mut entry = self
            .hlls
            .entry(key.to_vec())
            .or_insert_with(HyperLogLog::default);
        let before = entry.count();
        for item in items {
            entry.add(item);
        }
        entry.count() != before
    }

    /// PFCOUNT: estimated cardinality of the union across the given keys.
    pub fn pf_count(&self, keys: &[&[u8]]) -> u64 {
        if keys.is_empty() {
            return 0;
        }
        if keys.len() == 1 {
            return self
                .hlls
                .get(keys[0])
                .map(|hll| hll.count())
                .unwrap_or(0);
        }
        // Multi-key: merge into a temporary HLL then estimate.
        let mut union = HyperLogLog::default();
        for key in keys {
            if let Some(hll) = self.hlls.get(*key) {
                union.merge(&hll);
            }
        }
        union.count()
    }

    /// PFMERGE: merge source HLLs into `dest` (union semantics).
    pub fn pf_merge(&self, dest: &[u8], srcs: &[&[u8]]) {
        let mut dest_entry = self
            .hlls
            .entry(dest.to_vec())
            .or_insert_with(HyperLogLog::default);
        for src_key in srcs {
            if let Some(src) = self.hlls.get(*src_key) {
                dest_entry.merge(&src);
            }
        }
    }

    // -- Count-Min Sketch API -----------------------------------------------

    /// CMS.INITBYDIM: create a CMS with explicit width and depth.
    pub fn cms_initbydim(
        &self,
        key: &[u8],
        width: usize,
        depth: usize,
    ) -> Result<(), ProbabilisticError> {
        let cms = CountMinSketch::try_new_with_dimensions(width, depth)?;
        self.cms_sketches.insert(key.to_vec(), cms);
        Ok(())
    }

    /// CMS.INITBYPROB: create a CMS from error bound and failure probability.
    pub fn cms_initbyprob(&self, key: &[u8], epsilon: f64, delta: f64) {
        let cms = CountMinSketch::new(epsilon, delta);
        self.cms_sketches.insert(key.to_vec(), cms);
    }

    /// CMS.INCRBY: increment multiple `(item, count)` pairs.
    pub fn cms_incrby(&self, key: &[u8], items: &[(&[u8], u64)]) {
        let mut entry = self
            .cms_sketches
            .entry(key.to_vec())
            .or_insert_with(|| CountMinSketch::new_with_dimensions(2048, 7));
        for (item, count) in items {
            entry.increment(item, *count);
        }
    }

    /// CMS.QUERY: estimate frequency for multiple items.
    pub fn cms_query(&self, key: &[u8], items: &[&[u8]]) -> Vec<u64> {
        match self.cms_sketches.get(key) {
            Some(cms) => items.iter().map(|item| cms.estimate(item)).collect(),
            None => vec![0; items.len()],
        }
    }

    /// CMS.MERGE: weighted merge of source sketches into `dest`.
    /// The destination must already exist; all dimensions must match.
    pub fn cms_merge(
        &self,
        dest: &[u8],
        srcs: &[&[u8]],
        weights: &[f64],
    ) -> Result<(), ProbabilisticError> {
        let Some(mut dest_entry) = self.cms_sketches.get_mut(dest) else {
            return Err(ProbabilisticError::InvalidDimensions(
                "destination CMS does not exist".into(),
            ));
        };
        for (i, src_key) in srcs.iter().enumerate() {
            let Some(src) = self.cms_sketches.get(*src_key) else {
                continue;
            };
            let w = weights.get(i).copied().unwrap_or(1.0);
            if (w - 1.0).abs() < f64::EPSILON {
                dest_entry.merge(&src);
            } else {
                let mut scaled = src.clone();
                scaled.scale_by(w);
                dest_entry.merge(&scaled);
            }
        }
        Ok(())
    }

    // -- TopK API -----------------------------------------------------------

    /// TOPK.RESERVE: create a TopK tracker.
    pub fn topk_reserve(
        &self,
        key: &[u8],
        k: usize,
        width: usize,
        depth: usize,
        decay: f64,
    ) {
        let topk = TopK::new(k, width, depth, decay);
        self.topks.insert(key.to_vec(), RwLock::new(topk));
    }

    /// TOPK.ADD: insert items. Returns items evicted from the top-k list.
    pub fn topk_add(&self, key: &[u8], items: &[&[u8]]) -> Vec<String> {
        let mut evicted = Vec::new();
        let entry = self
            .topks
            .entry(key.to_vec())
            .or_insert_with(|| RwLock::new(TopK::new(10, 1024, 5, 0.9)));
        let mut topk = entry.write();
        for item in items {
            if let Some(e) = topk.add(item) {
                evicted.push(String::from_utf8_lossy(&e).into_owned());
            }
        }
        evicted
    }

    /// TOPK.INCRBY: insert items with explicit weights. Returns evicted items.
    pub fn topk_incrby(&self, key: &[u8], items: &[(&[u8], u64)]) -> Vec<String> {
        let mut evicted = Vec::new();
        let entry = self
            .topks
            .entry(key.to_vec())
            .or_insert_with(|| RwLock::new(TopK::new(10, 1024, 5, 0.9)));
        let mut topk = entry.write();
        for (item, count) in items {
            if let Some(e) = topk.add_with_count(item, *count) {
                evicted.push(String::from_utf8_lossy(&e).into_owned());
            }
        }
        evicted
    }

    /// TOPK.QUERY: returns whether each item is currently in the top-k.
    pub fn topk_query(&self, key: &[u8], items: &[&[u8]]) -> Vec<bool> {
        match self.topks.get(key) {
            Some(entry) => {
                let topk = entry.read();
                items.iter().map(|item| topk.query(item) > 0).collect()
            }
            None => vec![false; items.len()],
        }
    }

    /// TOPK.COUNT: estimated sketch count for each item.
    pub fn topk_count(&self, key: &[u8], items: &[&[u8]]) -> Vec<u64> {
        match self.topks.get(key) {
            Some(entry) => {
                let topk = entry.read();
                items.iter().map(|item| topk.query(item)).collect()
            }
            None => vec![0; items.len()],
        }
    }

    /// TOPK.LIST: current top-k items sorted by count (descending).
    pub fn topk_list(&self, key: &[u8]) -> Vec<(String, u64)> {
        match self.topks.get(key) {
            Some(entry) => {
                let topk = entry.read();
                topk.list()
                    .into_iter()
                    .map(|(bytes, count)| {
                        (String::from_utf8_lossy(&bytes).into_owned(), count)
                    })
                    .collect()
            }
            None => vec![],
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn ps() -> ProbabilisticStore {
        ProbabilisticStore::new()
    }

    // -- Cuckoo tests -------------------------------------------------------

    #[test]
    fn cf_add_then_exists() {
        let store = ps();
        assert!(store.cf_add(b"myfilter", b"alpha"));
        assert!(store.cf_exists(b"myfilter", b"alpha"));
        assert!(!store.cf_exists(b"myfilter", b"beta"));
    }

    #[test]
    fn cf_del_removes_item() {
        let store = ps();
        store.cf_reserve(b"delfilter", 1000);
        assert!(store.cf_add(b"delfilter", b"to-be-deleted"));
        assert!(store.cf_exists(b"delfilter", b"to-be-deleted"));
        assert!(store.cf_del(b"delfilter", b"to-be-deleted"));
        assert!(!store.cf_exists(b"delfilter", b"to-be-deleted"));
    }

    #[test]
    fn cf_del_nonexistent_returns_false() {
        let store = ps();
        assert!(!store.cf_del(b"ghost", b"item"));
    }

    #[test]
    fn cf_count_tracks_insertions() {
        let store = ps();
        store.cf_reserve(b"cnt", 100);
        assert_eq!(store.cf_count(b"cnt"), 0);
        store.cf_add(b"cnt", b"a");
        store.cf_add(b"cnt", b"b");
        store.cf_add(b"cnt", b"c");
        assert_eq!(store.cf_count(b"cnt"), 3);
    }

    #[test]
    fn cf_mexists_batch() {
        let store = ps();
        store.cf_add(b"batch", b"x");
        store.cf_add(b"batch", b"y");
        let results = store.cf_mexists(b"batch", &[b"x" as &[u8], b"y", b"z"]);
        assert_eq!(results, vec![true, true, false]);
    }

    #[test]
    fn cf_addnx_skips_existing() {
        let store = ps();
        assert!(store.cf_addnx(b"nx", b"item1"));
        assert!(!store.cf_addnx(b"nx", b"item1"));
        assert!(store.cf_addnx(b"nx", b"item2"));
    }

    // -- HyperLogLog tests --------------------------------------------------

    #[test]
    fn pf_add_and_count_approx() {
        let store = ps();
        for i in 0..100u32 {
            store.pf_add(b"hll", &[i.to_le_bytes().as_ref()]);
        }
        let count = store.pf_count(&[b"hll"]);
        let err = (count as f64 - 100.0).abs() / 100.0;
        assert!(err < 0.05, "HLL error {err:.3} > 5%, count={count}");
    }

    #[test]
    fn pf_count_three_items() {
        let store = ps();
        store.pf_add(b"k", &[b"a" as &[u8], b"b", b"c"]);
        let c = store.pf_count(&[b"k"]);
        assert!((c as i64 - 3).abs() <= 1, "count={c}");
    }

    #[test]
    fn pf_merge_union() {
        let store = ps();
        store.pf_add(b"s1", &[b"a" as &[u8], b"b"]);
        store.pf_add(b"s2", &[b"c" as &[u8], b"d"]);
        store.pf_merge(b"dest", &[b"s1", b"s2"]);
        let c = store.pf_count(&[b"dest"]);
        // Union of {a,b} and {c,d} = 4 distinct items.
        assert!((c as i64 - 4).abs() <= 1, "merged count={c}");
    }

    #[test]
    fn pf_add_returns_true_on_first_add() {
        let store = ps();
        assert!(store.pf_add(b"flag", &[b"new"]));
    }

    // -- Count-Min Sketch tests ---------------------------------------------

    #[test]
    fn cms_incrby_then_query() {
        let store = ps();
        store.cms_initbydim(b"freq", 2048, 5).unwrap();
        store.cms_incrby(b"freq", &[(b"popular" as &[u8], 42)]);
        let counts = store.cms_query(b"freq", &[b"popular", b"absent"]);
        assert!(counts[0] >= 42, "expected >= 42, got {}", counts[0]);
        assert_eq!(counts[1], 0);
    }

    #[test]
    fn cms_merge_accumulates() {
        let store = ps();
        store.cms_initbydim(b"a", 1024, 4).unwrap();
        store.cms_initbydim(b"b", 1024, 4).unwrap();
        store.cms_incrby(b"a", &[(b"x" as &[u8], 10)]);
        store.cms_incrby(b"b", &[(b"x" as &[u8], 20)]);
        store.cms_merge(b"a", &[b"b"], &[1.0]).unwrap();
        let counts = store.cms_query(b"a", &[b"x"]);
        assert!(counts[0] >= 30, "expected >= 30 after merge, got {}", counts[0]);
    }

    #[test]
    fn cms_initbyprob_auto_size() {
        let store = ps();
        store.cms_initbyprob(b"prob_sketch", 0.001, 0.01);
        store.cms_incrby(b"prob_sketch", &[(b"item" as &[u8], 5)]);
        let counts = store.cms_query(b"prob_sketch", &[b"item"]);
        assert!(counts[0] >= 5);
    }

    // -- TopK tests ---------------------------------------------------------

    #[test]
    fn topk_add_and_list() {
        let store = ps();
        store.topk_reserve(b"tk", 3, 256, 4, 0.9);
        for _ in 0..100 {
            store.topk_add(b"tk", &[b"alpha"]);
        }
        for _ in 0..50 {
            store.topk_add(b"tk", &[b"beta"]);
        }
        for _ in 0..10 {
            store.topk_add(b"tk", &[b"gamma"]);
        }
        let list = store.topk_list(b"tk");
        assert!(!list.is_empty());
        assert_eq!(list[0].0, "alpha");
    }

    #[test]
    fn topk_query_boolean() {
        let store = ps();
        store.topk_reserve(b"q", 5, 128, 4, 0.9);
        for _ in 0..200 {
            store.topk_add(b"q", &[b"frequent"]);
        }
        let res = store.topk_query(b"q", &[b"frequent", b"absent"]);
        assert!(res[0], "frequent item should be in top-k");
        assert!(!res[1], "absent item should not be in top-k");
    }

    #[test]
    fn topk_incrby_count() {
        let store = ps();
        store.topk_reserve(b"incr", 5, 256, 4, 0.9);
        store.topk_incrby(b"incr", &[(b"heavy" as &[u8], 1000)]);
        let counts = store.topk_count(b"incr", &[b"heavy"]);
        assert!(counts[0] > 0, "expected non-zero count, got {}", counts[0]);
    }

    #[test]
    fn topk_zipf_distribution_top10() {
        // Insert items with Zipf-like distribution and verify the most frequent
        // items are captured in the top-10 list.
        let store = ps();
        store.topk_reserve(b"zipf", 10, 1024, 5, 0.9);
        let items_with_counts: Vec<(Vec<u8>, u64)> = (0..20u64)
            .map(|i| {
                let key = format!("item_{i}").into_bytes();
                let count = 1000u64 / (i + 1);
                (key, count)
            })
            .collect();
        let pairs: Vec<(&[u8], u64)> = items_with_counts
            .iter()
            .map(|(k, c)| (k.as_slice(), *c))
            .collect();
        store.topk_incrby(b"zipf", &pairs);
        let list = store.topk_list(b"zipf");
        assert!(
            list.len() <= 10,
            "top-k list should have at most 10 items, got {}",
            list.len()
        );
        if !list.is_empty() {
            assert_eq!(list[0].0, "item_0", "expected item_0 at top, got {}", list[0].0);
        }
    }

    // -- HLL serialization round-trip ---------------------------------------

    #[test]
    fn hll_serialize_roundtrip() {
        let store = ps();
        for i in 0..300u32 {
            store.pf_add(b"serial", &[i.to_le_bytes().as_ref()]);
        }
        let count_before = store.pf_count(&[b"serial"]);
        let serialized = store
            .hlls
            .get(b"serial".as_ref())
            .unwrap()
            .serialize();
        let hll2 = HyperLogLog::deserialize(&serialized).unwrap();
        assert_eq!(count_before, hll2.count(), "serialization changed count");
    }
}

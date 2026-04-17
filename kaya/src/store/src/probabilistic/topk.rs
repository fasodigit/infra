// SPDX-License-Identifier: AGPL-3.0-or-later
//! TopK heavy-hitter tracker (HeavyKeeper).
//!
//! Based on Gong et al. 2018 ("HeavyKeeper: An Accurate Algorithm for Finding
//! Top-k Elephant Flows"). Uses exponential decay on collisions with
//! probability `decay^count` and maintains a separate binary min-heap for
//! sorted top-k access.
//!
//! # RESP3-compatible probabilistic commands
//!
//! - `TOPK.RESERVE <key> <k> <width> <depth> <decay>`
//! - `TOPK.ADD <key> <item> [item ...]` — returns evicted items.
//! - `TOPK.INCRBY <key> <item> <count> [item count ...]`
//! - `TOPK.QUERY <key> <item> [item ...]`
//! - `TOPK.LIST <key>` — top-k items sorted by count (descending).

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use bytes::Bytes;
use rand::Rng;
use tracing::instrument;
use xxhash_rust::xxh3::xxh3_64_with_seed;

const TOPK_SEED_BASE: u64 = 0x5151_5151_5151_5151;
const FINGERPRINT_SEED: u64 = 0xFA50_FA50_FA50_FA50;

/// A cell in the HeavyKeeper sketch.
#[derive(Debug, Clone, Copy, Default)]
pub struct Bucket {
    pub fingerprint: u64,
    pub count: u64,
}

/// A heap entry: (count, item). We use a min-heap internally by negating.
#[derive(Debug, Clone)]
struct HeapEntry {
    count: u64,
    item: Bytes,
}

impl PartialEq for HeapEntry {
    fn eq(&self, other: &Self) -> bool {
        self.count == other.count && self.item == other.item
    }
}
impl Eq for HeapEntry {}
impl PartialOrd for HeapEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for HeapEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reversed for min-heap behavior in BinaryHeap (which is max by default).
        other
            .count
            .cmp(&self.count)
            .then_with(|| other.item.cmp(&self.item))
    }
}

/// TopK heavy-hitter tracker.
pub struct TopK {
    k: usize,
    width: usize,
    depth: usize,
    decay: f64,
    buckets: Vec<Vec<Bucket>>,
    /// Min-heap of current top-k by count.
    heap: BinaryHeap<HeapEntry>,
    /// Snapshot of current counts in the heap for O(1) membership lookup.
    tracked: HashMap<Bytes, u64>,
}

impl std::fmt::Debug for TopK {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TopK")
            .field("k", &self.k)
            .field("width", &self.width)
            .field("depth", &self.depth)
            .field("decay", &self.decay)
            .field("tracked", &self.tracked.len())
            .finish()
    }
}

impl TopK {
    /// Create a TopK tracker with `k` tracked items, a `width`x`depth` sketch,
    /// and exponential `decay` in (0.0, 1.0). Typical: `decay = 0.9`.
    #[instrument(level = "debug", skip_all)]
    pub fn new(k: usize, width: usize, depth: usize, decay: f64) -> Self {
        let k = k.max(1);
        let width = width.max(1);
        let depth = depth.max(1);
        let decay = if (0.0..1.0).contains(&decay) {
            decay
        } else {
            0.9
        };
        Self {
            k,
            width,
            depth,
            decay,
            buckets: vec![vec![Bucket::default(); width]; depth],
            heap: BinaryHeap::with_capacity(k + 1),
            tracked: HashMap::with_capacity(k + 1),
        }
    }

    /// TOPK.ADD: insert `item`. Returns the item evicted from the top-k (if any).
    #[instrument(level = "trace", skip_all)]
    pub fn add(&mut self, item: &[u8]) -> Option<Bytes> {
        self.add_with_count(item, 1)
    }

    /// TOPK.INCRBY: insert `item` with weight `count`.
    #[instrument(level = "trace", skip_all)]
    pub fn add_with_count(&mut self, item: &[u8], count: u64) -> Option<Bytes> {
        if count == 0 {
            return None;
        }
        let fp = xxh3_64_with_seed(item, FINGERPRINT_SEED);
        let mut max_count = 0u64;
        let mut rng = rand::thread_rng();

        for (row_idx, row) in self.buckets.iter_mut().enumerate() {
            let seed = TOPK_SEED_BASE
                .wrapping_mul(row_idx as u64 + 1)
                .wrapping_add(row_idx as u64);
            let col = (xxh3_64_with_seed(item, seed) as usize) % self.width;
            let cell = &mut row[col];

            if cell.count == 0 {
                cell.fingerprint = fp;
                cell.count = count;
                if count > max_count {
                    max_count = count;
                }
            } else if cell.fingerprint == fp {
                cell.count = cell.count.saturating_add(count);
                if cell.count > max_count {
                    max_count = cell.count;
                }
            } else {
                // HeavyKeeper exponential decay: for each unit of count, decay
                // the cell with probability decay^cell.count. If cell hits 0,
                // evict and claim it.
                for _ in 0..count {
                    let prob = self.decay.powi(cell.count as i32);
                    let r: f64 = rng.gen();
                    if r < prob {
                        cell.count = cell.count.saturating_sub(1);
                        if cell.count == 0 {
                            cell.fingerprint = fp;
                            cell.count = 1;
                            break;
                        }
                    }
                }
                if cell.fingerprint == fp && cell.count > max_count {
                    max_count = cell.count;
                }
            }
        }

        if max_count == 0 {
            return None;
        }

        self.update_heap(item, max_count)
    }

    /// TOPK.QUERY: estimated count for `item` (min across rows for matching fingerprint).
    pub fn query(&self, item: &[u8]) -> u64 {
        let fp = xxh3_64_with_seed(item, FINGERPRINT_SEED);
        let mut best = 0u64;
        for (row_idx, row) in self.buckets.iter().enumerate() {
            let seed = TOPK_SEED_BASE
                .wrapping_mul(row_idx as u64 + 1)
                .wrapping_add(row_idx as u64);
            let col = (xxh3_64_with_seed(item, seed) as usize) % self.width;
            let cell = &row[col];
            if cell.fingerprint == fp && cell.count > best {
                best = cell.count;
            }
        }
        best
    }

    /// TOPK.LIST: current top-k items sorted by count (descending).
    pub fn list(&self) -> Vec<(Bytes, u64)> {
        let mut out: Vec<(Bytes, u64)> = self
            .tracked
            .iter()
            .map(|(k, &v)| (k.clone(), v))
            .collect();
        out.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        out
    }

    /// Number of items tracked in the heap (at most k).
    pub fn len(&self) -> usize {
        self.tracked.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tracked.is_empty()
    }

    pub fn k(&self) -> usize {
        self.k
    }

    // -- internals ---------------------------------------------------------

    /// Update the top-k heap with a (item, count) observation. Returns
    /// the item that was evicted from the top-k (if any).
    fn update_heap(&mut self, item: &[u8], count: u64) -> Option<Bytes> {
        // Fast path: already tracked, just update.
        if let Some(prev) = self.tracked.get(item).copied() {
            if count > prev {
                // Rewrite by rebuilding the heap entry (cheaper than heap API).
                self.tracked.insert(Bytes::copy_from_slice(item), count);
                self.rebuild_heap();
            }
            return None;
        }

        // Not yet tracked.
        if self.tracked.len() < self.k {
            let bytes = Bytes::copy_from_slice(item);
            self.tracked.insert(bytes.clone(), count);
            self.heap.push(HeapEntry {
                count,
                item: bytes,
            });
            return None;
        }

        // Full: compare against min entry.
        let Some(min) = self.heap.peek() else {
            return None;
        };
        if count <= min.count {
            return None;
        }

        // Evict the min.
        let evicted = self.heap.pop().map(|e| {
            self.tracked.remove(&e.item);
            e.item
        });
        let bytes = Bytes::copy_from_slice(item);
        self.tracked.insert(bytes.clone(), count);
        self.heap.push(HeapEntry {
            count,
            item: bytes,
        });
        evicted
    }

    fn rebuild_heap(&mut self) {
        self.heap.clear();
        for (item, &count) in &self.tracked {
            self.heap.push(HeapEntry {
                count,
                item: item.clone(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_tracking() {
        let mut t = TopK::new(3, 256, 4, 0.9);
        for _ in 0..100 {
            t.add(b"a");
        }
        for _ in 0..50 {
            t.add(b"b");
        }
        for _ in 0..10 {
            t.add(b"c");
        }
        let list = t.list();
        assert!(!list.is_empty());
        assert_eq!(list[0].0, Bytes::from_static(b"a"));
    }
}

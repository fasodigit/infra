//! Bloom filter implementation for BF.ADD / BF.EXISTS commands.
//!
//! Used heavily by the event-bus-lib deduplication layer.

use std::collections::HashMap;

use parking_lot::RwLock;

/// A probabilistic Bloom filter.
#[derive(Debug, Clone)]
pub struct BloomFilter {
    /// Bit array stored as bytes.
    bits: Vec<u8>,
    /// Number of bits in the filter.
    num_bits: usize,
    /// Number of hash functions to use.
    num_hashes: u32,
    /// Number of items inserted.
    count: usize,
}

impl BloomFilter {
    /// Create a new Bloom filter with the given expected capacity and false positive rate.
    pub fn new(expected_items: usize, fp_rate: f64) -> Self {
        let num_bits = Self::optimal_num_bits(expected_items, fp_rate);
        let num_hashes = Self::optimal_num_hashes(num_bits, expected_items);
        let num_bytes = (num_bits + 7) / 8;

        Self {
            bits: vec![0u8; num_bytes],
            num_bits,
            num_hashes,
            count: 0,
        }
    }

    /// BF.ADD: add an item to the filter. Returns true if the item was
    /// definitely NOT present before (i.e., this is a new insertion).
    pub fn add(&mut self, item: &[u8]) -> bool {
        let mut was_new = false;
        for i in 0..self.num_hashes {
            let idx = self.hash_index(item, i);
            let byte_idx = idx / 8;
            let bit_idx = idx % 8;
            if self.bits[byte_idx] & (1 << bit_idx) == 0 {
                was_new = true;
            }
            self.bits[byte_idx] |= 1 << bit_idx;
        }
        if was_new {
            self.count += 1;
        }
        was_new
    }

    /// BF.EXISTS: check if an item might be in the filter.
    /// Returns `false` if definitely not present, `true` if possibly present.
    pub fn exists(&self, item: &[u8]) -> bool {
        for i in 0..self.num_hashes {
            let idx = self.hash_index(item, i);
            let byte_idx = idx / 8;
            let bit_idx = idx % 8;
            if self.bits[byte_idx] & (1 << bit_idx) == 0 {
                return false;
            }
        }
        true
    }

    /// Number of items inserted.
    pub fn count(&self) -> usize {
        self.count
    }

    /// Reset the filter.
    pub fn clear(&mut self) {
        self.bits.fill(0);
        self.count = 0;
    }

    // -- internal -----------------------------------------------------------

    fn hash_index(&self, item: &[u8], seed: u32) -> usize {
        use std::hash::{Hash, Hasher};
        let mut hasher = ahash::AHasher::default();
        seed.hash(&mut hasher);
        item.hash(&mut hasher);
        (hasher.finish() as usize) % self.num_bits
    }

    fn optimal_num_bits(n: usize, p: f64) -> usize {
        let m = -(n as f64 * p.ln()) / (2.0_f64.ln().powi(2));
        m.ceil() as usize
    }

    fn optimal_num_hashes(m: usize, n: usize) -> u32 {
        let k = (m as f64 / n as f64) * 2.0_f64.ln();
        k.ceil().max(1.0) as u32
    }
}

// ---------------------------------------------------------------------------
// Bloom filter manager (for multiple named filters)
// ---------------------------------------------------------------------------

/// Manages multiple named Bloom filters (BF.RESERVE / BF.ADD / BF.EXISTS).
pub struct BloomManager {
    filters: RwLock<HashMap<String, BloomFilter>>,
}

impl BloomManager {
    pub fn new() -> Self {
        Self {
            filters: RwLock::new(HashMap::new()),
        }
    }

    /// BF.RESERVE: create a new filter with expected capacity and FP rate.
    pub fn reserve(&self, name: &str, capacity: usize, fp_rate: f64) {
        let mut filters = self.filters.write();
        filters.insert(name.to_string(), BloomFilter::new(capacity, fp_rate));
    }

    /// BF.ADD: add to a named filter (auto-creates with defaults if needed).
    pub fn add(&self, name: &str, item: &[u8]) -> bool {
        let mut filters = self.filters.write();
        let filter = filters
            .entry(name.to_string())
            .or_insert_with(|| BloomFilter::new(10_000, 0.01));
        filter.add(item)
    }

    /// BF.EXISTS: check existence in a named filter.
    pub fn exists(&self, name: &str, item: &[u8]) -> bool {
        let filters = self.filters.read();
        filters.get(name).map(|f| f.exists(item)).unwrap_or(false)
    }

    /// BF.MADD: add multiple items.
    pub fn madd(&self, name: &str, items: &[&[u8]]) -> Vec<bool> {
        let mut filters = self.filters.write();
        let filter = filters
            .entry(name.to_string())
            .or_insert_with(|| BloomFilter::new(10_000, 0.01));
        items.iter().map(|item| filter.add(item)).collect()
    }

    /// BF.MEXISTS: check multiple items.
    pub fn mexists(&self, name: &str, items: &[&[u8]]) -> Vec<bool> {
        let filters = self.filters.read();
        match filters.get(name) {
            Some(f) => items.iter().map(|item| f.exists(item)).collect(),
            None => vec![false; items.len()],
        }
    }
}

impl Default for BloomManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bloom_add_exists() {
        let mut bf = BloomFilter::new(1000, 0.01);
        assert!(bf.add(b"hello"));
        assert!(bf.exists(b"hello"));
        assert!(!bf.exists(b"world"));
    }

    #[test]
    fn bloom_manager_basic() {
        let mgr = BloomManager::new();
        mgr.reserve("dedup", 10_000, 0.01);
        assert!(mgr.add("dedup", b"msg-001"));
        assert!(mgr.exists("dedup", b"msg-001"));
        assert!(!mgr.exists("dedup", b"msg-002"));
    }
}

// SPDX-License-Identifier: AGPL-3.0-or-later
//! Count-Min Sketch: approximate frequency counting.
//!
//! Based on Cormode & Muthukrishnan 2005. Stores a 2D array of counters,
//! hashes each key into `depth` cells (one per row), and estimates frequency
//! as the minimum cell value.
//!
//! # RESP3-compatible probabilistic commands
//!
//! - `CMS.INITBYDIM <key> <width> <depth>`
//! - `CMS.INITBYPROB <key> <epsilon> <delta>`
//! - `CMS.INCRBY <key> <item> <count> [item count ...]`
//! - `CMS.QUERY <key> <item> [item ...]`
//! - `CMS.MERGE <dest> <nsrc> <src1> [src2 ...] [WEIGHTS w1 ...]`

use once_cell::sync::Lazy;
use prometheus::{register_int_counter, IntCounter};
use tracing::instrument;
use xxhash_rust::xxh3::xxh3_64_with_seed;

use super::error::ProbabilisticError;

const CMS_SEED_BASE: u64 = 0xDEAD_BEEF_CAFE_BABE;

static KAYA_CMS_UPDATES_TOTAL: Lazy<IntCounter> = Lazy::new(|| {
    register_int_counter!(
        "kaya_cms_updates_total",
        "Total number of CMS counter updates"
    )
    .unwrap_or_else(|_| IntCounter::new("dup_kaya_cms_updates_total", "fallback").unwrap())
});

static KAYA_CMS_QUERIES_TOTAL: Lazy<IntCounter> = Lazy::new(|| {
    register_int_counter!(
        "kaya_cms_queries_total",
        "Total number of CMS queries"
    )
    .unwrap_or_else(|_| IntCounter::new("dup_kaya_cms_queries_total", "fallback").unwrap())
});

/// A Count-Min Sketch estimator.
#[derive(Debug, Clone)]
pub struct CountMinSketch {
    width: usize,
    depth: usize,
    matrix: Vec<Vec<u32>>,
    seeds: Vec<u64>,
}

impl CountMinSketch {
    /// Create a CMS sized from error bound `epsilon` and failure probability `delta`.
    ///
    /// width = ceil(e / epsilon), depth = ceil(ln(1/delta)).
    #[instrument(level = "debug", skip_all)]
    pub fn new(epsilon: f64, delta: f64) -> Self {
        let width = (std::f64::consts::E / epsilon.max(1e-9)).ceil() as usize;
        let depth = (1.0 / delta.max(1e-9)).ln().ceil() as usize;
        let width = width.max(4);
        let depth = depth.max(1);
        Self::new_with_dimensions(width, depth)
    }

    /// Create a CMS with explicit `width` and `depth`.
    #[instrument(level = "debug", skip_all)]
    pub fn new_with_dimensions(width: usize, depth: usize) -> Self {
        let width = width.max(1);
        let depth = depth.max(1);
        let matrix = vec![vec![0u32; width]; depth];
        let seeds = (0..depth)
            .map(|i| CMS_SEED_BASE.wrapping_mul(i as u64 + 1).wrapping_add(i as u64))
            .collect();
        Self {
            width,
            depth,
            matrix,
            seeds,
        }
    }

    /// Validated constructor for explicit dimensions.
    pub fn try_new_with_dimensions(
        width: usize,
        depth: usize,
    ) -> Result<Self, ProbabilisticError> {
        if width == 0 || depth == 0 {
            return Err(ProbabilisticError::InvalidDimensions(
                "width and depth must be > 0".into(),
            ));
        }
        Ok(Self::new_with_dimensions(width, depth))
    }

    /// CMS.INCRBY: add `count` to the frequency of `item`.
    #[instrument(level = "trace", skip_all)]
    pub fn increment(&mut self, item: &[u8], count: u64) {
        KAYA_CMS_UPDATES_TOTAL.inc();
        let delta = count.min(u32::MAX as u64) as u32;
        for (row, &seed) in self.matrix.iter_mut().zip(self.seeds.iter()) {
            let col = (xxh3_64_with_seed(item, seed) as usize) % self.width;
            row[col] = row[col].saturating_add(delta);
        }
    }

    /// CMS.QUERY: estimate the frequency of `item` (min across rows).
    #[instrument(level = "trace", skip_all)]
    pub fn estimate(&self, item: &[u8]) -> u64 {
        KAYA_CMS_QUERIES_TOTAL.inc();
        let mut min = u32::MAX;
        for (row, &seed) in self.matrix.iter().zip(self.seeds.iter()) {
            let col = (xxh3_64_with_seed(item, seed) as usize) % self.width;
            let v = row[col];
            if v < min {
                min = v;
            }
        }
        min as u64
    }

    /// CMS.MERGE: add cell-wise counts from another sketch. Dimensions must match.
    #[instrument(level = "debug", skip_all)]
    pub fn merge(&mut self, other: &Self) {
        if self.width != other.width || self.depth != other.depth {
            return;
        }
        for (dst_row, src_row) in self.matrix.iter_mut().zip(other.matrix.iter()) {
            for (d, s) in dst_row.iter_mut().zip(src_row.iter()) {
                *d = d.saturating_add(*s);
            }
        }
    }

    /// Number of columns.
    pub fn width(&self) -> usize {
        self.width
    }

    /// Number of rows (hash functions).
    pub fn depth(&self) -> usize {
        self.depth
    }

    /// Scale all counters by `factor` (rounds each cell to nearest integer).
    /// Used by weighted CMS.MERGE.
    pub fn scale_by(&mut self, factor: f64) {
        for row in &mut self.matrix {
            for cell in row.iter_mut() {
                *cell = ((*cell as f64) * factor).round() as u32;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn increment_and_query() {
        let mut cms = CountMinSketch::new_with_dimensions(2048, 5);
        for _ in 0..100 {
            cms.increment(b"popular", 1);
        }
        for _ in 0..5 {
            cms.increment(b"rare", 1);
        }
        let p = cms.estimate(b"popular");
        let r = cms.estimate(b"rare");
        assert!(p >= 100, "popular estimate {p} < 100");
        assert!(r >= 5, "rare estimate {r} < 5");
    }

    #[test]
    fn dimensions_from_epsilon_delta() {
        let cms = CountMinSketch::new(0.001, 0.01);
        assert!(cms.width() >= 2000);
        assert!(cms.depth() >= 4);
    }
}

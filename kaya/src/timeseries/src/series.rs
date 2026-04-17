//! [`TimeSeries`]: the logical time-series object holding a list of compressed
//! [`Chunk`]s plus metadata, labels, retention policy, and compaction rules.

use std::collections::HashMap;
use tracing::debug;

use crate::aggregation::Aggregator;
use crate::chunk::{Chunk, CHUNK_CAPACITY};
use crate::error::TsError;

// ---------------------------------------------------------------------------
// DuplicatePolicy
// ---------------------------------------------------------------------------

/// How to handle a new data point whose timestamp already exists in the series.
#[derive(Debug, Clone, PartialEq)]
pub enum DuplicatePolicy {
    /// Reject the duplicate with an error.
    Block,
    /// Replace the old value with the new one.
    Last,
    /// Keep the old value, discard the new one.
    First,
    /// Keep whichever is smaller.
    Min,
    /// Keep whichever is larger.
    Max,
    /// Add the new value to the existing one.
    Sum,
}

impl DuplicatePolicy {
    /// Parse from a string (case-insensitive).
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_uppercase().as_str() {
            "BLOCK" => Some(DuplicatePolicy::Block),
            "LAST" => Some(DuplicatePolicy::Last),
            "FIRST" => Some(DuplicatePolicy::First),
            "MIN" => Some(DuplicatePolicy::Min),
            "MAX" => Some(DuplicatePolicy::Max),
            "SUM" => Some(DuplicatePolicy::Sum),
            _ => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            DuplicatePolicy::Block => "block",
            DuplicatePolicy::Last => "last",
            DuplicatePolicy::First => "first",
            DuplicatePolicy::Min => "min",
            DuplicatePolicy::Max => "max",
            DuplicatePolicy::Sum => "sum",
        }
    }
}

// ---------------------------------------------------------------------------
// CompactionRule
// ---------------------------------------------------------------------------

/// A downsampling rule that writes aggregated points to a destination series.
#[derive(Debug, Clone)]
pub struct CompactionRule {
    /// The destination series key (as raw bytes).
    pub dest_key: Vec<u8>,
    /// Bucket size in milliseconds.
    pub bucket_ms: i64,
    /// Aggregation function applied to each bucket.
    pub aggregator: Aggregator,
    /// Timestamp of the last bucket that was emitted.
    pub last_bucket_ts: i64,
}

impl CompactionRule {
    pub fn new(dest_key: Vec<u8>, bucket_ms: i64, aggregator: Aggregator) -> Self {
        Self {
            dest_key,
            bucket_ms,
            aggregator,
            last_bucket_ts: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// TimeSeries
// ---------------------------------------------------------------------------

/// A single named time series stored in KAYA.
///
/// Internally, data is kept in a sequence of Gorilla-compressed [`Chunk`]s.
/// A new chunk is opened whenever the current one is full. Chunks older than
/// the configured `retention_ms` are removed during [`compact`].
#[derive(Debug)]
pub struct TimeSeries {
    /// User-defined metadata labels, e.g. `{"sensor": "temp", "room": "hall"}`.
    pub labels: HashMap<String, String>,
    /// Retention window in milliseconds (`0` = unlimited).
    pub retention_ms: i64,
    /// Duplicate timestamp policy.
    pub duplicate_policy: DuplicatePolicy,
    /// Ordered list of chunks (oldest first).
    chunks: Vec<Chunk>,
    /// Downsampling rules.
    pub rules: Vec<CompactionRule>,
    /// Total data points across all chunks.
    total_points: usize,
    /// Chunk capacity target (default [`CHUNK_CAPACITY`]).
    #[allow(dead_code)]
    chunk_capacity: usize,
}

impl TimeSeries {
    /// Create a new, empty time series.
    pub fn new(
        labels: HashMap<String, String>,
        retention_ms: i64,
        duplicate_policy: DuplicatePolicy,
    ) -> Self {
        Self {
            labels,
            retention_ms,
            duplicate_policy,
            chunks: Vec::new(),
            rules: Vec::new(),
            total_points: 0,
            chunk_capacity: CHUNK_CAPACITY,
        }
    }

    // -- data ingestion --

    /// Add a data point `(ts_ms, val)` to the series.
    ///
    /// Handles duplicate policy and automatic chunk rotation.
    pub fn add(&mut self, ts: i64, val: f64) -> Result<(), TsError> {
        // Check for duplicates in the last chunk.
        if let Some(last) = self.chunks.last() {
            if last.last_ts() == ts {
                return self.handle_duplicate(ts, val);
            }
        }

        // Rotate: open a new chunk if needed.
        let need_new_chunk = self
            .chunks
            .last()
            .map(|c| c.is_full())
            .unwrap_or(true);

        if need_new_chunk {
            let c = Chunk::new(ts, val);
            self.chunks.push(c);
        } else {
            let last = self.chunks.last_mut().unwrap();
            last.append(ts, val)?;
        }
        self.total_points += 1;
        Ok(())
    }

    fn handle_duplicate(&mut self, ts: i64, new_val: f64) -> Result<(), TsError> {
        match &self.duplicate_policy {
            DuplicatePolicy::Block => Err(TsError::DuplicateBlocked {
                ts,
                policy: "block".into(),
            }),
            DuplicatePolicy::First => {
                // Keep existing — silently ignore new value.
                Ok(())
            }
            DuplicatePolicy::Last | DuplicatePolicy::Min | DuplicatePolicy::Max | DuplicatePolicy::Sum => {
                // We need to re-encode the last point with the merged value.
                // Strategy: pop the last chunk, rebuild it up to len-1, then re-add.
                // For efficiency we keep the full series range query and re-insert.
                // For simplicity: replace the last chunk's last point by rebuilding it.
                let policy = self.duplicate_policy.clone();
                if let Some(chunk) = self.chunks.last() {
                    let pts: Vec<(i64, f64)> = chunk.iter().collect();
                    let last_idx = pts.len() - 1;
                    let existing_val = pts[last_idx].1;
                    let merged = match policy {
                        DuplicatePolicy::Last => new_val,
                        DuplicatePolicy::Min => existing_val.min(new_val),
                        DuplicatePolicy::Max => existing_val.max(new_val),
                        DuplicatePolicy::Sum => existing_val + new_val,
                        _ => unreachable!(),
                    };

                    // Rebuild chunk with updated last point.
                    if pts.len() == 1 {
                        let new_chunk = Chunk::new(ts, merged);
                        *self.chunks.last_mut().unwrap() = new_chunk;
                    } else {
                        let mut new_chunk = Chunk::new(pts[0].0, pts[0].1);
                        for &(t, v) in &pts[1..last_idx] {
                            let _ = new_chunk.append(t, v);
                        }
                        let _ = new_chunk.append(ts, merged);
                        *self.chunks.last_mut().unwrap() = new_chunk;
                    }
                }
                Ok(())
            }
        }
    }

    // -- queries --

    /// Return all data points in `[from_ts, to_ts]`.
    pub fn range(&self, from_ts: i64, to_ts: i64) -> Vec<(i64, f64)> {
        let mut result = Vec::new();
        for chunk in &self.chunks {
            // Skip chunks entirely outside the range.
            if chunk.last_ts() < from_ts {
                continue;
            }
            if chunk.first_ts() > to_ts {
                break;
            }
            result.extend(chunk.range(from_ts, to_ts));
        }
        result
    }

    /// Return all data points in reverse order within `[from_ts, to_ts]`.
    pub fn revrange(&self, from_ts: i64, to_ts: i64) -> Vec<(i64, f64)> {
        let mut pts = self.range(from_ts, to_ts);
        pts.reverse();
        pts
    }

    /// Return the last data point, if any.
    pub fn last_point(&self) -> Option<(i64, f64)> {
        for chunk in self.chunks.iter().rev() {
            let pts: Vec<_> = chunk.iter().collect();
            if let Some(&last) = pts.last() {
                return Some(last);
            }
        }
        None
    }

    // -- retention & compaction --

    /// Remove chunks that are entirely older than `now - retention_ms`.
    pub fn compact(&mut self, now: i64) {
        if self.retention_ms <= 0 {
            return;
        }
        let cutoff = now - self.retention_ms;
        let before = self.chunks.len();
        let dropped_points: usize = self
            .chunks
            .iter()
            .filter(|c| c.last_ts() < cutoff)
            .map(|c| c.len())
            .sum();
        self.chunks.retain(|c| c.last_ts() >= cutoff);
        let after = self.chunks.len();
        if before != after {
            self.total_points = self.total_points.saturating_sub(dropped_points);
            debug!(
                dropped_chunks = before - after,
                dropped_points,
                "compacted series"
            );
        }
    }

    /// Delete data points in `[from_ts, to_ts]`. Returns the count deleted.
    pub fn delete_range(&mut self, from_ts: i64, to_ts: i64) -> usize {
        let mut deleted = 0usize;
        let mut new_chunks: Vec<Chunk> = Vec::new();

        for chunk in self.chunks.drain(..) {
            let pts: Vec<(i64, f64)> = chunk.iter().collect();
            let kept: Vec<(i64, f64)> = pts
                .iter()
                .filter(|(ts, _)| *ts < from_ts || *ts > to_ts)
                .cloned()
                .collect();
            deleted += pts.len() - kept.len();
            if !kept.is_empty() {
                let mut new_chunk = Chunk::new(kept[0].0, kept[0].1);
                for &(t, v) in &kept[1..] {
                    let _ = new_chunk.append(t, v);
                }
                new_chunks.push(new_chunk);
            }
        }
        self.chunks = new_chunks;
        self.total_points = self.total_points.saturating_sub(deleted);
        deleted
    }

    // -- compaction rules --

    /// Add a compaction rule.
    pub fn add_rule(&mut self, rule: CompactionRule) {
        self.rules.push(rule);
    }

    /// Remove a compaction rule targeting `dest_key`. Returns `true` if found.
    pub fn remove_rule(&mut self, dest_key: &[u8]) -> bool {
        let before = self.rules.len();
        self.rules.retain(|r| r.dest_key != dest_key);
        self.rules.len() != before
    }

    // -- diagnostics --

    /// Total number of data points across all chunks.
    pub fn total_points(&self) -> usize {
        self.total_points
    }

    /// Number of chunks.
    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// Approximate memory usage in bytes.
    pub fn memory_bytes(&self) -> usize {
        self.chunks.iter().map(|c| c.size_bytes()).sum()
    }

    /// Timestamp of the first data point, if any.
    pub fn first_ts(&self) -> Option<i64> {
        self.chunks.first().map(|c| c.first_ts())
    }

    /// Timestamp of the last data point, if any.
    pub fn last_ts(&self) -> Option<i64> {
        self.chunks.last().map(|c| c.last_ts())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_series(ret: i64, pol: DuplicatePolicy) -> TimeSeries {
        TimeSeries::new(HashMap::new(), ret, pol)
    }

    #[test]
    fn test_add_and_range() {
        let mut s = make_series(0, DuplicatePolicy::Block);
        for i in 0..10i64 {
            s.add(i * 1000, i as f64).unwrap();
        }
        let pts = s.range(2000, 6000);
        assert_eq!(pts.len(), 5);
        assert_eq!(pts[0].0, 2000);
        assert_eq!(pts[4].0, 6000);
    }

    #[test]
    fn test_retention_compact() {
        let mut s = make_series(5000, DuplicatePolicy::Last);
        for i in 0..10i64 {
            s.add(i * 1000, i as f64).unwrap();
        }
        // now=10000, retention=5000, cutoff=5000 → chunks with last_ts < 5000 removed.
        s.compact(10000);
        let pts = s.range(0, 20000);
        // Points at ts 0-4000 should be gone (last_ts of their chunk < 5000).
        // Because all points land in the same chunk (< 256), we need to check differently.
        // With a single chunk: last_ts=9000 >= cutoff=5000, so chunk is kept.
        assert!(!pts.is_empty());
    }

    #[test]
    fn test_retention_compact_multi_chunk() {
        let mut s = make_series(100, DuplicatePolicy::Last); // 100 ms retention
        // Fill more than one chunk
        for i in 0..300i64 {
            s.add(i, i as f64).unwrap();
        }
        // compact at ts=300, cutoff=200 → chunks whose last_ts < 200 should be removed.
        s.compact(300);
        let _pts = s.range(0, 300);
        // First chunk covers ts 0-255 (last_ts=255 < 200? No, 255 >= 200 → kept).
        // Actually last_ts of first chunk = 255 >= 200 so it's kept too.
        // Adjust: retention=50, compact at 300 → cutoff=250.
        // Re-test with tighter retention.
        let mut s2 = TimeSeries::new(HashMap::new(), 50, DuplicatePolicy::Last);
        for i in 0..300i64 {
            s2.add(i, i as f64).unwrap();
        }
        s2.compact(300);
        let pts2 = s2.range(0, 300);
        // First chunk: last_ts=255, cutoff=250 → 255>=250 → kept.
        // Second chunk: ts 256-299. Both chunks kept in this case.
        // With cutoff=260 (retention=40, now=300):
        let mut s3 = TimeSeries::new(HashMap::new(), 40, DuplicatePolicy::Last);
        for i in 0..300i64 {
            s3.add(i, i as f64).unwrap();
        }
        s3.compact(300);
        let pts3 = s3.range(0, 300);
        // cutoff = 300-40 = 260, first chunk last_ts=255 < 260 → removed.
        // Only second chunk (ts 256..299) should remain.
        assert!(!pts3.is_empty());
        assert!(pts3[0].0 >= 256, "first ts should be >= 256, got {}", pts3[0].0);
        let _ = pts2; // silence unused warning
    }

    #[test]
    fn test_duplicate_block() {
        let mut s = make_series(0, DuplicatePolicy::Block);
        s.add(1000, 1.0).unwrap();
        let err = s.add(1000, 2.0);
        assert!(matches!(err, Err(TsError::DuplicateBlocked { .. })));
    }

    #[test]
    fn test_duplicate_last() {
        let mut s = make_series(0, DuplicatePolicy::Last);
        s.add(1000, 1.0).unwrap();
        s.add(1000, 2.0).unwrap();
        let pts = s.range(0, 2000);
        assert_eq!(pts.len(), 1);
        assert!((pts[0].1 - 2.0).abs() < 1e-9);
    }

    #[test]
    fn test_duplicate_min() {
        let mut s = make_series(0, DuplicatePolicy::Min);
        s.add(1000, 5.0).unwrap();
        s.add(1000, 2.0).unwrap();
        let pts = s.range(0, 2000);
        assert_eq!(pts.len(), 1);
        assert!((pts[0].1 - 2.0).abs() < 1e-9);
    }

    #[test]
    fn test_delete_range() {
        let mut s = make_series(0, DuplicatePolicy::Last);
        for i in 0..10i64 {
            s.add(i * 1000, i as f64).unwrap();
        }
        let deleted = s.delete_range(2000, 5000);
        assert_eq!(deleted, 4);
        let pts = s.range(0, 10000);
        assert_eq!(pts.len(), 6);
    }

    #[test]
    fn test_revrange() {
        let mut s = make_series(0, DuplicatePolicy::Block);
        for i in 0..5i64 {
            s.add(i * 1000, i as f64).unwrap();
        }
        let pts = s.revrange(0, 4000);
        assert_eq!(pts[0].0, 4000);
        assert_eq!(pts[4].0, 0);
    }
}

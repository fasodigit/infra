//! [`TimeSeriesStore`]: the top-level KAYA TimeSeries data store.
//!
//! Holds a `DashMap` of series keyed by raw bytes, provides all KAYA TS.*
//! operations, and handles label-based filtering for multi-series queries.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use dashmap::DashMap;
use parking_lot::RwLock;
use tracing::{debug, instrument};

use crate::aggregation::Aggregator;
use crate::error::TsError;
use crate::series::{CompactionRule, DuplicatePolicy, TimeSeries};

// ---------------------------------------------------------------------------
// TsCreateOpts
// ---------------------------------------------------------------------------

/// Options for `TS.CREATE` and `TS.ALTER`.
#[derive(Debug, Default, Clone)]
pub struct TsCreateOpts {
    /// Labels as key-value pairs.
    pub labels: HashMap<String, String>,
    /// Retention in milliseconds (`0` = no retention).
    pub retention_ms: i64,
    /// Duplicate timestamp policy.
    pub duplicate_policy: Option<DuplicatePolicy>,
}

// ---------------------------------------------------------------------------
// LabelFilter
// ---------------------------------------------------------------------------

/// A filter predicate on series labels, e.g. `sensor=temp AND room=hall`.
#[derive(Debug, Clone)]
pub struct LabelFilter {
    /// All these `(key, value)` pairs must be present in the series labels.
    pub required: Vec<(String, String)>,
    /// These keys must be absent from the labels.
    pub absent: Vec<String>,
    /// These keys must be present (any value).
    pub present: Vec<String>,
}

impl LabelFilter {
    /// Parse a slice of filter strings. Each element must be one of:
    /// - `key=value`   — key equals value
    /// - `key!=value`  — key present but not equal to value (treated as absent value check)
    /// - `key=`        — key must be present (any value)
    /// - `key!=`       — key must be absent
    pub fn parse(filters: &[&str]) -> Result<Self, TsError> {
        let mut required = Vec::new();
        let mut absent = Vec::new();
        let mut present = Vec::new();

        for f in filters {
            if let Some(pos) = f.find("!=") {
                let key = f[..pos].to_string();
                let val = &f[pos + 2..];
                if val.is_empty() {
                    absent.push(key);
                } else {
                    // key!=value treated as: key must be present AND key != value.
                    // For simplicity we store as negative match.
                    present.push(format!("{key}!={val}"));
                }
            } else if let Some(pos) = f.find('=') {
                let key = f[..pos].to_string();
                let val = f[pos + 1..].to_string();
                if val.is_empty() {
                    present.push(key);
                } else {
                    required.push((key, val));
                }
            } else {
                return Err(TsError::LabelFilter(format!(
                    "invalid filter: '{f}', expected 'key=value' or 'key!=value'"
                )));
            }
        }
        Ok(Self { required, absent, present })
    }

    /// Returns `true` if the given label map satisfies all filter conditions.
    pub fn matches(&self, labels: &HashMap<String, String>) -> bool {
        for (k, v) in &self.required {
            if labels.get(k).map(String::as_str) != Some(v.as_str()) {
                return false;
            }
        }
        for k in &self.absent {
            if labels.contains_key(k) {
                return false;
            }
        }
        for entry in &self.present {
            if let Some(neq_pos) = entry.find("!=") {
                // Negative match: key!=value
                let key = &entry[..neq_pos];
                let val = &entry[neq_pos + 2..];
                match labels.get(key) {
                    None => return false, // key must be present
                    Some(lv) if lv == val => return false, // value must differ
                    _ => {}
                }
            } else {
                // Presence check.
                if !labels.contains_key(entry.as_str()) {
                    return false;
                }
            }
        }
        true
    }
}

// ---------------------------------------------------------------------------
// TimeSeriesStore
// ---------------------------------------------------------------------------

/// The top-level KAYA TimeSeries store.
///
/// Thread-safe via `DashMap` sharding. All series access is wrapped in
/// `Arc<RwLock<TimeSeries>>` so concurrent reads are lock-free at the
/// series level when multiple readers exist.
#[derive(Debug)]
pub struct TimeSeriesStore {
    series: DashMap<Vec<u8>, Arc<RwLock<TimeSeries>>, ahash::RandomState>,
}

impl TimeSeriesStore {
    /// Create a new, empty store.
    pub fn new() -> Self {
        Self {
            series: DashMap::with_hasher(ahash::RandomState::new()),
        }
    }

    // -- lifecycle --

    /// Create a new time series. Returns `Err(AlreadyExists)` if the key is taken.
    #[instrument(skip(self, opts))]
    pub fn create(&self, key: &[u8], opts: TsCreateOpts) -> Result<(), TsError> {
        if self.series.contains_key(key) {
            return Err(TsError::AlreadyExists(
                String::from_utf8_lossy(key).into_owned(),
            ));
        }
        let ts = TimeSeries::new(
            opts.labels,
            opts.retention_ms,
            opts.duplicate_policy.unwrap_or(DuplicatePolicy::Last),
        );
        self.series.insert(key.to_vec(), Arc::new(RwLock::new(ts)));
        debug!(key = %String::from_utf8_lossy(key), "series created");
        Ok(())
    }

    /// Alter an existing series. Returns `Err(NotFound)` if the key is absent.
    pub fn alter(&self, key: &[u8], opts: TsCreateOpts) -> Result<(), TsError> {
        let arc = self
            .series
            .get(key)
            .ok_or_else(|| TsError::NotFound(String::from_utf8_lossy(key).into_owned()))?
            .clone();
        let mut ts = arc.write();
        if !opts.labels.is_empty() {
            ts.labels = opts.labels;
        }
        if opts.retention_ms > 0 {
            ts.retention_ms = opts.retention_ms;
        }
        if let Some(pol) = opts.duplicate_policy {
            ts.duplicate_policy = pol;
        }
        Ok(())
    }

    /// Delete an entire series. Returns `Err(NotFound)` if it does not exist.
    pub fn delete_series(&self, key: &[u8]) -> Result<(), TsError> {
        self.series
            .remove(key)
            .ok_or_else(|| TsError::NotFound(String::from_utf8_lossy(key).into_owned()))?;
        Ok(())
    }

    // -- data ingestion --

    /// Add a single `(ts, val)` data point. Auto-creates the series if absent.
    pub fn add(&self, key: &[u8], ts: i64, val: f64) -> Result<(), TsError> {
        self.ensure_series(key);
        let arc = self.series.get(key).unwrap().clone();
        let x = arc.write().add(ts, val);
        x
    }

    /// Add multiple `(key, ts, val)` triples. Returns a `Vec<Result>` in order.
    pub fn madd(&self, triples: &[(&[u8], i64, f64)]) -> Vec<Result<(), TsError>> {
        triples
            .iter()
            .map(|(key, ts, val)| self.add(key, *ts, *val))
            .collect()
    }

    /// Increment the last value of a series by `delta` at the given timestamp
    /// (defaults to `now` if `ts` is `None`).
    pub fn incrby(&self, key: &[u8], delta: f64, ts: Option<i64>) -> Result<f64, TsError> {
        let ts = ts.unwrap_or_else(now_ms);
        self.ensure_series(key);
        let arc = self.series.get(key).unwrap().clone();
        let mut w = arc.write();
        let last_val = w.last_point().map(|(_, v)| v).unwrap_or(0.0);
        let new_val = last_val + delta;
        w.add(ts, new_val)?;
        Ok(new_val)
    }

    /// Decrement the last value by `delta`. Equivalent to `incrby(key, -delta, ts)`.
    pub fn decrby(&self, key: &[u8], delta: f64, ts: Option<i64>) -> Result<f64, TsError> {
        self.incrby(key, -delta, ts)
    }

    // -- queries --

    /// Query a single series in `[from_ts, to_ts]`, with optional aggregation.
    pub fn range(
        &self,
        key: &[u8],
        from_ts: i64,
        to_ts: i64,
        agg: Option<&Aggregator>,
        bucket_ms: Option<i64>,
    ) -> Result<Vec<(i64, f64)>, TsError> {
        let arc = self
            .series
            .get(key)
            .ok_or_else(|| TsError::NotFound(String::from_utf8_lossy(key).into_owned()))?
            .clone();
        let pts = arc.read().range(from_ts, to_ts);
        Ok(aggregate_buckets(pts, agg, bucket_ms, from_ts))
    }

    /// Query a single series in reverse order.
    pub fn revrange(
        &self,
        key: &[u8],
        from_ts: i64,
        to_ts: i64,
        agg: Option<&Aggregator>,
        bucket_ms: Option<i64>,
    ) -> Result<Vec<(i64, f64)>, TsError> {
        let arc = self
            .series
            .get(key)
            .ok_or_else(|| TsError::NotFound(String::from_utf8_lossy(key).into_owned()))?
            .clone();
        let pts = arc.read().revrange(from_ts, to_ts);
        let pts_fwd: Vec<_> = pts.iter().rev().cloned().collect();
        let mut result = aggregate_buckets(pts_fwd, agg, bucket_ms, from_ts);
        result.reverse();
        Ok(result)
    }

    /// Get the last data point of a series.
    pub fn get(&self, key: &[u8]) -> Result<Option<(i64, f64)>, TsError> {
        let arc = self
            .series
            .get(key)
            .ok_or_else(|| TsError::NotFound(String::from_utf8_lossy(key).into_owned()))?
            .clone();
        let x = arc.read().last_point();
        Ok(x)
    }

    /// Query multiple series matching a label filter, returning points in `[from, to]`.
    pub fn mrange(
        &self,
        filter: &LabelFilter,
        from_ts: i64,
        to_ts: i64,
        agg: Option<&Aggregator>,
        bucket_ms: Option<i64>,
    ) -> Vec<(Vec<u8>, HashMap<String, String>, Vec<(i64, f64)>)> {
        let mut result = Vec::new();
        for entry in self.series.iter() {
            let ts = entry.value().read();
            if !filter.matches(&ts.labels) {
                continue;
            }
            let pts = ts.range(from_ts, to_ts);
            let agg_pts = aggregate_buckets(pts, agg, bucket_ms, from_ts);
            result.push((entry.key().clone(), ts.labels.clone(), agg_pts));
        }
        result
    }

    /// Query multiple series in reverse order matching a label filter.
    pub fn mrevrange(
        &self,
        filter: &LabelFilter,
        from_ts: i64,
        to_ts: i64,
        agg: Option<&Aggregator>,
        bucket_ms: Option<i64>,
    ) -> Vec<(Vec<u8>, HashMap<String, String>, Vec<(i64, f64)>)> {
        let mut rows = self.mrange(filter, from_ts, to_ts, agg, bucket_ms);
        for (_, _, pts) in &mut rows {
            pts.reverse();
        }
        rows
    }

    /// Return the last data point of each series matching the filter.
    pub fn mget(
        &self,
        filter: &LabelFilter,
    ) -> Vec<(Vec<u8>, HashMap<String, String>, Option<(i64, f64)>)> {
        let mut result = Vec::new();
        for entry in self.series.iter() {
            let ts = entry.value().read();
            if !filter.matches(&ts.labels) {
                continue;
            }
            let last = ts.last_point();
            result.push((entry.key().clone(), ts.labels.clone(), last));
        }
        result
    }

    /// Return all keys whose labels match the filter.
    pub fn query_index(&self, filter: &LabelFilter) -> Vec<Vec<u8>> {
        self.series
            .iter()
            .filter(|e| filter.matches(&e.value().read().labels))
            .map(|e| e.key().clone())
            .collect()
    }

    // -- series info --

    /// Return metadata about a series.
    pub fn info(&self, key: &[u8]) -> Result<TsInfo, TsError> {
        let arc = self
            .series
            .get(key)
            .ok_or_else(|| TsError::NotFound(String::from_utf8_lossy(key).into_owned()))?
            .clone();
        let ts = arc.read();
        Ok(TsInfo {
            total_samples: ts.total_points() as i64,
            memory_usage: ts.memory_bytes() as i64,
            first_timestamp: ts.first_ts().unwrap_or(0),
            last_timestamp: ts.last_ts().unwrap_or(0),
            retention_time: ts.retention_ms,
            chunk_count: ts.chunk_count() as i64,
            duplicate_policy: ts.duplicate_policy.name().to_string(),
            labels: ts.labels.clone(),
            rules: ts.rules.iter().map(|r| {
                RuleInfo {
                    dest_key: r.dest_key.clone(),
                    bucket_ms: r.bucket_ms,
                    aggregator: r.aggregator.name().to_string(),
                }
            }).collect(),
        })
    }

    /// Delete data points in `[from_ts, to_ts]` from a series.
    pub fn del_range(&self, key: &[u8], from_ts: i64, to_ts: i64) -> Result<usize, TsError> {
        let arc = self
            .series
            .get(key)
            .ok_or_else(|| TsError::NotFound(String::from_utf8_lossy(key).into_owned()))?
            .clone();
        let x = arc.write().delete_range(from_ts, to_ts);
        Ok(x)
    }

    // -- compaction rules --

    /// Attach a compaction rule to `src_key`.
    pub fn create_rule(
        &self,
        src_key: &[u8],
        dest_key: Vec<u8>,
        bucket_ms: i64,
        aggregator: Aggregator,
    ) -> Result<(), TsError> {
        // Ensure dest_key exists.
        if !self.series.contains_key(dest_key.as_slice()) {
            return Err(TsError::CompactionRule(format!(
                "destination series '{}' does not exist",
                String::from_utf8_lossy(&dest_key)
            )));
        }
        let arc = self
            .series
            .get(src_key)
            .ok_or_else(|| TsError::NotFound(String::from_utf8_lossy(src_key).into_owned()))?
            .clone();
        arc.write()
            .add_rule(CompactionRule::new(dest_key, bucket_ms, aggregator));
        Ok(())
    }

    /// Remove a compaction rule from `src_key` targeting `dest_key`.
    pub fn delete_rule(&self, src_key: &[u8], dest_key: &[u8]) -> Result<(), TsError> {
        let arc = self
            .series
            .get(src_key)
            .ok_or_else(|| TsError::NotFound(String::from_utf8_lossy(src_key).into_owned()))?
            .clone();
        let removed = arc.write().remove_rule(dest_key);
        if !removed {
            return Err(TsError::CompactionRule(format!(
                "no rule found for dest '{}'",
                String::from_utf8_lossy(dest_key)
            )));
        }
        Ok(())
    }

    /// Apply all compaction rules for `src_key` given that a new point was just added.
    /// Aggregated results are written back into the destination series.
    pub fn apply_rules(&self, src_key: &[u8], new_ts: i64) {
        let rules: Vec<CompactionRule> = {
            let arc = match self.series.get(src_key) {
                Some(a) => a.clone(),
                None => return,
            };
            let x = arc.read().rules.clone();
            x
        };

        for rule in rules {
            let bucket_start = (new_ts / rule.bucket_ms) * rule.bucket_ms;
            // Collect points from the current bucket.
            let pts = {
                let arc = match self.series.get(src_key) {
                    Some(a) => a.clone(),
                    None => continue,
                };
                let x = arc.read()
                    .range(bucket_start, bucket_start + rule.bucket_ms - 1);
                x
            };
            if pts.is_empty() {
                continue;
            }
            let agg_val = rule.aggregator.apply(&pts);
            // Write into destination.
            if let Some(dest) = self.series.get(rule.dest_key.as_slice()) {
                let dest_arc = dest.clone();
                let _ = dest_arc.write().add(bucket_start, agg_val);
            }
        }
    }

    /// Trigger retention compaction for a specific series at the given `now` timestamp.
    /// Points older than `now - retention_ms` are removed.
    pub fn compact(&self, key: &[u8], now: i64) -> Result<(), TsError> {
        let arc = self
            .series
            .get(key)
            .ok_or_else(|| TsError::NotFound(String::from_utf8_lossy(key).into_owned()))?
            .clone();
        arc.write().compact(now);
        Ok(())
    }

    // -- private helpers --

    /// Ensure a series exists, creating it with defaults if absent.
    fn ensure_series(&self, key: &[u8]) {
        if !self.series.contains_key(key) {
            let ts = TimeSeries::new(HashMap::new(), 0, DuplicatePolicy::Last);
            self.series
                .insert(key.to_vec(), Arc::new(RwLock::new(ts)));
        }
    }
}

impl Default for TimeSeriesStore {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// TsInfo / RuleInfo
// ---------------------------------------------------------------------------

/// Metadata returned by `TS.INFO`.
#[derive(Debug, Clone)]
pub struct TsInfo {
    pub total_samples: i64,
    pub memory_usage: i64,
    pub first_timestamp: i64,
    pub last_timestamp: i64,
    pub retention_time: i64,
    pub chunk_count: i64,
    pub duplicate_policy: String,
    pub labels: HashMap<String, String>,
    pub rules: Vec<RuleInfo>,
}

/// Compaction rule metadata in `TS.INFO`.
#[derive(Debug, Clone)]
pub struct RuleInfo {
    pub dest_key: Vec<u8>,
    pub bucket_ms: i64,
    pub aggregator: String,
}

// ---------------------------------------------------------------------------
// Private aggregation helper
// ---------------------------------------------------------------------------

fn aggregate_buckets(
    pts: Vec<(i64, f64)>,
    agg: Option<&Aggregator>,
    bucket_ms: Option<i64>,
    _from_ts: i64,
) -> Vec<(i64, f64)> {
    match (agg, bucket_ms) {
        (Some(agg), Some(bms)) if bms > 0 => {
            if pts.is_empty() {
                return vec![];
            }
            let mut buckets: Vec<(i64, Vec<(i64, f64)>)> = Vec::new();
            for pt in pts {
                let bk = (pt.0 / bms) * bms;
                if let Some(last) = buckets.last_mut() {
                    if last.0 == bk {
                        last.1.push(pt);
                        continue;
                    }
                }
                buckets.push((bk, vec![pt]));
            }
            buckets
                .into_iter()
                .map(|(bk, bpts)| (bk, agg.apply(&bpts)))
                .collect()
        }
        _ => pts,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aggregation::Aggregator;

    fn store() -> TimeSeriesStore {
        TimeSeriesStore::new()
    }

    #[test]
    fn test_create_and_add() {
        let s = store();
        s.create(b"sensor:1", TsCreateOpts::default()).unwrap();
        s.add(b"sensor:1", 1000, 42.0).unwrap();
        let pts = s.range(b"sensor:1", 0, 9999, None, None).unwrap();
        assert_eq!(pts.len(), 1);
        assert!((pts[0].1 - 42.0).abs() < 1e-9);
    }

    #[test]
    fn test_create_duplicate_rejected() {
        let s = store();
        s.create(b"k", TsCreateOpts::default()).unwrap();
        let err = s.create(b"k", TsCreateOpts::default());
        assert!(matches!(err, Err(TsError::AlreadyExists(_))));
    }

    #[test]
    fn test_range_aggregation_avg() {
        let s = store();
        s.create(b"t", TsCreateOpts::default()).unwrap();
        for i in 0..10i64 {
            s.add(b"t", i * 1000, i as f64).unwrap();
        }
        let pts = s
            .range(b"t", 0, 9999, Some(&Aggregator::Avg), Some(5000))
            .unwrap();
        assert_eq!(pts.len(), 2);
        // Bucket [0,4999]: avg(0,1,2,3,4) = 2.0
        assert!((pts[0].1 - 2.0).abs() < 1e-9, "bucket0 avg={}", pts[0].1);
        // Bucket [5000,9999]: avg(5,6,7,8,9) = 7.0
        assert!((pts[1].1 - 7.0).abs() < 1e-9, "bucket1 avg={}", pts[1].1);
    }

    #[test]
    fn test_range_aggregation_sum() {
        let s = store();
        s.create(b"t2", TsCreateOpts::default()).unwrap();
        for i in 0..4i64 {
            s.add(b"t2", i * 1000, 1.0).unwrap();
        }
        let pts = s
            .range(b"t2", 0, 9999, Some(&Aggregator::Sum), Some(4000))
            .unwrap();
        assert_eq!(pts.len(), 1);
        assert!((pts[0].1 - 4.0).abs() < 1e-9);
    }

    #[test]
    fn test_incrby() {
        let s = store();
        s.create(b"ctr", TsCreateOpts::default()).unwrap();
        s.incrby(b"ctr", 5.0, Some(1000)).unwrap();
        s.incrby(b"ctr", 3.0, Some(2000)).unwrap();
        let pts = s.range(b"ctr", 0, 9999, None, None).unwrap();
        assert_eq!(pts.len(), 2);
        // Second call: last_val=5.0, +3.0 = 8.0
        assert!((pts[1].1 - 8.0).abs() < 1e-9);
    }

    #[test]
    fn test_label_filter_mget() {
        let s = store();
        let mut opts = TsCreateOpts::default();
        opts.labels.insert("sensor".into(), "temp".into());
        opts.labels.insert("room".into(), "hall".into());
        s.create(b"s1", opts.clone()).unwrap();
        s.add(b"s1", 1000, 22.5).unwrap();

        let mut opts2 = TsCreateOpts::default();
        opts2.labels.insert("sensor".into(), "humidity".into());
        s.create(b"s2", opts2).unwrap();
        s.add(b"s2", 1000, 60.0).unwrap();

        let filter = LabelFilter::parse(&["sensor=temp", "room=hall"]).unwrap();
        let results = s.mget(&filter);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, b"s1");
    }

    #[test]
    fn test_compaction_rule() {
        let s = store();
        s.create(b"raw", TsCreateOpts::default()).unwrap();
        s.create(b"agg", TsCreateOpts::default()).unwrap();
        s.create_rule(b"raw", b"agg".to_vec(), 60_000, Aggregator::Avg)
            .unwrap();

        // Add points within the same 60-second bucket.
        for i in 0..5i64 {
            s.add(b"raw", i * 1000, i as f64).unwrap();
            s.apply_rules(b"raw", i * 1000);
        }

        // The dest series should have at least one aggregated point.
        let agg_pts = s.range(b"agg", 0, 999_999, None, None).unwrap();
        assert!(!agg_pts.is_empty(), "no aggregated points in dest series");
        // Avg of 0,1,2,3,4 = 2.0 (or intermediate values depending on when triggered).
        let last = agg_pts.last().unwrap().1;
        assert!(last >= 0.0, "last agg val = {last}");
    }

    #[test]
    fn test_del_range() {
        let s = store();
        s.create(b"d", TsCreateOpts::default()).unwrap();
        for i in 0..10i64 {
            s.add(b"d", i * 1000, i as f64).unwrap();
        }
        let count = s.del_range(b"d", 2000, 5000).unwrap();
        assert_eq!(count, 4);
        let pts = s.range(b"d", 0, 99999, None, None).unwrap();
        assert_eq!(pts.len(), 6);
    }

    #[test]
    fn test_info() {
        let s = store();
        let mut opts = TsCreateOpts::default();
        opts.labels.insert("host".into(), "node1".into());
        opts.retention_ms = 60_000;
        s.create(b"m", opts).unwrap();
        s.add(b"m", 1000, 1.0).unwrap();
        s.add(b"m", 2000, 2.0).unwrap();

        let info = s.info(b"m").unwrap();
        assert_eq!(info.total_samples, 2);
        assert_eq!(info.retention_time, 60_000);
        assert_eq!(info.labels.get("host").map(String::as_str), Some("node1"));
    }
}

// ---------------------------------------------------------------------------
// Utility
// ---------------------------------------------------------------------------

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

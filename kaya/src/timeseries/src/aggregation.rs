//! Aggregation functions for KAYA TimeSeries downsampling and query aggregation.
//!
//! Each [`Aggregator`] variant implements [`Aggregator::apply`] which consumes a
//! slice of `(timestamp_ms, value)` tuples and returns a single `f64` result.

/// Aggregation functions supported by KAYA TimeSeries.
#[derive(Debug, Clone, PartialEq)]
pub enum Aggregator {
    /// Arithmetic mean of all values.
    Avg,
    /// Sum of all values.
    Sum,
    /// Minimum value.
    Min,
    /// Maximum value.
    Max,
    /// Count of data points.
    Count,
    /// First (oldest) value in the bucket.
    First,
    /// Last (newest) value in the bucket.
    Last,
    /// Range: `max - min`.
    Range,
    /// Population standard deviation.
    Std,
    /// Population variance.
    Var,
    /// 50th percentile (median).
    P50,
    /// 90th percentile.
    P90,
    /// 95th percentile.
    P95,
    /// 99th percentile.
    P99,
    /// Time-weighted average: area under the step function divided by time span.
    Twa,
}

impl Aggregator {
    /// Compute the aggregate over a slice of `(timestamp_ms, value)` points.
    ///
    /// Returns `f64::NAN` if `points` is empty (callers should handle this).
    pub fn apply(&self, points: &[(i64, f64)]) -> f64 {
        if points.is_empty() {
            return f64::NAN;
        }
        match self {
            Aggregator::Avg => {
                let sum: f64 = points.iter().map(|(_, v)| v).sum();
                sum / points.len() as f64
            }
            Aggregator::Sum => points.iter().map(|(_, v)| v).sum(),
            Aggregator::Min => points
                .iter()
                .map(|(_, v)| *v)
                .fold(f64::INFINITY, f64::min),
            Aggregator::Max => points
                .iter()
                .map(|(_, v)| *v)
                .fold(f64::NEG_INFINITY, f64::max),
            Aggregator::Count => points.len() as f64,
            Aggregator::First => points[0].1,
            Aggregator::Last => points[points.len() - 1].1,
            Aggregator::Range => {
                let mn = points.iter().map(|(_, v)| *v).fold(f64::INFINITY, f64::min);
                let mx = points
                    .iter()
                    .map(|(_, v)| *v)
                    .fold(f64::NEG_INFINITY, f64::max);
                mx - mn
            }
            Aggregator::Var => {
                let mean = Aggregator::Avg.apply(points);
                let var: f64 = points.iter().map(|(_, v)| (v - mean).powi(2)).sum::<f64>()
                    / points.len() as f64;
                var
            }
            Aggregator::Std => Aggregator::Var.apply(points).sqrt(),
            Aggregator::P50 => percentile(points, 0.50),
            Aggregator::P90 => percentile(points, 0.90),
            Aggregator::P95 => percentile(points, 0.95),
            Aggregator::P99 => percentile(points, 0.99),
            Aggregator::Twa => time_weighted_avg(points),
        }
    }

    /// Parse from a string (case-insensitive). Returns `None` for unknown names.
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_uppercase().as_str() {
            "AVG" => Some(Aggregator::Avg),
            "SUM" => Some(Aggregator::Sum),
            "MIN" => Some(Aggregator::Min),
            "MAX" => Some(Aggregator::Max),
            "COUNT" => Some(Aggregator::Count),
            "FIRST" => Some(Aggregator::First),
            "LAST" => Some(Aggregator::Last),
            "RANGE" => Some(Aggregator::Range),
            "STD" => Some(Aggregator::Std),
            "VAR" => Some(Aggregator::Var),
            "P50" => Some(Aggregator::P50),
            "P90" => Some(Aggregator::P90),
            "P95" => Some(Aggregator::P95),
            "P99" => Some(Aggregator::P99),
            "TWA" => Some(Aggregator::Twa),
            _ => None,
        }
    }

    /// Return the canonical string name of this aggregator.
    pub fn name(&self) -> &'static str {
        match self {
            Aggregator::Avg => "avg",
            Aggregator::Sum => "sum",
            Aggregator::Min => "min",
            Aggregator::Max => "max",
            Aggregator::Count => "count",
            Aggregator::First => "first",
            Aggregator::Last => "last",
            Aggregator::Range => "range",
            Aggregator::Std => "std",
            Aggregator::Var => "var",
            Aggregator::P50 => "p50",
            Aggregator::P90 => "p90",
            Aggregator::P95 => "p95",
            Aggregator::P99 => "p99",
            Aggregator::Twa => "twa",
        }
    }
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Compute a percentile (0.0-1.0) using linear interpolation.
fn percentile(points: &[(i64, f64)], p: f64) -> f64 {
    let mut vals: Vec<f64> = points.iter().map(|(_, v)| *v).collect();
    vals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = vals.len();
    if n == 1 {
        return vals[0];
    }
    let pos = p * (n - 1) as f64;
    let lo = pos.floor() as usize;
    let hi = pos.ceil() as usize;
    if lo == hi {
        vals[lo]
    } else {
        let frac = pos - lo as f64;
        vals[lo] * (1.0 - frac) + vals[hi] * frac
    }
}

/// Time-weighted average: area under the step function (hold-last) divided by
/// total time span. For a single point returns that point's value.
fn time_weighted_avg(points: &[(i64, f64)]) -> f64 {
    if points.len() == 1 {
        return points[0].1;
    }
    let t_start = points[0].0;
    let t_end = points[points.len() - 1].0;
    let span = (t_end - t_start) as f64;
    if span <= 0.0 {
        return points[0].1;
    }
    let mut area = 0.0f64;
    for i in 0..points.len() - 1 {
        let dt = (points[i + 1].0 - points[i].0) as f64;
        area += points[i].1 * dt;
    }
    area / span
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn pts(vals: &[f64]) -> Vec<(i64, f64)> {
        vals.iter()
            .enumerate()
            .map(|(i, &v)| (i as i64 * 1000, v))
            .collect()
    }

    #[test]
    fn test_avg() {
        let p = pts(&[1.0, 2.0, 3.0, 4.0]);
        assert!((Aggregator::Avg.apply(&p) - 2.5).abs() < 1e-9);
    }

    #[test]
    fn test_sum() {
        let p = pts(&[1.0, 2.0, 3.0]);
        assert!((Aggregator::Sum.apply(&p) - 6.0).abs() < 1e-9);
    }

    #[test]
    fn test_min_max() {
        let p = pts(&[5.0, 1.0, 3.0]);
        assert!((Aggregator::Min.apply(&p) - 1.0).abs() < 1e-9);
        assert!((Aggregator::Max.apply(&p) - 5.0).abs() < 1e-9);
    }

    #[test]
    fn test_first_last() {
        let p = pts(&[10.0, 20.0, 30.0]);
        assert!((Aggregator::First.apply(&p) - 10.0).abs() < 1e-9);
        assert!((Aggregator::Last.apply(&p) - 30.0).abs() < 1e-9);
    }

    #[test]
    fn test_range() {
        let p = pts(&[2.0, 5.0, 1.0]);
        assert!((Aggregator::Range.apply(&p) - 4.0).abs() < 1e-9);
    }

    #[test]
    fn test_std_var() {
        let p = pts(&[2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0]);
        let var = Aggregator::Var.apply(&p);
        let std = Aggregator::Std.apply(&p);
        assert!((var - 4.0).abs() < 0.01, "var={var}");
        assert!((std - 2.0).abs() < 0.01, "std={std}");
    }

    #[test]
    fn test_percentiles() {
        let p = pts(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0]);
        assert!((Aggregator::P50.apply(&p) - 5.5).abs() < 0.01);
        let p99 = Aggregator::P99.apply(&p);
        assert!(p99 > 9.0, "p99={p99}");
    }

    #[test]
    fn test_twa() {
        // Constant value → TWA equals that value.
        let p = vec![(0i64, 5.0), (1000, 5.0), (2000, 5.0)];
        assert!((Aggregator::Twa.apply(&p) - 5.0).abs() < 1e-9);

        // Step function: value=0 for first half, value=10 for second.
        let p2 = vec![(0i64, 0.0), (500, 0.0), (1000, 10.0)];
        let twa = Aggregator::Twa.apply(&p2);
        // area = 0*500 + 0*500 = 0, span=1000 → 0
        assert!((twa - 0.0).abs() < 1e-9, "twa={twa}");
    }

    #[test]
    fn test_from_str() {
        assert_eq!(Aggregator::from_str("avg"), Some(Aggregator::Avg));
        assert_eq!(Aggregator::from_str("AVG"), Some(Aggregator::Avg));
        assert_eq!(Aggregator::from_str("p99"), Some(Aggregator::P99));
        assert_eq!(Aggregator::from_str("twa"), Some(Aggregator::Twa));
        assert_eq!(Aggregator::from_str("unknown"), None);
    }

    #[test]
    fn test_empty_returns_nan() {
        assert!(Aggregator::Avg.apply(&[]).is_nan());
    }
}

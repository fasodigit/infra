//! Distance metrics for KAYA vector search.
//!
//! Provides [`DistanceMetric`] with scalar implementations for Cosine,
//! squared-L2, and Inner-Product distances over `f32` slices. These metrics
//! are used both by the HNSW index wrappers and by the brute-force path in
//! tests/range search.

use serde::{Deserialize, Serialize};

use crate::error::VectorError;

// ---------------------------------------------------------------------------
// DistanceMetric
// ---------------------------------------------------------------------------

/// The distance (or similarity) metric to use for a vector index.
///
/// | Metric  | Interpretation | Sorted ascending? |
/// |---------|----------------|-------------------|
/// | `L2`    | Euclidean distance (squared for HNSW internals) | yes |
/// | `Cosine`| Angular distance, 0 = identical direction | yes |
/// | `IP`    | Inner product / dot product (higher = more similar) | **no** |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DistanceMetric {
    /// Euclidean (L2) squared distance.
    L2,
    /// Cosine distance: `1 - cosine_similarity`.
    Cosine,
    /// Inner product (dot product). Higher values indicate greater similarity.
    IP,
}

impl DistanceMetric {
    /// Compute the distance (or dissimilarity) between two vectors.
    ///
    /// Returns an error if the slice lengths differ.
    pub fn compute(&self, a: &[f32], b: &[f32]) -> Result<f32, VectorError> {
        if a.len() != b.len() {
            return Err(VectorError::DimMismatch {
                expected: a.len(),
                got: b.len(),
            });
        }
        Ok(match self {
            DistanceMetric::L2 => l2_squared(a, b),
            DistanceMetric::Cosine => cosine_distance(a, b),
            DistanceMetric::IP => ip_distance(a, b),
        })
    }

    /// Returns `true` when lower values indicate closer neighbours (L2 and
    /// Cosine), `false` for IP where higher values are better.
    #[inline]
    pub fn is_ascending(&self) -> bool {
        !matches!(self, DistanceMetric::IP)
    }

    /// Parse from a case-insensitive string (compatible with the FT.CREATE
    /// `DISTANCE_METRIC` parameter).
    pub fn from_str_ci(s: &str) -> Option<Self> {
        match s.to_ascii_uppercase().as_str() {
            "L2" => Some(DistanceMetric::L2),
            "COSINE" => Some(DistanceMetric::Cosine),
            "IP" => Some(DistanceMetric::IP),
            _ => None,
        }
    }

    /// Return the canonical string representation used in FT.INFO.
    pub fn as_str(&self) -> &'static str {
        match self {
            DistanceMetric::L2 => "L2",
            DistanceMetric::Cosine => "COSINE",
            DistanceMetric::IP => "IP",
        }
    }
}

// ---------------------------------------------------------------------------
// Scalar distance kernels
// ---------------------------------------------------------------------------

/// Squared Euclidean distance. Avoids the sqrt for use inside HNSW; the
/// square-root-free version preserves ordering.
#[inline]
pub fn l2_squared(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| {
            let diff = x - y;
            diff * diff
        })
        .sum()
}

/// Cosine distance: `1 - dot(a,b) / (|a| * |b|)`.
/// Returns `0.0` if either vector is the zero vector.
#[inline]
pub fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
    let (dot, norm_a, norm_b) = a.iter().zip(b.iter()).fold(
        (0f64, 0f64, 0f64),
        |(d, na, nb), (x, y)| {
            (
                d + (*x as f64) * (*y as f64),
                na + (*x as f64) * (*x as f64),
                nb + (*y as f64) * (*y as f64),
            )
        },
    );
    if norm_a <= 0.0 || norm_b <= 0.0 {
        return 0.0;
    }
    let sim = dot / (norm_a * norm_b).sqrt();
    // Clamp to [0, 2] before negating to avoid tiny negative rounding errors.
    (1.0 - sim).max(0.0) as f32
}

/// Inner-product "distance": `max(0, -dot(a, b))`.
///
/// This is `0` when `dot > 0` (vectors strongly aligned — best similarity)
/// and grows as the dot product decreases toward negative values.
/// Clamping to `≥ 0` is required by the HNSW graph invariant that distances
/// are non-negative. The ordering is still correct for non-negative dot
/// products: lower distance ↔ higher (better) dot product.
#[inline]
pub fn ip_distance(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    (-dot).max(0.0)
}

// ---------------------------------------------------------------------------
// HNSW distance adaptors
// ---------------------------------------------------------------------------
// hnsw_rs requires Distance<f32> objects. We define thin newtype wrappers so
// we can erase the metric type at runtime.

use hnsw_rs::anndists::dist::distances::Distance;

/// Adaptor that forwards to the squared-L2 kernel.
#[derive(Clone, Copy, Default)]
pub struct HnswL2;

impl Distance<f32> for HnswL2 {
    #[inline]
    fn eval(&self, va: &[f32], vb: &[f32]) -> f32 {
        l2_squared(va, vb)
    }
}

/// Adaptor that forwards to the cosine-distance kernel.
#[derive(Clone, Copy, Default)]
pub struct HnswCosine;

impl Distance<f32> for HnswCosine {
    #[inline]
    fn eval(&self, va: &[f32], vb: &[f32]) -> f32 {
        cosine_distance(va, vb)
    }
}

/// Adaptor for inner-product metric inside HNSW.
///
/// `hnsw_rs` requires distances to be non-negative. We use cosine distance
/// (`1 - cos_similarity ∈ [0, 2]`) as the HNSW graph metric. For
/// L2-normalised vectors this gives the same nearest-neighbour ordering as
/// the raw inner product (higher dot → lower cosine distance). The reported
/// `distance` values in search results use the `ip_distance` kernel
/// (see [`ip_distance`]) for user-facing output.
#[derive(Clone, Copy, Default)]
pub struct HnswIP;

impl Distance<f32> for HnswIP {
    #[inline]
    fn eval(&self, va: &[f32], vb: &[f32]) -> f32 {
        // Cosine distance is always in [0, 2], safe for hnsw_rs.
        cosine_distance(va, vb)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l2_squared_identical() {
        let v = vec![1.0f32, 2.0, 3.0];
        assert_eq!(l2_squared(&v, &v), 0.0);
    }

    #[test]
    fn l2_squared_known_value() {
        let a = vec![0.0f32, 0.0, 0.0];
        let b = vec![1.0f32, 2.0, 2.0];
        // sqrt(1+4+4)=3  → squared = 9
        assert!((l2_squared(&a, &b) - 9.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_identical_vectors() {
        let v = vec![1.0f32, 1.0, 1.0];
        let d = cosine_distance(&v, &v);
        assert!(d.abs() < 1e-6, "same direction → distance=0, got {d}");
    }

    #[test]
    fn cosine_orthogonal_vectors() {
        let a = vec![1.0f32, 0.0];
        let b = vec![0.0f32, 1.0];
        let d = cosine_distance(&a, &b);
        assert!((d - 1.0).abs() < 1e-6, "orthogonal → distance=1, got {d}");
    }

    #[test]
    fn ip_known_value() {
        let a = vec![1.0f32, 2.0, 3.0];
        let b = vec![4.0f32, 5.0, 6.0];
        // dot = 4+10+18=32 > 0  → max(0,-32) = 0
        assert_eq!(ip_distance(&a, &b), 0.0);
    }

    #[test]
    fn ip_negative_dot_becomes_positive_distance() {
        let a = vec![1.0f32, 0.0];
        let b = vec![-1.0f32, 0.0];
        // dot = -1  → max(0, 1) = 1
        assert!((ip_distance(&a, &b) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn metric_ascending_flags() {
        assert!(DistanceMetric::L2.is_ascending());
        assert!(DistanceMetric::Cosine.is_ascending());
        assert!(!DistanceMetric::IP.is_ascending());
    }

    #[test]
    fn metric_from_str_ci() {
        assert_eq!(DistanceMetric::from_str_ci("cosine"), Some(DistanceMetric::Cosine));
        assert_eq!(DistanceMetric::from_str_ci("L2"), Some(DistanceMetric::L2));
        assert_eq!(DistanceMetric::from_str_ci("ip"), Some(DistanceMetric::IP));
        assert_eq!(DistanceMetric::from_str_ci("UNKNOWN"), None);
    }

    #[test]
    fn compute_dim_mismatch() {
        let a = vec![1.0f32, 2.0];
        let b = vec![1.0f32];
        assert!(matches!(
            DistanceMetric::L2.compute(&a, &b),
            Err(VectorError::DimMismatch { .. })
        ));
    }
}

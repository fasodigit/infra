//! HNSW vector index wrapper for KAYA.
//!
//! [`VectorIndex`] encapsulates an `hnsw_rs` graph for a single named index.
//! Because `hnsw_rs::Hnsw` is generic over the distance type (a compile-time
//! parameter), we use an internal enum to dispatch at runtime.
//!
//! ## Tombstone-based deletion
//! `hnsw_rs` does not support point removal from the graph. When `delete` is
//! called we record the `id` in a `HashSet`; tombstoned IDs are filtered out
//! of search results. The internal count is adjusted accordingly.
//!
//! ## ID mapping
//! KAYA uses `u64` document IDs. `hnsw_rs` uses `usize` internally. We keep a
//! `BTreeMap<usize, u64>` that maps the internal HNSW slot to the external
//! `u64` ID, and a reverse map `HashMap<u64, usize>` for fast delete.

use std::collections::{BTreeMap, HashMap, HashSet};

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use hnsw_rs::hnsw::Hnsw;

use crate::distance::{DistanceMetric, HnswCosine, HnswIP, HnswL2};
use crate::error::VectorError;

// ---------------------------------------------------------------------------
// Public configuration types
// ---------------------------------------------------------------------------

/// Construction options for a new index.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub struct IndexOpts {
    /// HNSW `M` parameter: max neighbours stored per node per layer.
    /// Typical values: 8–48. Default: 16.
    pub m: usize,
    /// HNSW `ef_construction`: beam width during graph construction.
    /// Typical values: 100–400. Default: 200.
    pub ef_construction: usize,
    /// Expected maximum number of elements (used for capacity hints).
    /// Default: 100_000.
    pub max_elements: usize,
}

impl Default for IndexOpts {
    fn default() -> Self {
        Self {
            m: 16,
            ef_construction: 200,
            max_elements: 100_000,
        }
    }
}

// ---------------------------------------------------------------------------
// Internal per-doc metadata
// ---------------------------------------------------------------------------

/// Metadata stored alongside each inserted vector.
#[derive(Debug, Clone, Default)]
pub struct DocAttrs {
    pub attrs: HashMap<String, String>,
}

// ---------------------------------------------------------------------------
// Enum of typed HNSW graphs
// ---------------------------------------------------------------------------

/// Type-erased HNSW graph. One variant per supported metric so that we avoid
/// a `Box<dyn Any>` dance. All variants store `f32` vectors.
// Manual Debug impl because Hnsw<T,D> doesn't implement Debug.
enum HnswGraph {
    L2(Hnsw<'static, f32, HnswL2>),
    Cosine(Hnsw<'static, f32, HnswCosine>),
    IP(Hnsw<'static, f32, HnswIP>),
}

impl std::fmt::Debug for HnswGraph {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let variant = match self {
            HnswGraph::L2(_) => "HnswGraph::L2",
            HnswGraph::Cosine(_) => "HnswGraph::Cosine",
            HnswGraph::IP(_) => "HnswGraph::IP",
        };
        f.write_str(variant)
    }
}

impl HnswGraph {
    fn new(metric: DistanceMetric, opts: &IndexOpts) -> Self {
        // hnsw_rs Hnsw::new(max_nb_connection, max_elements, max_layer, ef_construction, dist)
        // max_layer = 16 (library cap). M maps to max_nb_connection.
        let max_layer = 16;
        match metric {
            DistanceMetric::L2 => HnswGraph::L2(Hnsw::new(
                opts.m,
                opts.max_elements,
                max_layer,
                opts.ef_construction,
                HnswL2,
            )),
            DistanceMetric::Cosine => HnswGraph::Cosine(Hnsw::new(
                opts.m,
                opts.max_elements,
                max_layer,
                opts.ef_construction,
                HnswCosine,
            )),
            DistanceMetric::IP => HnswGraph::IP(Hnsw::new(
                opts.m,
                opts.max_elements,
                max_layer,
                opts.ef_construction,
                HnswIP,
            )),
        }
    }

    /// Insert a vector with an internal `usize` slot id.
    fn insert(&mut self, slot: usize, vector: &[f32]) {
        match self {
            HnswGraph::L2(h) => h.insert((vector, slot)),
            HnswGraph::Cosine(h) => h.insert((vector, slot)),
            HnswGraph::IP(h) => h.insert((vector, slot)),
        }
    }

    /// Search for the `k` nearest neighbours of `query` with beam width `ef`.
    /// Returns a list of `(slot, distance)` pairs.
    fn search_knn(&self, query: &[f32], k: usize, ef: usize) -> Vec<(usize, f32)> {
        let nbrs = match self {
            HnswGraph::L2(h) => h.search(query, k, ef),
            HnswGraph::Cosine(h) => h.search(query, k, ef),
            HnswGraph::IP(h) => h.search(query, k, ef),
        };
        nbrs.into_iter()
            .map(|n| (n.d_id, n.distance))
            .collect()
    }

    /// Return the number of points stored internally (including tombstoned).
    fn nb_points(&self) -> usize {
        match self {
            HnswGraph::L2(h) => h.get_nb_point(),
            HnswGraph::Cosine(h) => h.get_nb_point(),
            HnswGraph::IP(h) => h.get_nb_point(),
        }
    }
}

// ---------------------------------------------------------------------------
// VectorIndex
// ---------------------------------------------------------------------------

/// A single named HNSW vector index.
///
/// Thread-safe via an internal `RwLock` on mutable state. The HNSW graph
/// itself is protected by the lock, so concurrent reads (searches) can
/// proceed in parallel while writes (insertions) are exclusive.
#[derive(Debug)]
pub struct VectorIndex {
    dim: usize,
    metric: DistanceMetric,
    opts: IndexOpts,
    inner: RwLock<IndexInner>,
}

#[derive(Debug)]
struct IndexInner {
    graph: HnswGraph,
    /// Next slot counter (monotonically increasing; never reused).
    next_slot: usize,
    /// Mapping: internal slot → external doc id.
    slot_to_id: BTreeMap<usize, u64>,
    /// Mapping: external doc id → internal slot (for delete).
    id_to_slot: HashMap<u64, usize>,
    /// Mapping: internal slot → stored vector (for range search + attrs).
    vectors: HashMap<usize, Vec<f32>>,
    /// Per-doc attribute store.
    attrs: HashMap<usize, DocAttrs>,
    /// Tombstoned external IDs (not yet removed from graph).
    tombstones: HashSet<u64>,
}

impl IndexInner {
    fn new(graph: HnswGraph) -> Self {
        Self {
            graph,
            next_slot: 0,
            slot_to_id: BTreeMap::new(),
            id_to_slot: HashMap::new(),
            vectors: HashMap::new(),
            attrs: HashMap::new(),
            tombstones: HashSet::new(),
        }
    }

    /// Number of live (non-tombstoned) documents.
    fn live_count(&self) -> usize {
        self.slot_to_id.len().saturating_sub(self.tombstones.len())
    }
}

impl VectorIndex {
    // -----------------------------------------------------------------------
    // Construction
    // -----------------------------------------------------------------------

    /// Create a new index.
    ///
    /// # Errors
    /// Returns [`VectorError::InvalidData`] if `dim` is zero or `opts.m`
    /// exceeds 255 (hnsw_rs hard limit).
    pub fn new(
        dim: usize,
        metric: DistanceMetric,
        opts: IndexOpts,
    ) -> Result<Self, VectorError> {
        if dim == 0 {
            return Err(VectorError::InvalidData("dimension must be > 0".into()));
        }
        if opts.m > 255 {
            return Err(VectorError::InvalidData(
                "M parameter must be ≤ 255 (hnsw_rs limit)".into(),
            ));
        }
        let graph = HnswGraph::new(metric, &opts);
        let inner = IndexInner::new(graph);
        Ok(Self {
            dim,
            metric,
            opts,
            inner: RwLock::new(inner),
        })
    }

    // -----------------------------------------------------------------------
    // Accessors
    // -----------------------------------------------------------------------

    /// Dimensionality of vectors stored in this index.
    #[inline]
    pub fn dim(&self) -> usize {
        self.dim
    }

    /// Number of live (non-tombstoned) documents.
    pub fn len(&self) -> usize {
        self.inner.read().live_count()
    }

    /// Returns `true` when no live documents exist.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The metric used by this index.
    #[inline]
    pub fn metric(&self) -> DistanceMetric {
        self.metric
    }

    /// Construction options (M, ef_construction, max_elements).
    pub fn opts(&self) -> &IndexOpts {
        &self.opts
    }

    // -----------------------------------------------------------------------
    // Mutations
    // -----------------------------------------------------------------------

    /// Insert or update a document.
    ///
    /// If `id` is already present and not tombstoned, the call is a no-op
    /// (HNSW graphs are append-only; to update, `delete` then `add`).
    ///
    /// # Errors
    /// Returns [`VectorError::DimMismatch`] if `vector.len() != self.dim`.
    pub fn add(
        &self,
        id: u64,
        vector: &[f32],
        attrs: HashMap<String, String>,
    ) -> Result<(), VectorError> {
        if vector.len() != self.dim {
            return Err(VectorError::DimMismatch {
                expected: self.dim,
                got: vector.len(),
            });
        }

        let mut inner = self.inner.write();

        // If already present and alive: idempotent re-add does nothing.
        if inner.id_to_slot.contains_key(&id) && !inner.tombstones.contains(&id) {
            return Ok(());
        }

        // If the id was previously tombstoned, clean it up.
        if inner.tombstones.contains(&id) {
            inner.tombstones.remove(&id);
        }

        let slot = inner.next_slot;
        inner.next_slot += 1;

        inner.graph.insert(slot, vector);
        inner.slot_to_id.insert(slot, id);
        inner.id_to_slot.insert(id, slot);
        inner.vectors.insert(slot, vector.to_vec());
        inner.attrs.insert(slot, DocAttrs { attrs });

        Ok(())
    }

    /// Mark a document as deleted (tombstone). Returns `true` if the id
    /// existed and was live, `false` otherwise.
    pub fn delete(&self, id: u64) -> bool {
        let mut inner = self.inner.write();
        if inner.id_to_slot.contains_key(&id) && !inner.tombstones.contains(&id) {
            inner.tombstones.insert(id);
            true
        } else {
            false
        }
    }

    // -----------------------------------------------------------------------
    // Queries
    // -----------------------------------------------------------------------

    /// Approximate KNN search.
    ///
    /// Returns at most `k` results as `(doc_id, distance)` pairs sorted by
    /// distance (ascending for L2/Cosine, ascending in the -IP sense for IP).
    ///
    /// # Errors
    /// Returns [`VectorError::DimMismatch`] if `query.len() != self.dim`.
    /// Returns [`VectorError::IndexEmpty`] if no live documents exist.
    pub fn search_knn(
        &self,
        query: &[f32],
        k: usize,
        ef: usize,
    ) -> Result<Vec<(u64, f32)>, VectorError> {
        if query.len() != self.dim {
            return Err(VectorError::DimMismatch {
                expected: self.dim,
                got: query.len(),
            });
        }

        let inner = self.inner.read();
        if inner.live_count() == 0 {
            return Err(VectorError::IndexEmpty);
        }

        // Ask for more results to account for tombstones.
        let k_adjusted = (k + inner.tombstones.len()).max(k);
        let ef_adjusted = ef.max(k_adjusted);

        let raw = inner.graph.search_knn(query, k_adjusted, ef_adjusted);

        let mut results: Vec<(u64, f32)> = raw
            .into_iter()
            .filter_map(|(slot, dist)| {
                let ext_id = inner.slot_to_id.get(&slot).copied()?;
                if inner.tombstones.contains(&ext_id) {
                    None
                } else {
                    Some((ext_id, dist))
                }
            })
            .take(k)
            .collect();

        // Sort: for IP the HNSW internally uses -dot so ascending is correct.
        results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        Ok(results)
    }

    /// Linear-scan range search. Returns all live documents whose distance to
    /// `query` is ≤ `radius`.
    ///
    /// This is exact (brute-force) and O(n). Use sparingly on large indexes.
    pub fn range_search(&self, query: &[f32], radius: f32) -> Vec<(u64, f32)> {
        let inner = self.inner.read();
        let mut results: Vec<(u64, f32)> = inner
            .slot_to_id
            .iter()
            .filter_map(|(slot, &ext_id)| {
                if inner.tombstones.contains(&ext_id) {
                    return None;
                }
                let vec = inner.vectors.get(slot)?;
                let dist = match self.metric {
                    DistanceMetric::L2 => crate::distance::l2_squared(query, vec),
                    DistanceMetric::Cosine => crate::distance::cosine_distance(query, vec),
                    DistanceMetric::IP => crate::distance::ip_distance(query, vec),
                };
                if dist <= radius {
                    Some((ext_id, dist))
                } else {
                    None
                }
            })
            .collect();

        results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        results
    }

    /// Retrieve per-doc attributes for a set of `(id, dist)` pairs.
    pub fn enrich(
        &self,
        hits: &[(u64, f32)],
    ) -> Vec<(u64, f32, HashMap<String, String>)> {
        let inner = self.inner.read();
        hits.iter()
            .map(|&(id, dist)| {
                let attrs = inner
                    .id_to_slot
                    .get(&id)
                    .and_then(|slot| inner.attrs.get(slot))
                    .map(|a| a.attrs.clone())
                    .unwrap_or_default();
                (id, dist, attrs)
            })
            .collect()
    }

    /// Total internal point count (including tombstoned). Useful for
    /// diagnostics / FT.INFO.
    pub fn internal_point_count(&self) -> usize {
        self.inner.read().graph.nb_points()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_index(dim: usize) -> VectorIndex {
        VectorIndex::new(dim, DistanceMetric::Cosine, IndexOpts::default()).unwrap()
    }

    #[test]
    fn create_and_add() {
        let idx = make_index(4);
        idx.add(1, &[1.0, 0.0, 0.0, 0.0], HashMap::new()).unwrap();
        assert_eq!(idx.len(), 1);
    }

    #[test]
    fn dim_mismatch_on_add() {
        let idx = make_index(4);
        let err = idx.add(1, &[1.0, 0.0], HashMap::new()).unwrap_err();
        assert!(matches!(err, VectorError::DimMismatch { .. }));
    }

    #[test]
    fn delete_tombstones() {
        let idx = make_index(4);
        idx.add(42, &[1.0, 0.0, 0.0, 0.0], HashMap::new()).unwrap();
        assert!(idx.delete(42));
        assert_eq!(idx.len(), 0);
        // second delete returns false
        assert!(!idx.delete(42));
    }

    #[test]
    fn search_returns_correct_order_cosine() {
        let idx = make_index(2);
        // Insert three vectors along different directions
        idx.add(1, &[1.0, 0.0], HashMap::new()).unwrap();
        idx.add(2, &[0.0, 1.0], HashMap::new()).unwrap();
        idx.add(3, &[1.0, 1.0], HashMap::new()).unwrap();

        let query = [1.0f32, 0.0];
        let results = idx.search_knn(&query, 3, 50).unwrap();
        // id=1 is most similar (same direction) → distance ≈ 0
        assert_eq!(results[0].0, 1);
        assert!(results[0].1 < 1e-5, "cosine(same) should be ~0");
    }

    #[test]
    fn search_empty_index() {
        let idx = make_index(4);
        let err = idx
            .search_knn(&[1.0, 0.0, 0.0, 0.0], 5, 50)
            .unwrap_err();
        assert!(matches!(err, VectorError::IndexEmpty));
    }

    #[test]
    fn range_search_basic() {
        let idx = VectorIndex::new(2, DistanceMetric::L2, IndexOpts::default()).unwrap();
        idx.add(10, &[0.0, 0.0], HashMap::new()).unwrap();
        idx.add(20, &[3.0, 4.0], HashMap::new()).unwrap(); // dist² = 25
        idx.add(30, &[1.0, 0.0], HashMap::new()).unwrap(); // dist² = 1

        let q = [0.0f32, 0.0];
        let results = idx.range_search(&q, 1.5);
        let ids: Vec<u64> = results.iter().map(|(id, _)| *id).collect();
        assert!(ids.contains(&10));
        assert!(ids.contains(&30));
        assert!(!ids.contains(&20));
    }
}

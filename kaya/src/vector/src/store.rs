//! Global vector store: registry of named [`VectorIndex`] instances.
//!
//! [`VectorStore`] uses a `DashMap` keyed by index name (as `Vec<u8>`) for
//! lock-free concurrent access. Each value is an `Arc<VectorIndex>` — the
//! index's own internal `RwLock` serialises mutations per-index.
//!
//! An alias table allows clients to address an index by an alternative name
//! (compatible with `FT.ALIASADD` / `FT.ALIASDEL` / `FT.ALIASUPDATE`).

use std::collections::HashMap;
use std::sync::Arc;

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use crate::distance::DistanceMetric;
use crate::error::VectorError;
use crate::index::{IndexOpts, VectorIndex};

// ---------------------------------------------------------------------------
// Filter (stub — V3.4 full-text will implement WHERE clauses)
// ---------------------------------------------------------------------------

/// A WHERE-style attribute filter. Currently not implemented.
///
/// Passing any `Filter` to [`VectorStore::search`] returns
/// [`VectorError::FilterNotImplemented`].
#[derive(Debug, Clone)]
pub struct Filter {
    pub field: String,
    pub value: String,
}

// ---------------------------------------------------------------------------
// IndexInfo
// ---------------------------------------------------------------------------

/// Summary information about a vector index, returned by `FT.INFO`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexInfo {
    pub name: String,
    pub dim: usize,
    pub metric: String,
    pub doc_count: usize,
    pub internal_point_count: usize,
    pub m: usize,
    pub ef_construction: usize,
    pub max_elements: usize,
}

// ---------------------------------------------------------------------------
// VectorStore
// ---------------------------------------------------------------------------

/// Thread-safe registry of named HNSW vector indexes.
#[derive(Debug, Default)]
pub struct VectorStore {
    indexes: DashMap<Vec<u8>, Arc<VectorIndex>>,
    /// Alias → canonical name.
    aliases: DashMap<Vec<u8>, Vec<u8>>,
}

impl VectorStore {
    /// Create a new, empty store.
    pub fn new() -> Self {
        Self::default()
    }

    // -----------------------------------------------------------------------
    // Index lifecycle
    // -----------------------------------------------------------------------

    /// Create a new index.
    ///
    /// # Errors
    /// Returns [`VectorError::IndexAlreadyExists`] if an index (or alias) with
    /// that name already exists.
    pub fn create_index(
        &self,
        name: &str,
        dim: usize,
        metric: DistanceMetric,
        opts: IndexOpts,
    ) -> Result<(), VectorError> {
        let key = name.as_bytes().to_vec();
        if self.indexes.contains_key(&key) || self.aliases.contains_key(&key) {
            return Err(VectorError::IndexAlreadyExists(name.to_owned()));
        }
        let idx = Arc::new(VectorIndex::new(dim, metric, opts)?);
        self.indexes.insert(key, idx);
        info!("vector index '{}' created (dim={}, metric={:?})", name, dim, metric);
        Ok(())
    }

    /// Drop an index and any aliases that point to it.
    ///
    /// Returns `true` if the index existed, `false` if not found.
    pub fn drop_index(&self, name: &str) -> bool {
        let key = name.as_bytes().to_vec();
        let removed = self.indexes.remove(&key).is_some();
        if removed {
            // Clean up aliases that pointed to this index.
            self.aliases.retain(|_, v| v != &key);
            info!("vector index '{}' dropped", name);
        }
        removed
    }

    // -----------------------------------------------------------------------
    // Alias management
    // -----------------------------------------------------------------------

    /// Add a new alias. Fails if the alias name is already used by an index or
    /// another alias, or if the target index does not exist.
    pub fn alias_add(&self, alias: &str, index: &str) -> Result<(), VectorError> {
        let alias_key = alias.as_bytes().to_vec();
        let index_key = index.as_bytes().to_vec();
        if self.indexes.contains_key(&alias_key) || self.aliases.contains_key(&alias_key) {
            return Err(VectorError::IndexAlreadyExists(alias.to_owned()));
        }
        if !self.indexes.contains_key(&index_key) {
            return Err(VectorError::IndexNotFound(index.to_owned()));
        }
        self.aliases.insert(alias_key, index_key);
        debug!("alias '{}' → '{}'", alias, index);
        Ok(())
    }

    /// Remove an alias. Returns `true` if it existed.
    pub fn alias_del(&self, alias: &str) -> bool {
        self.aliases.remove(alias.as_bytes()).is_some()
    }

    /// Update (reassign) an alias to a new target. If the alias does not exist
    /// it is created.
    pub fn alias_update(&self, alias: &str, index: &str) -> Result<(), VectorError> {
        let index_key = index.as_bytes().to_vec();
        if !self.indexes.contains_key(&index_key) {
            return Err(VectorError::IndexNotFound(index.to_owned()));
        }
        self.aliases.insert(alias.as_bytes().to_vec(), index_key);
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Document operations
    // -----------------------------------------------------------------------

    /// Add or update a document in an index.
    pub fn add_doc(
        &self,
        index_name: &str,
        id: u64,
        vector: &[f32],
        attrs: HashMap<String, String>,
    ) -> Result<(), VectorError> {
        let idx = self.resolve(index_name)?;
        idx.add(id, vector, attrs)
    }

    /// Tombstone a document. Returns `true` if it was live.
    pub fn del_doc(&self, index_name: &str, id: u64) -> Result<bool, VectorError> {
        let idx = self.resolve(index_name)?;
        Ok(idx.delete(id))
    }

    // -----------------------------------------------------------------------
    // Search
    // -----------------------------------------------------------------------

    /// KNN search with optional attribute filter.
    ///
    /// # Errors
    /// Returns [`VectorError::FilterNotImplemented`] if `filter` is not
    /// `None` (will be implemented in V3.4 full-text integration).
    pub fn search(
        &self,
        index_name: &str,
        query: &[f32],
        k: usize,
        ef: usize,
        filter: Option<&Filter>,
    ) -> Result<Vec<(u64, f32, HashMap<String, String>)>, VectorError> {
        if filter.is_some() {
            return Err(VectorError::FilterNotImplemented);
        }
        let idx = self.resolve(index_name)?;
        let hits = idx.search_knn(query, k, ef)?;
        Ok(idx.enrich(&hits))
    }

    // -----------------------------------------------------------------------
    // Introspection
    // -----------------------------------------------------------------------

    /// Return info about an index.
    pub fn info(&self, index_name: &str) -> Result<IndexInfo, VectorError> {
        let idx = self.resolve(index_name)?;
        let name = self.canonical_name(index_name).unwrap_or_else(|| index_name.to_owned());
        Ok(IndexInfo {
            name,
            dim: idx.dim(),
            metric: idx.metric().as_str().to_owned(),
            doc_count: idx.len(),
            internal_point_count: idx.internal_point_count(),
            m: idx.opts().m,
            ef_construction: idx.opts().ef_construction,
            max_elements: idx.opts().max_elements,
        })
    }

    /// Return all index names.
    pub fn list_indexes(&self) -> Vec<String> {
        self.indexes
            .iter()
            .map(|e| String::from_utf8_lossy(e.key()).into_owned())
            .collect()
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Resolve a name (possibly an alias) to the underlying `Arc<VectorIndex>`.
    fn resolve(&self, name: &str) -> Result<Arc<VectorIndex>, VectorError> {
        let key = name.as_bytes().to_vec();
        // Direct lookup first.
        if let Some(idx) = self.indexes.get(&key) {
            return Ok(Arc::clone(&idx));
        }
        // Then check aliases.
        if let Some(canonical) = self.aliases.get(&key) {
            if let Some(idx) = self.indexes.get(canonical.value()) {
                return Ok(Arc::clone(&idx));
            }
        }
        Err(VectorError::IndexNotFound(name.to_owned()))
    }

    /// Returns the canonical index name (resolving aliases).
    fn canonical_name(&self, name: &str) -> Option<String> {
        let key = name.as_bytes().to_vec();
        if self.indexes.contains_key(&key) {
            return Some(name.to_owned());
        }
        self.aliases
            .get(&key)
            .map(|c| String::from_utf8_lossy(c.value()).into_owned())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn default_store() -> VectorStore {
        VectorStore::new()
    }

    #[test]
    fn create_and_drop() {
        let s = default_store();
        s.create_index("emb", 8, DistanceMetric::L2, IndexOpts::default()).unwrap();
        assert!(s.info("emb").is_ok());
        assert!(s.drop_index("emb"));
        assert!(!s.drop_index("emb"));
    }

    #[test]
    fn duplicate_create_fails() {
        let s = default_store();
        s.create_index("idx", 4, DistanceMetric::Cosine, IndexOpts::default()).unwrap();
        let err = s.create_index("idx", 4, DistanceMetric::Cosine, IndexOpts::default()).unwrap_err();
        assert!(matches!(err, VectorError::IndexAlreadyExists(_)));
    }

    #[test]
    fn alias_lifecycle() {
        let s = default_store();
        s.create_index("main_idx", 4, DistanceMetric::Cosine, IndexOpts::default()).unwrap();
        s.alias_add("my_alias", "main_idx").unwrap();
        // Resolve via alias
        let info = s.info("my_alias").unwrap();
        assert_eq!(info.dim, 4);
        // Delete alias
        assert!(s.alias_del("my_alias"));
        assert!(s.info("my_alias").is_err());
    }

    #[test]
    fn add_and_search() {
        let s = default_store();
        s.create_index("v", 2, DistanceMetric::L2, IndexOpts::default()).unwrap();
        s.add_doc("v", 1, &[0.0, 0.0], HashMap::new()).unwrap();
        s.add_doc("v", 2, &[1.0, 0.0], HashMap::new()).unwrap();
        s.add_doc("v", 3, &[0.0, 1.0], HashMap::new()).unwrap();

        let results = s.search("v", &[0.0, 0.0], 2, 50, None).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, 1); // origin → dist = 0
    }

    #[test]
    fn filter_not_implemented() {
        let s = default_store();
        s.create_index("f", 2, DistanceMetric::L2, IndexOpts::default()).unwrap();
        s.add_doc("f", 1, &[1.0, 0.0], HashMap::new()).unwrap();
        let filter = Some(Filter { field: "x".into(), value: "y".into() });
        let err = s.search("f", &[1.0, 0.0], 1, 50, filter.as_ref()).unwrap_err();
        assert!(matches!(err, VectorError::FilterNotImplemented));
    }

    #[test]
    fn info_returns_correct_fields() {
        let s = default_store();
        s.create_index("info_test", 16, DistanceMetric::IP, IndexOpts { m: 8, ef_construction: 100, max_elements: 500 }).unwrap();
        let info = s.info("info_test").unwrap();
        assert_eq!(info.dim, 16);
        assert_eq!(info.metric, "IP");
        assert_eq!(info.m, 8);
        assert_eq!(info.ef_construction, 100);
    }
}

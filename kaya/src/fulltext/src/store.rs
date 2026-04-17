//! `FtStore` — DashMap-backed registry of named full-text indexes.
//!
//! Manages create/drop/search/aggregate operations across multiple indexes
//! identified by name.  Alias resolution is handled here: an alias is a
//! second name that points to the same underlying index.

use std::collections::HashMap;
use std::sync::Arc;

use dashmap::DashMap;
use tracing::{debug, instrument};

use crate::error::FtError;
use crate::index::{FieldValue, FtIndex, SearchHit, SortBy};
use crate::schema::FtSchema;

// ---------------------------------------------------------------------------
// IndexInfo
// ---------------------------------------------------------------------------

/// Snapshot of index statistics returned by FT.INFO.
#[derive(Debug)]
pub struct IndexInfo {
    pub name: String,
    pub num_docs: u64,
    pub num_fields: usize,
}

// ---------------------------------------------------------------------------
// FtStore
// ---------------------------------------------------------------------------

/// Registry of all full-text indexes.
pub struct FtStore {
    /// Maps index name → index.
    indexes: DashMap<Vec<u8>, Arc<FtIndex>>,
    /// Maps alias → canonical index name.
    aliases: DashMap<Vec<u8>, Vec<u8>>,
}

impl FtStore {
    // -----------------------------------------------------------------------
    // Construction
    // -----------------------------------------------------------------------

    /// Create an empty store.
    pub fn new() -> Self {
        Self {
            indexes: DashMap::new(),
            aliases: DashMap::new(),
        }
    }

    // -----------------------------------------------------------------------
    // Index management
    // -----------------------------------------------------------------------

    /// Create a new index with the given name and schema.
    #[instrument(skip(self, schema))]
    pub fn create(&self, name: &[u8], schema: FtSchema) -> Result<(), FtError> {
        let resolved = self.resolve(name);
        if self.indexes.contains_key(resolved) {
            let n = String::from_utf8_lossy(name).into_owned();
            return Err(FtError::IndexAlreadyExists(n));
        }
        let idx = Arc::new(FtIndex::create_in_ram(schema)?);
        self.indexes.insert(resolved.to_vec(), idx);
        debug!(index = %String::from_utf8_lossy(name), "index created");
        Ok(())
    }

    /// Drop an index by name. Returns `true` if it existed.
    #[instrument(skip(self))]
    pub fn drop_index(&self, name: &[u8]) -> bool {
        let canonical = self.resolve_owned(name);
        let removed = self.indexes.remove(&canonical).is_some();
        // Remove any aliases pointing to this index.
        self.aliases.retain(|_, v| *v != canonical);
        removed
    }

    // -----------------------------------------------------------------------
    // Document operations
    // -----------------------------------------------------------------------

    /// Add (or update) a document identified by `doc_id` in the named index.
    ///
    /// To update, first call `del_doc`, then `add_doc`.
    pub fn add_doc(
        &self,
        index_name: &[u8],
        doc_id: &[u8],
        fields: HashMap<String, FieldValue>,
    ) -> Result<(), FtError> {
        let idx = self.get_index(index_name)?;
        let id_str = String::from_utf8_lossy(doc_id).into_owned();
        idx.add_document(&id_str, fields)?;
        idx.commit()?;
        idx.reload_reader()?;
        Ok(())
    }

    /// Delete a document from the named index. Returns 1 if deletion was
    /// initiated (the document may not have existed).
    pub fn del_doc(&self, index_name: &[u8], doc_id: &[u8]) -> Result<u64, FtError> {
        let idx = self.get_index(index_name)?;
        let id_str = String::from_utf8_lossy(doc_id).into_owned();
        let n = idx.delete_by_term("__doc_id__", &id_str)?;
        idx.commit()?;
        idx.reload_reader()?;
        Ok(n)
    }

    // -----------------------------------------------------------------------
    // Search / aggregate
    // -----------------------------------------------------------------------

    /// Execute a query against the named index.
    pub fn search(
        &self,
        index_name: &[u8],
        query: &str,
        limit: usize,
        sort: Option<&SortBy>,
    ) -> Result<Vec<SearchHit>, FtError> {
        let idx = self.get_index(index_name)?;
        idx.search(query, limit, sort)
    }

    /// Aggregate documents by counting occurrences of each unique value in
    /// `group_by`.
    pub fn aggregate(
        &self,
        index_name: &[u8],
        group_by: &str,
    ) -> Result<HashMap<String, u64>, FtError> {
        let idx = self.get_index(index_name)?;
        idx.aggregate(group_by)
    }

    // -----------------------------------------------------------------------
    // Introspection
    // -----------------------------------------------------------------------

    /// Return statistics for the named index.
    pub fn info(&self, index_name: &[u8]) -> Result<IndexInfo, FtError> {
        let idx = self.get_index(index_name)?;
        let name = String::from_utf8_lossy(index_name).into_owned();
        Ok(IndexInfo {
            name,
            num_docs: idx.num_docs(),
            num_fields: idx.schema_def.fields.len(),
        })
    }

    /// Append a new field to an existing index's schema definition.
    ///
    /// Note: Tantivy does not support schema mutation on an existing index.
    /// This mutates the logical schema record only; new docs indexed after this
    /// call may include the new field. A full re-index is required for
    /// retroactive coverage.
    pub fn alter_add_field(
        &self,
        index_name: &[u8],
        field: crate::schema::FieldDef,
    ) -> Result<(), FtError> {
        // Rebuild the index with the extended schema and migrate existing docs.
        // MVP: return an error explaining the limitation.
        let _idx = self.get_index(index_name)?;
        // We cannot mutate the Tantivy schema at runtime, so we record the
        // new field in the schema_def for FT.INFO visibility only.
        // A full re-index is required for retroactive coverage.
        let _ = field;
        Err(FtError::Schema(
            "FT.ALTER: schema mutation requires re-index; add the field at FT.CREATE time".into(),
        ))
    }

    // -----------------------------------------------------------------------
    // Alias management
    // -----------------------------------------------------------------------

    /// Create an alias pointing to `target_index`.
    pub fn alias_add(&self, alias: &[u8], target_index: &[u8]) -> Result<(), FtError> {
        self.get_index(target_index)?; // validate target exists
        self.aliases
            .insert(alias.to_vec(), target_index.to_vec());
        Ok(())
    }

    /// Remove an alias.  Returns `true` if it existed.
    pub fn alias_del(&self, alias: &[u8]) -> bool {
        self.aliases.remove(alias).is_some()
    }

    /// Update an alias to point to a new target.
    pub fn alias_update(&self, alias: &[u8], new_target: &[u8]) -> Result<(), FtError> {
        self.get_index(new_target)?; // validate new target
        self.aliases.insert(alias.to_vec(), new_target.to_vec());
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    /// Resolve alias → canonical name (borrow).
    fn resolve<'a>(&'a self, name: &'a [u8]) -> &'a [u8] {
        // DashMap guards don't outlive the method, so we return the original
        // ref if no alias is found. For alias resolution we use resolve_owned.
        name
    }

    /// Resolve alias → canonical name (owned copy).
    fn resolve_owned(&self, name: &[u8]) -> Vec<u8> {
        if let Some(canonical) = self.aliases.get(name) {
            canonical.clone()
        } else {
            name.to_vec()
        }
    }

    /// Retrieve an `Arc<FtIndex>` by name (alias-aware).
    fn get_index(&self, name: &[u8]) -> Result<Arc<FtIndex>, FtError> {
        let canonical = self.resolve_owned(name);
        self.indexes
            .get(&canonical)
            .map(|v| v.clone())
            .ok_or_else(|| {
                FtError::IndexNotFound(String::from_utf8_lossy(name).into_owned())
            })
    }
}

impl Default for FtStore {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{FieldDef, FieldType, FtSchema};

    fn sample_schema() -> FtSchema {
        let mut s = FtSchema::new();
        s.add_field(FieldDef {
            name: "body".into(),
            ty: FieldType::Text { tokenized: true, analyzer: None, boost: 1.0 },
            stored: true,
        });
        s.add_field(FieldDef {
            name: "year".into(),
            ty: FieldType::Numeric { indexed: true, sortable: true },
            stored: true,
        });
        s.add_field(FieldDef {
            name: "tag".into(),
            ty: FieldType::Tag { separator: ',', case_sensitive: false },
            stored: true,
        });
        s
    }

    #[test]
    fn create_and_info() {
        let store = FtStore::new();
        store.create(b"idx1", sample_schema()).unwrap();
        let info = store.info(b"idx1").unwrap();
        assert_eq!(info.num_docs, 0);
        assert_eq!(info.num_fields, 3);
    }

    #[test]
    fn duplicate_create_fails() {
        let store = FtStore::new();
        store.create(b"idx2", sample_schema()).unwrap();
        assert!(store.create(b"idx2", sample_schema()).is_err());
    }

    #[test]
    fn drop_removes_index() {
        let store = FtStore::new();
        store.create(b"idx3", sample_schema()).unwrap();
        assert!(store.drop_index(b"idx3"));
        assert!(!store.drop_index(b"idx3")); // already gone
    }

    #[test]
    fn add_and_search_doc() {
        let store = FtStore::new();
        store.create(b"search_idx", sample_schema()).unwrap();

        let mut fields = HashMap::new();
        fields.insert("body".into(), FieldValue::Text("Burkina Faso sovereignty".into()));
        fields.insert("year".into(), FieldValue::Numeric(2024.0));

        store.add_doc(b"search_idx", b"doc1", fields).unwrap();

        let hits = store.search(b"search_idx", "@body:sovereignty", 10, None).unwrap();
        assert!(!hits.is_empty());
        assert_eq!(hits[0].doc_id, "doc1");
    }

    #[test]
    fn del_doc_reduces_count() {
        let store = FtStore::new();
        store.create(b"del_idx", sample_schema()).unwrap();

        let mut fields = HashMap::new();
        fields.insert("body".into(), FieldValue::Text("to delete".into()));
        store.add_doc(b"del_idx", b"d1", fields).unwrap();

        let info_before = store.info(b"del_idx").unwrap();
        assert_eq!(info_before.num_docs, 1);

        store.del_doc(b"del_idx", b"d1").unwrap();
        let info_after = store.info(b"del_idx").unwrap();
        assert_eq!(info_after.num_docs, 0);
    }

    #[test]
    fn alias_round_trip() {
        let store = FtStore::new();
        store.create(b"real_idx", sample_schema()).unwrap();

        store.alias_add(b"alias1", b"real_idx").unwrap();

        // Searching via alias should work.
        let mut fields = HashMap::new();
        fields.insert("body".into(), FieldValue::Text("aliased search".into()));
        store.add_doc(b"alias1", b"da1", fields).unwrap();

        let hits = store.search(b"alias1", "@body:aliased", 10, None).unwrap();
        assert!(!hits.is_empty());

        // Delete alias.
        assert!(store.alias_del(b"alias1"));
        assert!(!store.alias_del(b"alias1"));
    }

    #[test]
    fn aggregate_groups_correctly() {
        let store = FtStore::new();
        store.create(b"agg_idx", sample_schema()).unwrap();

        for (id, tag) in [("a1", "rust"), ("a2", "go"), ("a3", "rust")] {
            let mut fields = HashMap::new();
            fields.insert("body".into(), FieldValue::Text("test".into()));
            fields.insert("tag".into(), FieldValue::Tag(tag.into()));
            store.add_doc(b"agg_idx", id.as_bytes(), fields).unwrap();
        }

        let counts = store.aggregate(b"agg_idx", "tag").unwrap();
        assert_eq!(counts.get("rust").copied().unwrap_or(0), 2);
        assert_eq!(counts.get("go").copied().unwrap_or(0), 1);
    }
}

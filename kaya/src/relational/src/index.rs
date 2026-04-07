//! Index types for typed collections: B-Tree, Hash, Bloom.

use std::collections::{BTreeMap, HashSet};

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

/// Index kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IndexKind {
    BTree,
    Hash,
    Bloom,
}

/// A secondary index on a field.
pub struct Index {
    pub field_name: String,
    pub kind: IndexKind,
    /// Maps field value (as string) -> set of document IDs.
    entries: RwLock<BTreeMap<String, HashSet<String>>>,
}

impl Index {
    pub fn new(field_name: String, kind: IndexKind) -> Self {
        Self {
            field_name,
            kind,
            entries: RwLock::new(BTreeMap::new()),
        }
    }

    /// Add a document to the index.
    pub fn insert_document(&self, doc_id: &str, doc: &serde_json::Value) {
        if let Some(val) = doc.get(&self.field_name) {
            let key = value_to_index_key(val);
            let mut entries = self.entries.write();
            entries
                .entry(key)
                .or_insert_with(HashSet::new)
                .insert(doc_id.to_string());
        }
    }

    /// Remove a document from the index.
    pub fn remove_document(&self, doc_id: &str, doc: &serde_json::Value) {
        if let Some(val) = doc.get(&self.field_name) {
            let key = value_to_index_key(val);
            let mut entries = self.entries.write();
            if let Some(ids) = entries.get_mut(&key) {
                ids.remove(doc_id);
                if ids.is_empty() {
                    entries.remove(&key);
                }
            }
        }
    }

    /// Lookup document IDs by exact field value.
    pub fn lookup(&self, value: &serde_json::Value) -> HashSet<String> {
        let key = value_to_index_key(value);
        let entries = self.entries.read();
        entries.get(&key).cloned().unwrap_or_default()
    }

    /// Number of distinct values indexed.
    pub fn cardinality(&self) -> usize {
        self.entries.read().len()
    }
}

impl std::fmt::Debug for Index {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Index")
            .field("field_name", &self.field_name)
            .field("kind", &self.kind)
            .finish()
    }
}

/// Convert a JSON value to a string key for indexing.
fn value_to_index_key(val: &serde_json::Value) -> String {
    match val {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => "null".to_string(),
        other => other.to_string(),
    }
}

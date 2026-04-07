//! KAYA Relational: Typed Collections, B-Tree/Hash/Bloom indexes, KQL parser.
//!
//! Provides COLLECTION.CREATE / COLLECTION.INSERT / COLLECTION.FIND commands
//! for structured data stored in-memory with secondary indexes.

pub mod collection;
pub mod index;
pub mod kql;
pub mod schema;

use std::collections::HashMap;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use collection::Collection;
pub use index::{Index, IndexKind};
pub use kql::KqlQuery;
pub use schema::{FieldDef, FieldType, Schema};

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum RelationalError {
    #[error("collection not found: {0}")]
    CollectionNotFound(String),

    #[error("collection already exists: {0}")]
    CollectionExists(String),

    #[error("schema validation error: {0}")]
    SchemaValidation(String),

    #[error("index error: {0}")]
    IndexError(String),

    #[error("query parse error: {0}")]
    QueryParse(String),

    #[error("type mismatch: field '{field}' expected {expected}, got {actual}")]
    TypeMismatch {
        field: String,
        expected: String,
        actual: String,
    },
}

// ---------------------------------------------------------------------------
// Document
// ---------------------------------------------------------------------------

/// A document stored in a collection: a JSON-like map.
pub type Document = serde_json::Value;

/// A document with its auto-assigned ID.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredDocument {
    pub id: String,
    pub data: Document,
}

// ---------------------------------------------------------------------------
// Collection Manager
// ---------------------------------------------------------------------------

/// Manages all typed collections.
pub struct CollectionManager {
    collections: RwLock<HashMap<String, Collection>>,
}

impl CollectionManager {
    pub fn new() -> Self {
        Self {
            collections: RwLock::new(HashMap::new()),
        }
    }

    /// COLLECTION.CREATE: create a new typed collection with a schema.
    pub fn create(
        &self,
        name: &str,
        schema: Schema,
    ) -> Result<(), RelationalError> {
        let mut cols = self.collections.write();
        if cols.contains_key(name) {
            return Err(RelationalError::CollectionExists(name.into()));
        }
        cols.insert(name.to_string(), Collection::new(name.to_string(), schema));
        Ok(())
    }

    /// COLLECTION.INSERT: insert a document into a collection.
    pub fn insert(
        &self,
        collection_name: &str,
        doc: Document,
    ) -> Result<String, RelationalError> {
        let cols = self.collections.read();
        let col = cols
            .get(collection_name)
            .ok_or_else(|| RelationalError::CollectionNotFound(collection_name.into()))?;
        col.insert(doc)
    }

    /// COLLECTION.FIND: query documents from a collection.
    pub fn find(
        &self,
        collection_name: &str,
        query: &KqlQuery,
    ) -> Result<Vec<StoredDocument>, RelationalError> {
        let cols = self.collections.read();
        let col = cols
            .get(collection_name)
            .ok_or_else(|| RelationalError::CollectionNotFound(collection_name.into()))?;
        col.find(query)
    }

    /// COLLECTION.DROP: drop a collection.
    pub fn drop_collection(&self, name: &str) -> Result<(), RelationalError> {
        let mut cols = self.collections.write();
        cols.remove(name)
            .ok_or_else(|| RelationalError::CollectionNotFound(name.into()))?;
        Ok(())
    }

    /// List all collection names.
    pub fn list(&self) -> Vec<String> {
        self.collections.read().keys().cloned().collect()
    }
}

impl Default for CollectionManager {
    fn default() -> Self {
        Self::new()
    }
}

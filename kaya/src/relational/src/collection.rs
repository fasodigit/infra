//! Collection: a named set of typed documents with indexes.

use std::collections::BTreeMap;

use parking_lot::RwLock;

use crate::index::{Index, IndexKind};
use crate::kql::KqlQuery;
use crate::schema::Schema;
use crate::{Document, RelationalError, StoredDocument};

/// A typed collection: schema + documents + indexes.
pub struct Collection {
    pub name: String,
    schema: Schema,
    /// Documents indexed by auto-generated ID.
    documents: RwLock<BTreeMap<String, Document>>,
    /// Secondary indexes on fields.
    indexes: RwLock<Vec<Index>>,
    /// Auto-increment counter for document IDs.
    next_id: RwLock<u64>,
}

impl Collection {
    pub fn new(name: String, schema: Schema) -> Self {
        let indexes: Vec<Index> = schema
            .indexed_fields()
            .into_iter()
            .map(|f| Index::new(f.to_string(), IndexKind::BTree))
            .collect();

        Self {
            name,
            schema,
            documents: RwLock::new(BTreeMap::new()),
            indexes: RwLock::new(indexes),
            next_id: RwLock::new(1),
        }
    }

    /// Insert a document. Validates against schema, assigns ID, updates indexes.
    pub fn insert(&self, doc: Document) -> Result<String, RelationalError> {
        self.schema.validate(&doc)?;

        let id = {
            let mut counter = self.next_id.write();
            let id = format!("{}:{}", self.name, *counter);
            *counter += 1;
            id
        };

        // Update indexes.
        {
            let indexes = self.indexes.read();
            for idx in indexes.iter() {
                idx.insert_document(&id, &doc);
            }
        }

        self.documents.write().insert(id.clone(), doc);
        Ok(id)
    }

    /// Find documents matching a query.
    pub fn find(&self, query: &KqlQuery) -> Result<Vec<StoredDocument>, RelationalError> {
        let docs = self.documents.read();

        let results: Vec<StoredDocument> = docs
            .iter()
            .filter(|(_, doc)| query.matches(doc))
            .take(query.limit.unwrap_or(usize::MAX))
            .map(|(id, doc)| StoredDocument {
                id: id.clone(),
                data: doc.clone(),
            })
            .collect();

        Ok(results)
    }

    /// Number of documents.
    pub fn len(&self) -> usize {
        self.documents.read().len()
    }

    pub fn is_empty(&self) -> bool {
        self.documents.read().is_empty()
    }

    pub fn schema(&self) -> &Schema {
        &self.schema
    }
}

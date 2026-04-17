//! Tantivy-backed in-RAM full-text index.
//!
//! Each `FtIndex` wraps one `tantivy::Index` created entirely in RAM via
//! `tantivy::Index::create_in_ram`.  An `IndexWriter` is kept alive for the
//! lifetime of the index; callers must call `commit()` to make writes visible
//! and `reload_reader()` to refresh the search snapshot.

use std::collections::HashMap;
use std::sync::Mutex;

use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{
    self as tv_schema, Field, IndexRecordOption, NumericOptions, OwnedValue, Schema,
    TextFieldIndexing, TextOptions, STORED,
};
use tantivy::{Index, IndexReader, IndexWriter, TantivyDocument};
use tracing::{debug, instrument};

use crate::error::FtError;
use crate::query::translate_to_tantivy;
use crate::schema::{FieldDef, FieldType, FtSchema};

// ---------------------------------------------------------------------------
// FieldValue — the user-facing value type
// ---------------------------------------------------------------------------

/// A typed field value that can be stored in / retrieved from an `FtIndex`.
#[derive(Debug, Clone)]
pub enum FieldValue {
    Text(String),
    Numeric(f64),
    Tag(String),
    Geo { lat: f64, lon: f64 },
    Date(i64),
}

impl FieldValue {
    /// Render the value as a display string (used in RESP3 output).
    pub fn as_display(&self) -> String {
        match self {
            FieldValue::Text(s) => s.clone(),
            FieldValue::Numeric(n) => n.to_string(),
            FieldValue::Tag(t) => t.clone(),
            FieldValue::Geo { lat, lon } => format!("{lat},{lon}"),
            FieldValue::Date(ms) => ms.to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// SortBy
// ---------------------------------------------------------------------------

/// Sort direction for FT.SEARCH results.
#[derive(Debug, Clone)]
pub struct SortBy {
    pub field: String,
    pub ascending: bool,
}

// ---------------------------------------------------------------------------
// SearchHit
// ---------------------------------------------------------------------------

/// A single result returned by `FtIndex::search`.
#[derive(Debug, Clone)]
pub struct SearchHit {
    /// Unique document ID as provided at index time (primary key bytes).
    pub doc_id: String,
    /// BM25 relevance score.
    pub score: f32,
    /// Stored field values.
    pub fields: HashMap<String, FieldValue>,
}

// ---------------------------------------------------------------------------
// Internal field name constants
// ---------------------------------------------------------------------------

const PK_FIELD: &str = "__doc_id__";

// ---------------------------------------------------------------------------
// FtIndex
// ---------------------------------------------------------------------------

/// An in-RAM Tantivy index, with schema derived from an [`FtSchema`].
pub struct FtIndex {
    /// The Tantivy index.
    index: Index,
    /// Tantivy schema (built from `FtSchema`).
    _tv_schema: Schema,
    /// Mapping from user field name → Tantivy `Field` handle.
    field_map: HashMap<String, Field>,
    /// The primary-key field handle.
    pk_field: Field,
    /// Writer — wrapped in `Mutex` so `&self` methods can access it.
    writer: Mutex<IndexWriter>,
    /// Reader — refreshed by `reload_reader()`.
    reader: Mutex<IndexReader>,
    /// Original logical schema (for schema introspection / FT.INFO).
    pub schema_def: FtSchema,
}

impl FtIndex {
    // -----------------------------------------------------------------------
    // Construction
    // -----------------------------------------------------------------------

    /// Create a new in-RAM index from the given logical schema.
    #[instrument(skip(schema))]
    pub fn create_in_ram(schema: FtSchema) -> Result<Self, FtError> {
        let mut builder = Schema::builder();

        // Always add the primary-key stored TEXT field (raw tokenizer = exact match).
        let pk_options = TextOptions::default()
            .set_stored()
            .set_indexing_options(
                TextFieldIndexing::default()
                    .set_tokenizer("raw")
                    .set_index_option(IndexRecordOption::Basic),
            );
        let pk_field = builder.add_text_field(PK_FIELD, pk_options);

        let mut field_map: HashMap<String, Field> = HashMap::new();

        for fdef in &schema.fields {
            let field = Self::add_schema_field(&mut builder, fdef)?;
            field_map.insert(fdef.name.clone(), field);
        }

        let tv_schema = builder.build();
        let index = Index::create_in_ram(tv_schema.clone());

        // 16 MB heap for the writer — sufficient for in-memory MVP.
        let writer: IndexWriter = index.writer(16_000_000).map_err(FtError::Tantivy)?;
        let reader = index.reader().map_err(FtError::Tantivy)?;

        debug!(fields = schema.fields.len(), "FtIndex created in RAM");

        Ok(Self {
            index,
            _tv_schema: tv_schema,
            field_map,
            pk_field,
            writer: Mutex::new(writer),
            reader: Mutex::new(reader),
            schema_def: schema,
        })
    }

    /// Map one logical `FieldDef` to Tantivy builder fields, return the
    /// Tantivy `Field` handle.
    fn add_schema_field(
        builder: &mut tv_schema::SchemaBuilder,
        fdef: &FieldDef,
    ) -> Result<Field, FtError> {
        let field = match &fdef.ty {
            FieldType::Text { tokenized, .. } => {
                let tokenizer = if *tokenized { "en_stem" } else { "raw" };
                let idx_opts = TextFieldIndexing::default()
                    .set_tokenizer(tokenizer)
                    .set_index_option(IndexRecordOption::WithFreqsAndPositions);
                let text_opts = if fdef.stored {
                    TextOptions::default().set_stored().set_indexing_options(idx_opts)
                } else {
                    TextOptions::default().set_indexing_options(idx_opts)
                };
                builder.add_text_field(&fdef.name, text_opts)
            }
            FieldType::Keyword => {
                let idx_opts = TextFieldIndexing::default()
                    .set_tokenizer("raw")
                    .set_index_option(IndexRecordOption::Basic);
                let text_opts = if fdef.stored {
                    TextOptions::default().set_stored().set_indexing_options(idx_opts)
                } else {
                    TextOptions::default().set_indexing_options(idx_opts)
                };
                builder.add_text_field(&fdef.name, text_opts)
            }
            FieldType::Tag { .. } => {
                // Tags are stored as raw strings; searching via the "raw"
                // tokenizer ensures exact-value matching.
                let idx_opts = TextFieldIndexing::default()
                    .set_tokenizer("raw")
                    .set_index_option(IndexRecordOption::Basic);
                let text_opts = if fdef.stored {
                    TextOptions::default().set_stored().set_indexing_options(idx_opts)
                } else {
                    TextOptions::default().set_indexing_options(idx_opts)
                };
                builder.add_text_field(&fdef.name, text_opts)
            }
            FieldType::Numeric { sortable, .. } => {
                let mut num_opts = NumericOptions::default().set_indexed();
                if *sortable {
                    num_opts = num_opts.set_fast();
                }
                if fdef.stored {
                    num_opts = num_opts.set_stored();
                }
                builder.add_f64_field(&fdef.name, num_opts)
            }
            FieldType::Geo => {
                // Geo is stored as a text "lat,lon" — simple MVP approach.
                let text_opts = if fdef.stored {
                    TextOptions::default() | STORED
                } else {
                    TextOptions::default()
                };
                builder.add_text_field(&fdef.name, text_opts)
            }
            FieldType::Date => {
                let mut num_opts = NumericOptions::default().set_indexed().set_fast();
                if fdef.stored {
                    num_opts = num_opts.set_stored();
                }
                builder.add_i64_field(&fdef.name, num_opts)
            }
        };
        Ok(field)
    }

    // -----------------------------------------------------------------------
    // Write path
    // -----------------------------------------------------------------------

    /// Index a document. `doc_id` acts as the primary key.
    /// Returns the internal Tantivy operation stamp as `u64`.
    pub fn add_document(
        &self,
        doc_id: &str,
        fields: HashMap<String, FieldValue>,
    ) -> Result<u64, FtError> {
        let mut doc = TantivyDocument::default();

        // Primary key.
        doc.add_text(self.pk_field, doc_id);

        for (name, value) in &fields {
            let tv_field = match self.field_map.get(name) {
                Some(f) => *f,
                None => continue, // silently skip unknown fields
            };

            match value {
                FieldValue::Text(s) | FieldValue::Tag(s) => {
                    doc.add_text(tv_field, s.as_str());
                }
                FieldValue::Numeric(n) => {
                    doc.add_f64(tv_field, *n);
                }
                FieldValue::Geo { lat, lon } => {
                    doc.add_text(tv_field, &format!("{lat},{lon}"));
                }
                FieldValue::Date(ms) => {
                    doc.add_i64(tv_field, *ms);
                }
            }
        }

        let stamp = self
            .writer
            .lock()
            .map_err(|_| FtError::WriterUnavailable)?
            .add_document(doc)
            .map_err(FtError::Tantivy)?;

        debug!(doc_id, "document indexed");
        Ok(stamp.into())
    }

    /// Delete all documents where `field` equals `value` (exact term match).
    pub fn delete_by_term(&self, field: &str, value: &str) -> Result<u64, FtError> {
        let tv_field = if field == PK_FIELD {
            self.pk_field
        } else {
            *self
                .field_map
                .get(field)
                .ok_or_else(|| FtError::FieldNotFound(field.to_owned()))?
        };

        let term = tantivy::Term::from_field_text(tv_field, value);
        self.writer
            .lock()
            .map_err(|_| FtError::WriterUnavailable)?
            .delete_term(term);
        Ok(1)
    }

    /// Flush pending writes to the index segment (makes them searchable after
    /// a subsequent `reload_reader`).
    pub fn commit(&self) -> Result<(), FtError> {
        self.writer
            .lock()
            .map_err(|_| FtError::WriterUnavailable)?
            .commit()
            .map_err(FtError::Tantivy)?;
        Ok(())
    }

    /// Reload the reader so searches see the latest committed documents.
    pub fn reload_reader(&self) -> Result<(), FtError> {
        self.reader
            .lock()
            .map_err(|_| FtError::WriterUnavailable)?
            .reload()
            .map_err(FtError::Tantivy)?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Read path
    // -----------------------------------------------------------------------

    /// Search the index.
    pub fn search(
        &self,
        query_str: &str,
        limit: usize,
        _sort_by: Option<&SortBy>,
    ) -> Result<Vec<SearchHit>, FtError> {
        let tantivy_query_str = translate_to_tantivy(query_str);

        let reader = self.reader.lock().map_err(|_| FtError::WriterUnavailable)?;
        let searcher = reader.searcher();

        // Build query parser over all user-facing text fields.
        let fields: Vec<Field> = self.field_map.values().copied().collect();
        let parser = QueryParser::for_index(&self.index, fields);

        let query: Box<dyn tantivy::query::Query> = if tantivy_query_str == "*" {
            Box::new(tantivy::query::AllQuery)
        } else {
            parser
                .parse_query(&tantivy_query_str)
                .map_err(FtError::TantivyQueryParse)?
        };

        let top_docs = searcher
            .search(&*query, &TopDocs::with_limit(limit))
            .map_err(FtError::Tantivy)?;

        let mut hits = Vec::with_capacity(top_docs.len());
        for (score, doc_addr) in top_docs {
            let retrieved: TantivyDocument =
                searcher.doc(doc_addr).map_err(FtError::Tantivy)?;

            let doc_id = retrieved
                .get_first(self.pk_field)
                .and_then(|v| match v {
                    OwnedValue::Str(s) => Some(s.clone()),
                    _ => None,
                })
                .unwrap_or_default();

            let mut field_values: HashMap<String, FieldValue> = HashMap::new();
            for (name, tv_field) in &self.field_map {
                if let Some(val) = retrieved.get_first(*tv_field) {
                    if let Some(fv) = self.owned_value_to_field_value(name, val) {
                        field_values.insert(name.clone(), fv);
                    }
                }
            }

            hits.push(SearchHit {
                doc_id,
                score,
                fields: field_values,
            });
        }

        Ok(hits)
    }

    /// Aggregate documents by counting occurrences of each unique value in
    /// `group_by`.
    pub fn aggregate(&self, group_by: &str) -> Result<HashMap<String, u64>, FtError> {
        let tv_field = *self
            .field_map
            .get(group_by)
            .ok_or_else(|| FtError::FieldNotFound(group_by.to_owned()))?;

        let reader = self.reader.lock().map_err(|_| FtError::WriterUnavailable)?;
        let searcher = reader.searcher();

        let all_query: Box<dyn tantivy::query::Query> = Box::new(tantivy::query::AllQuery);
        let top_docs = searcher
            .search(&*all_query, &TopDocs::with_limit(1_000_000))
            .map_err(FtError::Tantivy)?;

        let mut counts: HashMap<String, u64> = HashMap::new();
        for (_, doc_addr) in top_docs {
            let doc: TantivyDocument = searcher.doc(doc_addr).map_err(FtError::Tantivy)?;
            if let Some(val) = doc.get_first(tv_field) {
                let key = match val {
                    OwnedValue::Str(s) => s.clone(),
                    OwnedValue::F64(n) => n.to_string(),
                    OwnedValue::I64(n) => n.to_string(),
                    OwnedValue::U64(n) => n.to_string(),
                    _ => continue,
                };
                *counts.entry(key).or_insert(0) += 1;
            }
        }

        Ok(counts)
    }

    /// Return the number of committed documents in the index.
    pub fn num_docs(&self) -> u64 {
        match self.reader.lock() {
            Ok(r) => r.searcher().num_docs(),
            Err(_) => 0,
        }
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    fn owned_value_to_field_value(
        &self,
        field_name: &str,
        val: &OwnedValue,
    ) -> Option<FieldValue> {
        let fdef = self.schema_def.field(field_name)?;
        match (&fdef.ty, val) {
            (FieldType::Text { .. }, OwnedValue::Str(s)) => {
                Some(FieldValue::Text(s.clone()))
            }
            (FieldType::Keyword, OwnedValue::Str(s)) => {
                Some(FieldValue::Text(s.clone()))
            }
            (FieldType::Tag { .. }, OwnedValue::Str(s)) => {
                Some(FieldValue::Tag(s.clone()))
            }
            (FieldType::Numeric { .. }, OwnedValue::F64(n)) => {
                Some(FieldValue::Numeric(*n))
            }
            (FieldType::Date, OwnedValue::I64(n)) => Some(FieldValue::Date(*n)),
            (FieldType::Geo, OwnedValue::Str(s)) => {
                let mut parts = s.splitn(2, ',');
                let lat = parts.next()?.parse().ok()?;
                let lon = parts.next()?.parse().ok()?;
                Some(FieldValue::Geo { lat, lon })
            }
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{FieldDef, FieldType, FtSchema};

    fn make_schema() -> FtSchema {
        let mut s = FtSchema::new();
        s.add_field(FieldDef {
            name: "title".into(),
            ty: FieldType::Text { tokenized: true, analyzer: None, boost: 1.0 },
            stored: true,
        });
        s.add_field(FieldDef {
            name: "price".into(),
            ty: FieldType::Numeric { indexed: true, sortable: true },
            stored: true,
        });
        s.add_field(FieldDef {
            name: "category".into(),
            ty: FieldType::Tag { separator: ',', case_sensitive: false },
            stored: true,
        });
        s
    }

    #[test]
    fn create_index_and_add_doc() {
        let idx = FtIndex::create_in_ram(make_schema()).unwrap();
        let mut fields = HashMap::new();
        fields.insert("title".into(), FieldValue::Text("hello world".into()));
        fields.insert("price".into(), FieldValue::Numeric(42.0));
        idx.add_document("doc1", fields).unwrap();
        idx.commit().unwrap();
        idx.reload_reader().unwrap();
        assert_eq!(idx.num_docs(), 1);
    }

    #[test]
    fn search_returns_hit() {
        let idx = FtIndex::create_in_ram(make_schema()).unwrap();
        let mut fields = HashMap::new();
        fields.insert("title".into(), FieldValue::Text("kaya database".into()));
        idx.add_document("doc2", fields).unwrap();
        idx.commit().unwrap();
        idx.reload_reader().unwrap();
        let hits = idx.search("@title:kaya", 10, None).unwrap();
        assert!(!hits.is_empty(), "should find at least one hit");
        assert_eq!(hits[0].doc_id, "doc2");
    }

    #[test]
    fn delete_by_pk_removes_doc() {
        let idx = FtIndex::create_in_ram(make_schema()).unwrap();
        let mut fields = HashMap::new();
        fields.insert("title".into(), FieldValue::Text("to be deleted".into()));
        idx.add_document("doc3", fields).unwrap();
        idx.commit().unwrap();
        idx.reload_reader().unwrap();
        assert_eq!(idx.num_docs(), 1);
        idx.delete_by_term(PK_FIELD, "doc3").unwrap();
        idx.commit().unwrap();
        idx.reload_reader().unwrap();
        assert_eq!(idx.num_docs(), 0);
    }
}

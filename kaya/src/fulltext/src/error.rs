//! Error types for kaya-fulltext.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum FtError {
    #[error("index not found: {0}")]
    IndexNotFound(String),

    #[error("index already exists: {0}")]
    IndexAlreadyExists(String),

    #[error("field not found in schema: {0}")]
    FieldNotFound(String),

    #[error("schema error: {0}")]
    Schema(String),

    #[error("query parse error: {0}")]
    QueryParse(String),

    #[error("tantivy error: {0}")]
    Tantivy(#[from] tantivy::TantivyError),

    #[error("tantivy query parse error: {0}")]
    TantivyQueryParse(#[from] tantivy::query::QueryParserError),

    #[error("document field type mismatch: field={field}, expected={expected}")]
    FieldTypeMismatch { field: String, expected: String },

    #[error("writer not available")]
    WriterUnavailable,

    #[error("alias not found: {0}")]
    AliasNotFound(String),
}

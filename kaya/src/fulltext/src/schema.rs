//! Schema definitions for full-text indexes.
//!
//! Defines the field types and schema structure used when creating a new
//! full-text index via FT.CREATE.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// FieldType
// ---------------------------------------------------------------------------

/// The type of a field in a full-text schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FieldType {
    /// Free-text field: optionally tokenized, with an optional analyzer name
    /// and a BM25 boost multiplier.
    Text {
        tokenized: bool,
        analyzer: Option<String>,
        boost: f32,
    },

    /// Exact-match string (stored as raw bytes, no tokenization).
    Keyword,

    /// 64-bit IEEE-754 numeric field.
    Numeric { indexed: bool, sortable: bool },

    /// Tag field: the raw value is split on `separator`, each token is
    /// lowercased unless `case_sensitive` is set.
    Tag { separator: char, case_sensitive: bool },

    /// Geo point — stored as (latitude, longitude) pair.
    Geo,

    /// RFC-3339 date string, stored as a millisecond timestamp.
    Date,
}

// ---------------------------------------------------------------------------
// FieldDef
// ---------------------------------------------------------------------------

/// Definition of a single field in a schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDef {
    /// Field name (must be unique within a schema).
    pub name: String,

    /// Logical type of the field.
    pub ty: FieldType,

    /// Whether the original field value should be retrievable via FT.SEARCH.
    pub stored: bool,
}

// ---------------------------------------------------------------------------
// FtSchema
// ---------------------------------------------------------------------------

/// Schema for a full-text index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FtSchema {
    /// Ordered list of field definitions.
    pub fields: Vec<FieldDef>,
}

impl FtSchema {
    /// Construct an empty schema.
    pub fn new() -> Self {
        Self { fields: Vec::new() }
    }

    /// Return the field definition for `name`, if present.
    pub fn field(&self, name: &str) -> Option<&FieldDef> {
        self.fields.iter().find(|f| f.name == name)
    }

    /// Append a new field definition.
    pub fn add_field(&mut self, field: FieldDef) {
        self.fields.push(field);
    }
}

impl Default for FtSchema {
    fn default() -> Self {
        Self::new()
    }
}

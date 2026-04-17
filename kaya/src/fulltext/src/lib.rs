//! KAYA FullText: Tantivy-backed full-text search.
//!
//! Provides RESP3-compatible full-text search commands (FT.*) with a
//! Tantivy engine running entirely in RAM.  The public surface is:
//!
//! - [`schema`] — field type definitions and schema builder.
//! - [`index`]  — single-index wrapper around `tantivy::Index`.
//! - [`store`]  — named index registry (`FtStore`).
//! - [`query`]  — query syntax translator (FT dialect → Tantivy query string).
//! - [`error`]  — `FtError` error type.

pub mod error;
pub mod index;
pub mod query;
pub mod schema;
pub mod store;

pub use error::FtError;
pub use index::{FieldValue, FtIndex, SearchHit, SortBy};
pub use schema::{FieldDef, FieldType, FtSchema};
pub use store::{FtStore, IndexInfo};

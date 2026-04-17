//! KAYA Vector: HNSW approximate nearest neighbour search (RedisSearch vector parity).
//!
//! This crate provides:
//! - [`distance::DistanceMetric`] — Cosine, L2, IP metrics.
//! - [`index::VectorIndex`] — single HNSW index with tombstone-based delete.
//! - [`store::VectorStore`] — registry of named indexes with alias support.
//! - [`error::VectorError`] — typed error enum.
//!
//! Command-level handlers (FT.*) live in `kaya-commands::vector`.

pub mod distance;
pub mod error;
pub mod index;
pub mod store;

pub use distance::DistanceMetric;
pub use error::VectorError;
pub use index::{IndexOpts, VectorIndex};
pub use store::{Filter, IndexInfo, VectorStore};

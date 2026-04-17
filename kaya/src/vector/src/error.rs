//! Error types for the KAYA Vector engine.

use thiserror::Error;

/// All errors that can be produced by the vector subsystem.
#[derive(Debug, Error)]
pub enum VectorError {
    /// The index does not exist.
    #[error("vector index not found: {0}")]
    IndexNotFound(String),

    /// An index with that name already exists.
    #[error("vector index already exists: {0}")]
    IndexAlreadyExists(String),

    /// The supplied vector has the wrong dimensionality.
    #[error("dimension mismatch: index expects {expected} but got {got}")]
    DimMismatch { expected: usize, got: usize },

    /// The HNSW graph is empty (no points inserted yet).
    #[error("vector index is empty")]
    IndexEmpty,

    /// A query vector or ID was malformed.
    #[error("invalid vector data: {0}")]
    InvalidData(String),

    /// A filter predicate was requested but is not yet implemented.
    #[error("filters are not yet implemented (V3.2 limitation)")]
    FilterNotImplemented,

    /// The operation requested is not supported for the given index type.
    #[error("operation not supported: {0}")]
    NotSupported(String),

    /// An internal error that should never be visible to users.
    #[error("internal vector engine error: {0}")]
    Internal(String),
}

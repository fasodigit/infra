//! Store error types.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("key not found")]
    NotFound,

    #[error("wrong type: expected {expected}, got {actual}")]
    WrongType { expected: String, actual: String },

    #[error("value is not a valid integer")]
    NotAnInteger,

    #[error("integer overflow")]
    IntegerOverflow,

    #[error("compression error: {0}")]
    Compression(String),

    #[error("memory limit exceeded")]
    MemoryLimitExceeded,

    #[error("key already exists")]
    AlreadyExists,

    #[error("internal error: {0}")]
    Internal(String),
}

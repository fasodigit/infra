//! Error types for `kaya-timeseries`.

use thiserror::Error;

/// Errors that can occur within a single compressed [`crate::chunk::Chunk`].
#[derive(Debug, Error, PartialEq)]
pub enum ChunkError {
    #[error("chunk is full (capacity {capacity} reached)")]
    Full { capacity: usize },

    #[error("timestamp {ts} is not monotonically increasing (last was {last})")]
    OutOfOrder { ts: i64, last: i64 },

    #[error("chunk is empty")]
    Empty,
}

/// Top-level errors for the TimeSeries subsystem.
#[derive(Debug, Error)]
pub enum TsError {
    #[error("series not found: {0}")]
    NotFound(String),

    #[error("series already exists: {0}")]
    AlreadyExists(String),

    #[error("duplicate timestamp {ts} rejected by policy {policy}")]
    DuplicateBlocked { ts: i64, policy: String },

    #[error("chunk error: {0}")]
    Chunk(#[from] ChunkError),

    #[error("invalid argument: {0}")]
    InvalidArgument(String),

    #[error("label filter error: {0}")]
    LabelFilter(String),

    #[error("compaction rule error: {0}")]
    CompactionRule(String),
}

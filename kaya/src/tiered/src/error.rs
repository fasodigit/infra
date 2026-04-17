//! Error types for the tiered storage engine.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum TieredError {
    #[error("fjall error: {0}")]
    Fjall(#[from] fjall::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("key not found: {0}")]
    NotFound(String),

    #[error("compression error: {0}")]
    Compression(String),

    #[error("store error: {0}")]
    Store(#[from] kaya_store::StoreError),

    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("policy error: {0}")]
    Policy(String),

    #[error("migration in progress")]
    MigrationInProgress,

    #[error("internal error: {0}")]
    Internal(String),
}

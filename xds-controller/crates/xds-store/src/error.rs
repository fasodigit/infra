// Store error types for KAYA-backed configuration storage.

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("resource not found: {kind}/{name}")]
    NotFound { kind: String, name: String },

    #[error("resource already exists: {kind}/{name}")]
    AlreadyExists { kind: String, name: String },

    #[error("version conflict: expected {expected}, found {actual}")]
    VersionConflict { expected: u64, actual: u64 },

    #[error("KAYA connection error: {0}")]
    Connection(String),

    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("internal store error: {0}")]
    Internal(String),
}

impl From<serde_json::Error> for StoreError {
    fn from(err: serde_json::Error) -> Self {
        StoreError::Serialization(err.to_string())
    }
}

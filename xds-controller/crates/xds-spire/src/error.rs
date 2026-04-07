// SPIRE integration error types.

#[derive(Debug, thiserror::Error)]
pub enum SpireError {
    #[error("SPIRE workload API connection failed: {0}")]
    ConnectionFailed(String),

    #[error("SVID fetch failed for {spiffe_id}: {reason}")]
    SvidFetchFailed { spiffe_id: String, reason: String },

    #[error("certificate rotation failed: {0}")]
    RotationFailed(String),

    #[error("invalid SPIFFE ID: {0}")]
    InvalidSpiffeId(String),

    #[error("SPIRE agent unavailable at {socket_path}")]
    AgentUnavailable { socket_path: String },

    #[error("internal SPIRE error: {0}")]
    Internal(String),
}

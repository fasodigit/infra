//! Stream error types.

use crate::StreamId;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StreamError {
    #[error("invalid stream ID: {0}")]
    InvalidId(String),

    #[error("stream not found: {0}")]
    StreamNotFound(String),

    #[error("consumer group already exists: {0}")]
    GroupExists(String),

    #[error("consumer group not found: {0}")]
    GroupNotFound(String),

    #[error("consumer not found: {0}")]
    ConsumerNotFound(String),

    #[error("ID {given} is <= last ID {last}; IDs must be monotonically increasing")]
    IdTooSmall { given: StreamId, last: StreamId },

    #[error("stream error: {0}")]
    Internal(String),
}

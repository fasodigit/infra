//! Error types for the KAYA Pub/Sub protocol.

use thiserror::Error;

/// Errors that can occur within the KAYA Pub/Sub subsystem.
#[derive(Debug, Error)]
pub enum PubSubError {
    /// The subscriber channel has been closed (receiver dropped).
    #[error("subscriber channel closed")]
    ChannelClosed,

    /// A glob pattern could not be parsed.
    #[error("invalid pattern: {0}")]
    PatternInvalid(String),

    /// The targeted shard in the Sharded Pub/Sub ring is unavailable.
    #[error("shard {0} unavailable")]
    ShardUnavailable(usize),
}

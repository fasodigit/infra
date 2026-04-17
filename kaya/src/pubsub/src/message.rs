//! Message types exchanged between the broker and subscribers.

use bytes::Bytes;

use crate::pattern::Pattern;

/// Opaque identifier for a connected client.
///
/// KAYA assigns a stable `ClientId` per connection at the network layer; the
/// Pub/Sub broker stores it only so the publisher can filter its own messages
/// if needed (parity with RESP3 Pub/Sub semantics).
pub type ClientId = u64;

/// A message delivered to a subscriber.
///
/// `pattern` is `Some` when the subscription was created via `PSUBSCRIBE`
/// and the message matches the pattern; for exact `SUBSCRIBE` deliveries it
/// is `None`.
#[derive(Debug, Clone)]
pub struct PubSubMessage {
    /// Channel the message was published to.
    pub channel: Bytes,
    /// Raw payload bytes.
    pub payload: Bytes,
    /// Originating pattern (for `PSUBSCRIBE` deliveries) or `None`.
    pub pattern: Option<Pattern>,
    /// Source client identifier if provided by the publisher.
    pub source_client: Option<ClientId>,
}

impl PubSubMessage {
    /// Build a new exact-channel message.
    pub fn exact(channel: Bytes, payload: Bytes) -> Self {
        Self {
            channel,
            payload,
            pattern: None,
            source_client: None,
        }
    }

    /// Build a new pattern-match message.
    pub fn pattern(channel: Bytes, payload: Bytes, pattern: Pattern) -> Self {
        Self {
            channel,
            payload,
            pattern: Some(pattern),
            source_client: None,
        }
    }

    /// Attach a source client identifier.
    pub fn with_source(mut self, client: ClientId) -> Self {
        self.source_client = Some(client);
        self
    }
}

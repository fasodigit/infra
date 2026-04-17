//! Per-client RESP3 connection state used by the tracking subsystem.
//!
//! The core TCP loop in [`crate::Connection`] is designed to be synchronous
//! on the request/response cycle. For RESP3 client-side caching the server
//! must, in addition, be able to push `invalidate` frames out-of-band. This
//! module provides the glue types used by the network layer to:
//!
//! * allocate a stable [`ClientId`] per incoming TCP connection,
//! * own the outbound `mpsc::Sender<Frame>` that the tracking table uses to
//!   deliver push frames,
//! * remember the negotiated RESP protocol version and the tracking mode.
//!
//! This module does not perform I/O itself — it is a description of the
//! per-connection state that the RESP3 request pipeline keeps alongside the
//! socket. The push drain task is spawned by [`Connection::run`] when the
//! client upgrades to RESP3 via `HELLO 3`.

use std::sync::atomic::{AtomicU64, Ordering};

use tokio::sync::mpsc;

use kaya_protocol::Frame;

use crate::tracking::{ClientId, TrackingMode};

/// Monotonic allocator of [`ClientId`] values. Starts at 1 (RESP convention —
/// clients are 1-indexed).
static CLIENT_ID_SEQ: AtomicU64 = AtomicU64::new(1);

/// Allocate a fresh, globally-unique [`ClientId`].
pub fn next_client_id() -> ClientId {
    CLIENT_ID_SEQ.fetch_add(1, Ordering::Relaxed)
}

/// Capacity of the per-connection push channel. Chosen so a modest burst of
/// invalidations cannot block the store hot path.
pub const PUSH_CHANNEL_CAPACITY: usize = 1024;

/// Convenience wrapper around the push-side of a client connection.
///
/// Created by the network layer when a new TCP connection is accepted. The
/// receiving half ([`ClientPushRx`]) is consumed by the background writer
/// task that drains push frames to the socket; the sending half
/// ([`ClientPushTx`]) is registered with the [`crate::tracking::TrackingTable`].
pub struct ClientChannel {
    pub tx: ClientPushTx,
    pub rx: ClientPushRx,
}

/// Sender end of the push channel.
pub type ClientPushTx = mpsc::Sender<Frame>;

/// Receiver end of the push channel.
pub type ClientPushRx = mpsc::Receiver<Frame>;

impl ClientChannel {
    /// Create a new bounded push channel.
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel(PUSH_CHANNEL_CAPACITY);
        Self { tx, rx }
    }
}

impl Default for ClientChannel {
    fn default() -> Self {
        Self::new()
    }
}

/// Summary of the state the network layer keeps for a RESP3-capable client.
///
/// Stored directly on [`crate::Connection`]; exposed here so tests and
/// command handlers can reason about a client even without a live socket.
pub struct ClientConnection {
    /// Stable server-side id for this client.
    pub id: ClientId,
    /// Negotiated RESP version: `2` (legacy) or `3` (push-capable).
    pub resp_version: u8,
    /// Current tracking mode for this client.
    pub tracking: TrackingMode,
    /// Push channel sender (registered in the tracking table's sender map).
    pub sender: ClientPushTx,
}

impl ClientConnection {
    /// Build a RESP2 client wrapping the provided push sender. Tracking
    /// starts [`TrackingMode::Off`].
    pub fn new(id: ClientId, sender: ClientPushTx) -> Self {
        Self {
            id,
            resp_version: 2,
            tracking: TrackingMode::Off,
            sender,
        }
    }

    /// Promote this client to RESP3.
    pub fn upgrade_to_resp3(&mut self) {
        self.resp_version = 3;
    }

    /// True if the client has completed the RESP3 handshake.
    pub fn is_resp3(&self) -> bool {
        self.resp_version >= 3
    }

    /// Send a push frame out-of-band. Drops the frame if the channel is
    /// full or closed (tracing a warning) — invalidations are advisory,
    /// and a missed invalidation simply means the client must refresh its
    /// local cache on next access.
    pub fn push(&self, frame: Frame) {
        match self.sender.try_send(frame) {
            Ok(()) => {}
            Err(mpsc::error::TrySendError::Full(_)) => {
                tracing::warn!(client_id = self.id, "push channel full, dropping frame");
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                tracing::debug!(client_id = self.id, "push channel closed");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaya_protocol::Frame;

    #[tokio::test]
    async fn push_channel_delivers_frames() {
        let mut chan = ClientChannel::new();
        let client = ClientConnection::new(42, chan.tx.clone());
        client.push(Frame::ok());
        let got = chan.rx.recv().await.expect("a frame");
        assert_eq!(got, Frame::ok());
    }

    #[tokio::test]
    async fn resp_upgrade_tracks_version() {
        let chan = ClientChannel::new();
        let mut client = ClientConnection::new(1, chan.tx);
        assert_eq!(client.resp_version, 2);
        assert!(!client.is_resp3());
        client.upgrade_to_resp3();
        assert!(client.is_resp3());
    }

    #[test]
    fn client_ids_are_monotonic() {
        let a = next_client_id();
        let b = next_client_id();
        assert!(b > a);
    }
}

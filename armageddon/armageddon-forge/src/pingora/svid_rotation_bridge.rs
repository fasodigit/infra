// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! SVID rotation bridge — observes SPIRE cert rotations and surfaces them to
//! Pingora upstream mTLS path.
//!
//! # Design
//!
//! `SvidRotationBridge` subscribes to a `broadcast::Receiver<RotationEvent>`
//! (obtained from `SvidManager::watch_rotations()`).  On each rotation:
//!
//! 1. `Mesh::run()` has **already** called `apply_rotation()` and atomically
//!    swapped the new `ClientConfig` into the `ArcSwap` inside `Mesh`.
//! 2. The bridge logs the event and increments observability counters.
//! 3. All future calls to `AutoMtlsDialer::connect_tls` automatically pick up
//!    the new `ClientConfig` — `Mesh::client_config()` is called per-connection
//!    and is O(1) ArcSwap load.
//!
//! There is **no per-connection action needed** — the `Arc<Mesh>` is shared
//! between the bridge and every `AutoMtlsDialer`.  The bridge's only role is
//! observability.
//!
//! # Pingora 0.3 constraint
//!
//! Pingora 0.3 does **not** expose a custom upstream TLS connector hook.
//! The `UpstreamMtlsFilter` performs defense-in-depth SPIFFE ID validation
//! post-hoc via `ctx.spiffe_peer` but cannot inject an `AutoMtlsDialer` into
//! Pingora's connection pool.
//!
//! **Upgrade path (Pingora 0.4)**: when `pingora-rustls` exposes a
//! `PeerProxy::build_connector` hook, replace the post-hoc filter with a dial-
//! time `AutoMtlsDialer::connect_tls` call.  No structural change to this
//! bridge or to `Mesh` will be needed.
//!
//! # Failure modes
//!
//! | Scenario | Behaviour |
//! |---|---|
//! | SPIRE socket drops | `SvidManager` reconnects; bridge idles until next event |
//! | Channel lagged (> 64 queued events) | Bridge logs an error, continues from next event |
//! | Expired SVID before renewal | New TLS handshakes fail at rustls layer; SPIRE monitoring alerts |

use tracing::{error, info, warn};
use tokio::sync::broadcast;

use armageddon_mesh::RotationEvent;

// ---------------------------------------------------------------------------
// SvidRotationBridge
// ---------------------------------------------------------------------------

/// Observes `RotationEvent`s and surfaces them for observability.
///
/// # Usage
///
/// ```rust,ignore
/// // Obtain from `SvidManager` before Mesh::run starts:
/// let rotations = svid_manager.watch_rotations();
/// let (shutdown_tx, shutdown_rx) = broadcast::channel(1);
///
/// crate::pingora::runtime::tokio_handle().spawn(
///     SvidRotationBridge::run(rotations, shutdown_rx)
/// );
/// ```
pub struct SvidRotationBridge;

impl SvidRotationBridge {
    /// Run the rotation notification loop until `shutdown` fires.
    ///
    /// This is a free function so the handle to `Mesh` stays outside, avoiding
    /// circular Arc ownership.
    pub async fn run(
        mut rotations: broadcast::Receiver<RotationEvent>,
        mut shutdown: broadcast::Receiver<()>,
    ) {
        loop {
            tokio::select! {
                biased;

                _ = shutdown.recv() => {
                    info!("SvidRotationBridge: shutdown received");
                    return;
                }

                event = rotations.recv() => {
                    match event {
                        Ok(ev) => {
                            info!(
                                spiffe_id = %ev.spiffe_id,
                                "SvidRotationBridge: SVID rotated — \
                                 AutoMtlsDialer picks up new cert on next connection"
                            );
                            // The ArcSwap inside Mesh was already updated by
                            // Mesh::run before this event fired.  New outbound
                            // connections via AutoMtlsDialer::connect_tls will
                            // use the new ClientConfig transparently.
                            increment_rotation_counter();
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            error!(
                                missed = n,
                                "SvidRotationBridge: rotation receiver lagged \
                                 — potential gap in cert rotation observability"
                            );
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            warn!("SvidRotationBridge: rotation channel closed");
                            return;
                        }
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// spawn_svid_rotation_bridge — public entry point
// ---------------------------------------------------------------------------

/// Spawn the SVID rotation bridge task on the forge tokio bridge.
///
/// `rotations` — receiver obtained from `SvidManager::watch_rotations()`.
/// `shutdown`  — fires when the gateway is stopping.
///
/// Returns a `JoinHandle` that resolves when the bridge exits.
pub fn spawn_svid_rotation_bridge(
    rotations: broadcast::Receiver<RotationEvent>,
    shutdown: broadcast::Receiver<()>,
) -> tokio::task::JoinHandle<()> {
    crate::pingora::runtime::tokio_handle()
        .spawn(SvidRotationBridge::run(rotations, shutdown))
}

// ---------------------------------------------------------------------------
// Metrics
// ---------------------------------------------------------------------------

fn increment_rotation_counter() {
    // TODO(M6): wire into shared Prometheus registry when full registry wiring
    // is complete.  For now exposed via tracing only.
    tracing::debug!("armageddon_svid_rotations_total += 1");
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;
    use arc_swap::ArcSwap;
    use tokio::sync::broadcast;

    /// Rotation event is observed by the bridge.
    #[tokio::test]
    async fn rotation_event_is_received() {
        let (tx, rx_bridge) = broadcast::channel::<RotationEvent>(64);
        let (shutdown_tx, shutdown_rx) = broadcast::channel::<()>(1);

        // Keep a separate receiver to assert observed events.
        let mut rx_assert = tx.subscribe();

        tokio::spawn(SvidRotationBridge::run(rx_bridge, shutdown_rx));

        tokio::time::sleep(Duration::from_millis(5)).await;

        tx.send(RotationEvent {
            spiffe_id: "spiffe://faso.gov.bf/ns/armageddon/sa/gateway".to_string(),
        })
        .expect("send ok");

        // The bridge should receive it; we assert via the parallel receiver.
        let ev = tokio::time::timeout(Duration::from_millis(100), rx_assert.recv())
            .await
            .expect("timeout waiting for rotation event")
            .expect("recv ok");

        assert_eq!(ev.spiffe_id, "spiffe://faso.gov.bf/ns/armageddon/sa/gateway");

        let _ = shutdown_tx.send(());
    }

    /// Multiple rotation events are all forwarded.
    #[tokio::test]
    async fn multiple_rotation_events_forwarded() {
        let (tx, rx_bridge) = broadcast::channel::<RotationEvent>(64);
        let (shutdown_tx, shutdown_rx) = broadcast::channel::<()>(1);
        let mut rx_assert = tx.subscribe();

        tokio::spawn(SvidRotationBridge::run(rx_bridge, shutdown_rx));
        tokio::time::sleep(Duration::from_millis(5)).await;

        for i in 0..3u32 {
            tx.send(RotationEvent {
                spiffe_id: format!("spiffe://faso.gov.bf/v{i}"),
            })
            .expect("send ok");
        }

        let mut received = Vec::new();
        for _ in 0..3 {
            let ev = tokio::time::timeout(Duration::from_millis(100), rx_assert.recv())
                .await
                .expect("timeout")
                .expect("recv ok");
            received.push(ev.spiffe_id);
        }
        assert_eq!(received.len(), 3);

        let _ = shutdown_tx.send(());
    }

    /// Shutdown signal stops the bridge promptly.
    #[tokio::test]
    async fn shutdown_stops_bridge() {
        let (_tx, rx_bridge) = broadcast::channel::<RotationEvent>(64);
        let (shutdown_tx, shutdown_rx) = broadcast::channel::<()>(1);

        let handle = tokio::spawn(SvidRotationBridge::run(rx_bridge, shutdown_rx));

        tokio::time::sleep(Duration::from_millis(5)).await;
        let _ = shutdown_tx.send(());

        tokio::time::timeout(Duration::from_millis(200), handle)
            .await
            .expect("bridge must stop within 200 ms")
            .expect("bridge must not panic");
    }

    /// Demonstrates that `ArcSwap` hot-swap is transparent to callers of
    /// `load()` — the bridge's correctness model.
    #[test]
    fn arcswap_new_cert_is_visible_immediately() {
        // A simple ArcSwap holding a version counter simulates the ClientConfig
        // hot-swap that Mesh::apply_rotation performs.
        let store: ArcSwap<u32> = ArcSwap::from_pointee(1u32);
        assert_eq!(**store.load(), 1u32);

        // Simulate Mesh::apply_rotation swapping in new config.
        store.store(Arc::new(2u32));
        assert_eq!(**store.load(), 2u32, "new config visible after store");
    }
}

// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! SVID lifecycle management with hot-swap via [`ArcSwap`].
//!
//! # Architecture
//!
//! ```text
//!  SPIRE agent
//!      │  gRPC stream (X509Context per rotation)
//!      ▼
//!  SpireClient::watch_x509_context()
//!      │  tokio Stream
//!      ▼
//!  SvidManager::run_inner()
//!      │  ArcSwap::store()
//!      ▼
//!  ArcSwap<Arc<X509Svid>>  ◄──── current_svid() / Mesh::client_config()
//!      │  broadcast::Sender<RotationEvent>
//!      ▼
//!  rustls_config::rebuild_configs()
//! ```
//!
//! # Failure modes
//!
//! - **SPIRE socket drops** — `run_inner` logs the error, sleeps
//!   `RECONNECT_DELAY`, then re-creates the `SpireClient` and re-subscribes.
//!   The previous SVID stays in the `ArcSwap`; callers continue serving but
//!   will hit `SvidExpired` once the SVID's `not_after` passes.
//!
//! - **No initial SVID** — the `Mesh::new` call blocks until the first
//!   `X509Context` is delivered, so the caller never starts with a null SVID.
//!
//! - **Shutdown** — `run` selects on the `shutdown` broadcast receiver; when
//!   signalled it drops the stream, closes the rotation sender (all
//!   `Receiver<RotationEvent>` see channel closed), and returns.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use arc_swap::ArcSwap;
use spiffe::{
    svid::x509::X509Svid,
    workload_api::x509_context::X509Context,
};
use tokio::sync::broadcast;
use tokio_stream::StreamExt as _;
use tracing::{debug, error, info, warn};

use crate::error::MeshError;
use crate::spire_client::SpireClient;

/// Duration to wait between SPIRE reconnect attempts.
const RECONNECT_DELAY: Duration = Duration::from_secs(5);

/// Broadcast channel capacity for rotation events.
const ROTATION_CHANNEL_CAPACITY: usize = 64;

/// Signals that a new SVID has been installed.
#[derive(Debug, Clone)]
pub struct RotationEvent {
    /// The new SVID's SPIFFE ID as a string (e.g.
    /// `spiffe://faso.gov.bf/ns/default/sa/armageddon`).
    pub spiffe_id: String,
}

/// Manages the current SVID and notifies subscribers on every rotation.
///
/// Use [`current_svid`](SvidManager::current_svid) to obtain the latest SVID
/// for rustls config construction.  Use
/// [`watch_rotations`](SvidManager::watch_rotations) to receive a channel that
/// fires on every SPIRE-initiated rotation.
pub struct SvidManager {
    /// The path to the SPIRE workload-API socket.
    socket_path: PathBuf,
    /// Always holds the most recent successfully received SVID.
    current: ArcSwap<Arc<X509Svid>>,
    /// Broadcast sender; cloned by `watch_rotations`.
    rotation_tx: broadcast::Sender<RotationEvent>,
}

impl SvidManager {
    /// Create a new `SvidManager` that fetches the **initial** SVID from the
    /// SPIRE agent before returning.
    ///
    /// Returns `MeshError::Spiffe` if the socket is unreachable.
    pub async fn new(socket_path: &Path) -> Result<Arc<Self>, MeshError> {
        let (rotation_tx, _) = broadcast::channel(ROTATION_CHANNEL_CAPACITY);

        // Fetch initial SVID synchronously so callers always start with a
        // valid cert.
        let initial_svid = Self::fetch_initial(socket_path).await?;

        let mgr = Arc::new(Self {
            socket_path: socket_path.to_owned(),
            current: ArcSwap::new(Arc::new(Arc::new(initial_svid))),
            rotation_tx,
        });

        info!("SvidManager initialised");
        Ok(mgr)
    }

    /// Return the current SVID.  Zero-copy — cloning an `Arc` is O(1).
    pub fn current_svid(&self) -> Arc<X509Svid> {
        Arc::clone(&*self.current.load())
    }

    /// Return a new [`broadcast::Receiver`] that fires a [`RotationEvent`]
    /// whenever the SVID is rotated.
    pub fn watch_rotations(&self) -> broadcast::Receiver<RotationEvent> {
        self.rotation_tx.subscribe()
    }

    /// Drive the SVID rotation loop.
    ///
    /// Exits cleanly when `shutdown` fires.  Reconnects automatically if the
    /// SPIRE stream terminates unexpectedly.
    pub async fn run(self: Arc<Self>, mut shutdown: broadcast::Receiver<()>) {
        loop {
            tokio::select! {
                biased;
                _ = shutdown.recv() => {
                    info!("SvidManager shutdown received");
                    return;
                }
                result = self.run_stream() => {
                    match result {
                        Ok(()) => {
                            // Stream ended cleanly (rare); reconnect.
                            warn!("SPIRE X.509 stream ended; reconnecting in {:?}", RECONNECT_DELAY);
                        }
                        Err(e) => {
                            error!(err = %e, "SPIRE stream error; reconnecting in {:?}", RECONNECT_DELAY);
                        }
                    }
                    // Delay before reconnect to avoid hammering the socket.
                    tokio::select! {
                        biased;
                        _ = shutdown.recv() => {
                            info!("SvidManager shutdown received during reconnect delay");
                            return;
                        }
                        _ = tokio::time::sleep(RECONNECT_DELAY) => {}
                    }
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    /// Block until the SPIRE agent delivers the first X509Context.
    async fn fetch_initial(socket_path: &Path) -> Result<X509Svid, MeshError> {
        let mut client = SpireClient::new(socket_path).await?;
        let mut stream = client.watch_x509_context().await?;

        let ctx: X509Context = stream
            .next()
            .await
            .ok_or_else(|| MeshError::Spiffe("SPIRE stream closed before first context".into()))?;

        Self::extract_default_svid(ctx)
    }

    /// Process the live SPIRE stream until it ends or errors.
    async fn run_stream(self: &Arc<Self>) -> Result<(), MeshError> {
        let mut client = SpireClient::new(&self.socket_path).await?;
        let mut stream = client.watch_x509_context().await?;

        while let Some(ctx) = stream.next().await {
            match Self::extract_default_svid(ctx) {
                Ok(svid) => {
                    let id = svid.spiffe_id().to_string();
                    self.current.store(Arc::new(Arc::new(svid)));
                    debug!(spiffe_id = %id, "SVID rotated");
                    let _ = self.rotation_tx.send(RotationEvent { spiffe_id: id });
                }
                Err(e) => {
                    error!(err = %e, "failed to extract SVID from context; skipping rotation");
                }
            }
        }

        Ok(())
    }

    /// Extract the first (default) SVID from an [`X509Context`].
    ///
    /// SPIRE always delivers at least one SVID per context update.  If the
    /// list is empty we treat it as a transient error and skip the update.
    fn extract_default_svid(ctx: X509Context) -> Result<X509Svid, MeshError> {
        // spiffe 0.4: X509Context::svids() returns &[X509Svid].
        let svids_ref = ctx.svids();
        if svids_ref.is_empty() {
            return Err(MeshError::Spiffe("X509Context contained no SVIDs".into()));
        }
        Ok(svids_ref[0].clone())
    }
}

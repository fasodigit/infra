// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Thin wrapper around the `spiffe` workload-API client.
//!
//! # Failure modes
//!
//! - **Socket unreachable** — `new` returns `MeshError::Io`; the caller
//!   (typically `SvidManager`) retries with exponential back-off.
//!
//! - **Stream severed** — `watch_x509_context` returns a stream that ends
//!   (yields `None`); `SvidManager` MUST re-connect by calling `new` again
//!   and re-invoking `watch_x509_context`.
//!
//! - **No SPIRE agent live** — integration tests stub the stream via
//!   `SpireClient::from_mock_stream`; no real Unix socket is required.

use std::path::Path;

use spiffe::{
    workload_api::client::WorkloadApiClient,
    workload_api::x509_context::X509Context,
};
use tokio_stream::Stream;
use tracing::{debug, error, info};

use crate::error::MeshError;

/// Thin wrapper that owns a live `WorkloadApiClient` connection.
///
/// All interaction with the SPIRE agent goes through this type; it is kept
/// deliberately minimal so that tests can substitute a mock stream.
pub struct SpireClient {
    inner: WorkloadApiClient,
}

impl SpireClient {
    /// Connect to the SPIRE agent socket at `socket_path`.
    ///
    /// The path must point to the SPIRE workload-API Unix domain socket, e.g.
    /// `/run/spire/sockets/agent.sock`.
    ///
    /// # Errors
    ///
    /// Returns [`MeshError::Spiffe`] if the socket is unreachable or the gRPC
    /// channel cannot be established.
    pub async fn new(socket_path: &Path) -> Result<Self, MeshError> {
        let path_str = socket_path
            .to_str()
            .ok_or_else(|| MeshError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "socket path contains non-UTF-8 characters",
            )))?;

        info!(socket = %path_str, "connecting to SPIRE workload API");

        let inner = WorkloadApiClient::new_from_path(path_str)
            .await
            .map_err(|e| MeshError::Spiffe(e.to_string()))?;

        debug!("SPIRE workload API client connected");
        Ok(Self { inner })
    }

    /// Subscribe to X.509 SVID updates from the SPIRE agent.
    ///
    /// The returned stream yields one [`X509Context`] immediately (current
    /// SVID set) and then on every SVID rotation.  The stream ends if the
    /// SPIRE agent disconnects.
    pub async fn watch_x509_context(
        &mut self,
    ) -> Result<impl Stream<Item = X509Context>, MeshError> {
        use tokio_stream::StreamExt as _;
        let stream = self
            .inner
            .stream_x509_contexts()
            .await
            .map_err(|e| {
                error!(err = %e, "failed to open X.509 context stream");
                MeshError::Spiffe(e.to_string())
            })?;

        // spiffe 0.4 yields `Result<X509Context, _>`; drop the Result wrapper
        // and log errors to keep the caller signature simple.
        Ok(stream.filter_map(|r| match r {
            Ok(ctx) => Some(ctx),
            Err(e) => {
                error!(err = %e, "SPIRE X509 context stream error");
                None
            }
        }))
    }
}

// SPDX-License-Identifier: AGPL-3.0-or-later
//! Tonic gRPC client pool to terroir-core :8730.
//!
//! A small fixed-size pool of persistent `Channel`s is built at startup
//! (default 5 connections). HTTP/2 multiplexes RPCs over each, so 5 channels
//! handle thousands of concurrent calls comfortably; the pool is sized this
//! way to amortize TCP/TLS handshake cost and tolerate transient socket
//! failures without re-resolving DNS.
//!
//! Channels are cloned (cheap — they wrap an `Arc`) on each call via
//! `next()`. Round-robin index avoids hot-spotting one channel.

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use anyhow::{Context, Result};
use tonic::transport::{Channel, Endpoint};

use crate::terroir_core_grpc::core_service_client::CoreServiceClient;

/// Default pool size — tweakable via env `TERROIR_CORE_GRPC_POOL_SIZE`.
pub const DEFAULT_POOL_SIZE: usize = 5;

/// Persistent pool of `Channel` handles to terroir-core gRPC :8730.
///
/// Cloning the pool is cheap (Arc).
#[derive(Clone)]
pub struct CoreGrpcPool {
    channels: Arc<Vec<Channel>>,
    idx: Arc<AtomicUsize>,
}

impl CoreGrpcPool {
    /// Build the pool by lazily connecting `pool_size` channels.
    ///
    /// `endpoint_url` typically `http://terroir-core:8730` in container,
    /// `http://localhost:8730` in dev.
    pub async fn new(endpoint_url: &str, pool_size: usize) -> Result<Self> {
        let pool_size = pool_size.max(1);
        let endpoint: Endpoint = Endpoint::from_shared(endpoint_url.to_owned())
            .context("parse terroir-core gRPC endpoint")?
            .timeout(std::time::Duration::from_secs(30))
            .connect_timeout(std::time::Duration::from_secs(5))
            .keep_alive_while_idle(true)
            .http2_keep_alive_interval(std::time::Duration::from_secs(20))
            .keep_alive_timeout(std::time::Duration::from_secs(5));

        let mut channels = Vec::with_capacity(pool_size);
        for i in 0..pool_size {
            // `connect()` performs a real TCP/HTTP-2 handshake. Failure here
            // should NOT be fatal — terroir-core may not be up yet when the
            // BFF boots in dev. Use `connect_lazy()` so the first RPC
            // performs the dial and returns a tonic transport error if
            // terroir-core is still down.
            let chan = endpoint.clone().connect_lazy();
            tracing::debug!(idx = i, endpoint = endpoint_url, "lazy gRPC channel ready");
            channels.push(chan);
        }
        tracing::info!(
            pool_size,
            endpoint = endpoint_url,
            "terroir-core gRPC pool initialized (lazy connect)"
        );

        Ok(Self {
            channels: Arc::new(channels),
            idx: Arc::new(AtomicUsize::new(0)),
        })
    }

    /// Borrow the next channel via round-robin, cloned (cheap).
    pub fn next_channel(&self) -> Channel {
        let i = self.idx.fetch_add(1, Ordering::Relaxed) % self.channels.len();
        self.channels[i].clone()
    }

    /// Build a typed `CoreServiceClient` over the next channel.
    pub fn client(&self) -> CoreServiceClient<Channel> {
        CoreServiceClient::new(self.next_channel())
    }
}

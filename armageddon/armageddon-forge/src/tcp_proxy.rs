// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Layer-4 TCP pass-through proxy for ARMAGEDDON FORGE.
//!
//! `TcpProxy` binds a local TCP listener and, for every accepted connection,
//! selects an upstream backend via a pluggable `LoadBalancer`, then copies
//! bytes bidirectionally using `tokio::io::copy_bidirectional`.
//!
//! Metrics emitted (Prometheus counters):
//! - `armageddon_tcp_bytes_in_total`  — bytes received from downstream clients.
//! - `armageddon_tcp_bytes_out_total` — bytes sent to downstream clients.

use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tracing::{debug, info, warn};

// -- metrics --

use prometheus::{IntCounter, Registry};

/// Byte-level metrics for the L4 TCP proxy.
#[derive(Clone)]
pub struct TcpMetrics {
    /// Total bytes received from downstream clients.
    pub bytes_in: IntCounter,
    /// Total bytes sent back to downstream clients (from upstream).
    pub bytes_out: IntCounter,
}

impl TcpMetrics {
    /// Register metrics in the given Prometheus registry.
    pub fn register(registry: &Registry) -> Result<Self, prometheus::Error> {
        let bytes_in = IntCounter::new(
            "armageddon_tcp_bytes_in_total",
            "Total bytes received from downstream TCP clients",
        )?;
        let bytes_out = IntCounter::new(
            "armageddon_tcp_bytes_out_total",
            "Total bytes sent to downstream TCP clients (from upstream)",
        )?;
        registry.register(Box::new(bytes_in.clone()))?;
        registry.register(Box::new(bytes_out.clone()))?;
        Ok(Self { bytes_in, bytes_out })
    }

    /// Build unregistered metrics suitable for unit tests.
    pub fn unregistered() -> Self {
        Self {
            bytes_in: IntCounter::new(
                "armageddon_tcp_bytes_in_total_test",
                "test counter",
            )
            .unwrap(),
            bytes_out: IntCounter::new(
                "armageddon_tcp_bytes_out_total_test",
                "test counter",
            )
            .unwrap(),
        }
    }
}

// -- load balancer trait --

/// Pluggable backend-selection strategy for L4 connections.
///
/// The trait is intentionally minimal: given the current pool of upstream
/// addresses, return one to connect to.  The caller does not pass session
/// metadata beyond a `hash_key` slice so that consistent-hash variants can
/// pin a client to a stable backend.
pub trait LoadBalancer: Send + Sync {
    /// Pick one upstream address from the pool.
    ///
    /// Returns `None` when the pool is empty or all backends are unavailable.
    fn select<'a>(
        &'a self,
        upstreams: &'a [SocketAddr],
        hash_key: Option<&[u8]>,
    ) -> Option<&'a SocketAddr>;

    /// Human-readable algorithm name used in log messages.
    fn name(&self) -> &'static str;
}

// -- round-robin default --

/// Simple atomic round-robin over a fixed list of upstream addresses.
///
/// No health tracking: use a higher-level health check to prune the list
/// before passing it to `TcpProxy`.
#[derive(Debug, Default)]
pub struct RoundRobinLb {
    counter: std::sync::atomic::AtomicUsize,
}

impl RoundRobinLb {
    /// Create a new round-robin balancer starting at index 0.
    pub fn new() -> Self {
        Self {
            counter: std::sync::atomic::AtomicUsize::new(0),
        }
    }
}

impl LoadBalancer for RoundRobinLb {
    fn select<'a>(
        &'a self,
        upstreams: &'a [SocketAddr],
        _hash_key: Option<&[u8]>,
    ) -> Option<&'a SocketAddr> {
        if upstreams.is_empty() {
            return None;
        }
        let idx = self
            .counter
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            % upstreams.len();
        Some(&upstreams[idx])
    }

    fn name(&self) -> &'static str {
        "round_robin"
    }
}

// -- TcpProxy --

/// Layer-4 TCP pass-through proxy.
///
/// Binds `listen_addr`, accepts connections, selects an upstream via `lb`, and
/// copies bytes bidirectionally until either side closes.
pub struct TcpProxy {
    /// Local address to bind the listener.
    pub listen_addr: SocketAddr,
    /// Pool of upstream addresses to forward to.
    pub upstream: Vec<SocketAddr>,
    /// Load-balancing strategy.
    pub lb: Arc<dyn LoadBalancer>,
    /// Optional Prometheus metrics.  Use `TcpMetrics::unregistered()` in tests.
    pub metrics: TcpMetrics,
}

impl TcpProxy {
    /// Create a new `TcpProxy` with round-robin load balancing and unregistered
    /// (no-op) metrics.  Useful for tests and quick setup.
    pub fn new_round_robin(listen_addr: SocketAddr, upstream: Vec<SocketAddr>) -> Self {
        Self {
            listen_addr,
            upstream,
            lb: Arc::new(RoundRobinLb::new()),
            metrics: TcpMetrics::unregistered(),
        }
    }

    /// Bind and start the accept loop.
    ///
    /// The loop runs until a value is received on `shutdown` or the listener
    /// encounters a fatal I/O error.  Per-connection tasks are spawned onto the
    /// current tokio runtime and are not awaited; they complete independently.
    pub async fn run(&self, mut shutdown: broadcast::Receiver<()>) -> io::Result<()> {
        let listener = TcpListener::bind(self.listen_addr).await?;
        info!(
            "TCP proxy listening on {} (lb={})",
            self.listen_addr,
            self.lb.name()
        );

        loop {
            tokio::select! {
                // Graceful shutdown signal.
                _ = shutdown.recv() => {
                    info!("TCP proxy shutting down (signal received)");
                    return Ok(());
                }

                // Accept a new connection.
                accept_result = listener.accept() => {
                    match accept_result {
                        Ok((client_stream, client_addr)) => {
                            let upstream_addr = match self.lb.select(&self.upstream, None) {
                                Some(addr) => *addr,
                                None => {
                                    warn!("no upstream available, dropping connection from {}", client_addr);
                                    continue;
                                }
                            };

                            debug!(
                                "TCP proxy: {} -> {}",
                                client_addr, upstream_addr
                            );

                            let metrics = self.metrics.clone();
                            tokio::spawn(async move {
                                if let Err(e) = proxy_connection(
                                    client_stream,
                                    upstream_addr,
                                    metrics,
                                )
                                .await
                                {
                                    debug!("TCP proxy connection error: {}", e);
                                }
                            });
                        }
                        Err(e) => {
                            warn!("TCP proxy accept error: {}", e);
                            // Non-fatal: continue accepting.
                        }
                    }
                }
            }
        }
    }
}

// -- internal helpers --

/// Connect to `upstream_addr`, then copy bytes between `client` and the
/// upstream stream bidirectionally.  Updates byte counters on completion.
async fn proxy_connection(
    mut client: TcpStream,
    upstream_addr: SocketAddr,
    metrics: TcpMetrics,
) -> io::Result<()> {
    let mut upstream = TcpStream::connect(upstream_addr).await?;

    let (client_to_upstream, upstream_to_client) =
        tokio::io::copy_bidirectional(&mut client, &mut upstream).await?;

    metrics.bytes_in.inc_by(client_to_upstream);
    metrics.bytes_out.inc_by(upstream_to_client);

    debug!(
        "TCP proxy session closed: client->upstream {} bytes, upstream->client {} bytes",
        client_to_upstream, upstream_to_client
    );

    Ok(())
}

// -- tests --

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::sync::broadcast;

    // -----------------------------------------------------------------
    // Test 1 — round-robin selects from pool correctly
    // -----------------------------------------------------------------
    #[test]
    fn test_round_robin_lb_selection() {
        let addrs: Vec<SocketAddr> = vec![
            "127.0.0.1:9001".parse().unwrap(),
            "127.0.0.1:9002".parse().unwrap(),
            "127.0.0.1:9003".parse().unwrap(),
        ];
        let lb = RoundRobinLb::new();

        let first = *lb.select(&addrs, None).unwrap();
        let second = *lb.select(&addrs, None).unwrap();
        let third = *lb.select(&addrs, None).unwrap();
        let fourth = *lb.select(&addrs, None).unwrap();

        assert_eq!(first, addrs[0]);
        assert_eq!(second, addrs[1]);
        assert_eq!(third, addrs[2]);
        assert_eq!(fourth, addrs[0]); // wraps
    }

    // -----------------------------------------------------------------
    // Test 2 — round-robin returns None for empty pool
    // -----------------------------------------------------------------
    #[test]
    fn test_round_robin_lb_empty() {
        let lb = RoundRobinLb::new();
        assert!(lb.select(&[], None).is_none());
    }

    // -----------------------------------------------------------------
    // Test 3 — client writes X bytes → upstream receives exactly X bytes
    // -----------------------------------------------------------------
    #[tokio::test]
    async fn test_tcp_proxy_bytes_forwarded() {
        // Spin up a fake upstream that collects received bytes then closes.
        let upstream_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let upstream_addr = upstream_listener.local_addr().unwrap();

        let (received_tx, received_rx) = tokio::sync::oneshot::channel::<Vec<u8>>();

        tokio::spawn(async move {
            let (mut stream, _) = upstream_listener.accept().await.unwrap();
            let mut buf = Vec::new();
            stream.read_to_end(&mut buf).await.unwrap();
            let _ = received_tx.send(buf);
        });

        // Start the TCP proxy.
        let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_addr = proxy_listener.local_addr().unwrap();

        let proxy = TcpProxy::new_round_robin(proxy_addr, vec![upstream_addr]);
        let (shutdown_tx, shutdown_rx) = broadcast::channel::<()>(1);

        tokio::spawn(async move {
            proxy.run(shutdown_rx).await.unwrap();
        });

        // Give the proxy a moment to bind.
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        // Connect and send 42 bytes.
        let payload = b"ARMAGEDDON sovereign gateway test payload!!";
        assert_eq!(payload.len(), 42);

        let mut client = TcpStream::connect(proxy_addr).await.unwrap();
        client.write_all(payload).await.unwrap();
        // Close write side so upstream read_to_end terminates.
        client.shutdown().await.unwrap();

        let received = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            received_rx,
        )
        .await
        .expect("timeout")
        .expect("channel dropped");

        assert_eq!(received, payload);
        let _ = shutdown_tx.send(());
    }

    // -----------------------------------------------------------------
    // Test 4 — upstream close causes client to receive EOF immediately
    // -----------------------------------------------------------------
    #[tokio::test]
    async fn test_tcp_proxy_upstream_close_propagates() {
        // Upstream immediately closes without sending anything.
        let upstream_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let upstream_addr = upstream_listener.local_addr().unwrap();

        tokio::spawn(async move {
            let (stream, _) = upstream_listener.accept().await.unwrap();
            drop(stream); // immediate close
        });

        let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_addr = proxy_listener.local_addr().unwrap();

        let proxy = TcpProxy::new_round_robin(proxy_addr, vec![upstream_addr]);
        let (shutdown_tx, shutdown_rx) = broadcast::channel::<()>(1);

        tokio::spawn(async move {
            proxy.run(shutdown_rx).await.unwrap();
        });

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        let mut client = TcpStream::connect(proxy_addr).await.unwrap();
        let mut buf = Vec::new();

        // Client should see EOF quickly because upstream closed.
        let n = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            client.read_to_end(&mut buf),
        )
        .await
        .expect("timeout waiting for EOF")
        .expect("read error");

        assert_eq!(n, 0, "expected EOF with 0 bytes, got {}", n);
        let _ = shutdown_tx.send(());
    }

    // -----------------------------------------------------------------
    // Test 5 — shutdown signal stops the proxy accept loop
    // -----------------------------------------------------------------
    #[tokio::test]
    async fn test_tcp_proxy_graceful_shutdown() {
        let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_addr = proxy_listener.local_addr().unwrap();

        let upstream_addr: SocketAddr = "127.0.0.1:1".parse().unwrap(); // unreachable, never reached
        let proxy = TcpProxy::new_round_robin(proxy_addr, vec![upstream_addr]);
        let (shutdown_tx, shutdown_rx) = broadcast::channel::<()>(1);

        let handle = tokio::spawn(async move {
            proxy.run(shutdown_rx).await
        });

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        let _ = shutdown_tx.send(());

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            handle,
        )
        .await
        .expect("proxy did not shut down in time")
        .expect("join error");

        assert!(result.is_ok());
    }

    // -----------------------------------------------------------------
    // Test 6 — metrics are incremented after a proxied transfer
    // -----------------------------------------------------------------
    #[tokio::test]
    async fn test_tcp_proxy_metrics_incremented() {
        let upstream_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let upstream_addr = upstream_listener.local_addr().unwrap();

        tokio::spawn(async move {
            let (mut stream, _) = upstream_listener.accept().await.unwrap();
            // Echo 3 bytes back.
            let mut buf = [0u8; 3];
            if stream.read_exact(&mut buf).await.is_ok() {
                let _ = stream.write_all(&buf).await;
            }
        });

        let metrics = TcpMetrics::unregistered();
        let metrics_clone = metrics.clone();

        let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_addr = proxy_listener.local_addr().unwrap();

        let proxy = TcpProxy {
            listen_addr: proxy_addr,
            upstream: vec![upstream_addr],
            lb: Arc::new(RoundRobinLb::new()),
            metrics,
        };

        let (shutdown_tx, shutdown_rx) = broadcast::channel::<()>(1);

        tokio::spawn(async move {
            proxy.run(shutdown_rx).await.unwrap();
        });

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        let mut client = TcpStream::connect(proxy_addr).await.unwrap();
        client.write_all(b"ABC").await.unwrap();
        let mut reply = [0u8; 3];
        client.read_exact(&mut reply).await.unwrap();
        assert_eq!(&reply, b"ABC");
        client.shutdown().await.unwrap();

        // Allow the connection task to complete and flush counters.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // bytes_in should be >= 3 (3 bytes from client to upstream).
        assert!(metrics_clone.bytes_in.get() >= 3, "bytes_in not incremented");
        // bytes_out should be >= 3 (3 bytes echoed back to client).
        assert!(metrics_clone.bytes_out.get() >= 3, "bytes_out not incremented");

        let _ = shutdown_tx.send(());
    }
}

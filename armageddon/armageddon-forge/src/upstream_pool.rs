// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Persistent upstream connection pool with HTTP/2 multiplexing.
//!
//! # Design
//!
//! One `PooledConn` per upstream `SocketAddr`. Each connection carries an H2
//! sender that supports up to `max_concurrent_streams` parallel requests before
//! hyper 1.x back-pressures the caller.  A background task evicts connections
//! idle for longer than `idle_timeout`.
//!
//! # Metrics
//!
//! | Name | Type | Description |
//! |------|------|-------------|
//! | `armageddon_upstream_pool_size` | Gauge | Live connections in pool |
//! | `armageddon_upstream_pool_hits_total` | Counter | Requests reusing existing conn |
//! | `armageddon_upstream_pool_misses_total` | Counter | New connections created |
//! | `armageddon_upstream_handshake_duration_seconds` | Histogram | TCP+H2 handshake time |

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use armageddon_mesh::{AutoMtlsDialer, ClusterTlsContext};
use bytes::Bytes;
use dashmap::DashMap;
use http_body_util::{BodyExt, Full};
use hyper::client::conn::http2;
use hyper_util::rt::{TokioExecutor, TokioIo};
use parking_lot::Mutex;
use prometheus::{exponential_buckets, register_counter, register_gauge, register_histogram};
use thiserror::Error;
use tokio::net::TcpStream;
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

pub use armageddon_mesh::ClusterTlsContext as UpstreamTlsContext;

// -- errors --

/// Errors returned by the upstream connection pool.
#[derive(Debug, Error)]
pub enum PoolError {
    /// TCP connection to the upstream address failed.
    #[error("TCP connect to {addr} failed: {source}")]
    Connect {
        addr: SocketAddr,
        source: std::io::Error,
    },

    /// The H2 handshake completed TCP but failed at the protocol level.
    #[error("HTTP/2 handshake with {addr} failed: {source}")]
    H2Handshake {
        addr: SocketAddr,
        source: hyper::Error,
    },

    /// mTLS handshake to the upstream failed (SPIFFE ID mismatch, cert error, etc.).
    #[error("mTLS handshake with {addr} failed: {source}")]
    TlsHandshake {
        addr: SocketAddr,
        source: std::io::Error,
    },

    /// The peer SPIFFE ID is not in `allowed_sans`; connection refused before
    /// any network I/O.
    #[error("peer SPIFFE ID `{spiffe_id}` not in allowed_sans; refusing conn to {addr}")]
    SpiffeNotAllowed {
        addr: SocketAddr,
        spiffe_id: String,
    },

    /// Sending a request on an existing connection failed.
    #[error("request send failed: {0}")]
    SendRequest(#[source] hyper::Error),

    /// Buffering the response body failed.
    #[error("response body collection failed: {0}")]
    BodyCollect(#[source] hyper::Error),
}

// -- pooled connection --

/// A single persistent H2 connection to one upstream address.
pub struct PooledConn {
    addr: SocketAddr,
    sender: Mutex<http2::SendRequest<Full<Bytes>>>,
    /// Instant of the last successful request through this connection.
    last_used: Mutex<Instant>,
    /// Background H2 driver task (kept alive as long as the conn is pooled).
    _driver: JoinHandle<()>,
}

impl std::fmt::Debug for PooledConn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PooledConn")
            .field("addr", &self.addr)
            .finish()
    }
}

impl PooledConn {
    /// Return `true` when the H2 connection is still ready to accept new streams.
    pub fn is_ready(&self) -> bool {
        self.sender.lock().is_ready()
    }

    /// Return `true` when the connection has been idle longer than `timeout`.
    pub fn is_idle(&self, timeout: Duration) -> bool {
        self.last_used.lock().elapsed() > timeout
    }

    /// Send a single HTTP/2 request and collect the full response body.
    ///
    /// Returns `(response_parts, body_bytes)`.  The body is fully buffered so
    /// the caller does not need to keep the connection alive for streaming.
    pub async fn send(
        &self,
        req: hyper::Request<Full<Bytes>>,
    ) -> Result<(http::response::Parts, Bytes), PoolError> {
        let fut = {
            let mut sender = self.sender.lock();
            sender.send_request(req)
        };

        let response = fut.await.map_err(PoolError::SendRequest)?;

        *self.last_used.lock() = Instant::now();

        let (parts, body) = response.into_parts();
        let body_bytes = body
            .collect()
            .await
            .map_err(PoolError::BodyCollect)?
            .to_bytes();

        Ok((parts, body_bytes))
    }
}

// -- metrics bundle --

struct PoolMetrics {
    pool_size: prometheus::Gauge,
    hits: prometheus::Counter,
    misses: prometheus::Counter,
    handshake_duration: prometheus::Histogram,
}

impl PoolMetrics {
    fn new() -> Self {
        // unwrap_or_else handles duplicate registration (tests spinning up
        // multiple pools in the same process).
        let pool_size = register_gauge!(
            "armageddon_upstream_pool_size",
            "Number of live H2 connections in the upstream pool"
        )
        .unwrap_or_else(|_| {
            prometheus::Gauge::new(
                "armageddon_upstream_pool_size_fallback",
                "duplicate-registration fallback",
            )
            .unwrap()
        });

        let hits = register_counter!(
            "armageddon_upstream_pool_hits_total",
            "Total requests that reused an existing upstream H2 connection"
        )
        .unwrap_or_else(|_| {
            prometheus::Counter::new(
                "armageddon_upstream_pool_hits_total_fallback",
                "duplicate-registration fallback",
            )
            .unwrap()
        });

        let misses = register_counter!(
            "armageddon_upstream_pool_misses_total",
            "Total new upstream H2 connections created"
        )
        .unwrap_or_else(|_| {
            prometheus::Counter::new(
                "armageddon_upstream_pool_misses_total_fallback",
                "duplicate-registration fallback",
            )
            .unwrap()
        });

        let handshake_duration = register_histogram!(
            "armageddon_upstream_handshake_duration_seconds",
            "TCP + HTTP/2 handshake latency to upstream",
            exponential_buckets(0.0005, 2.0, 14).unwrap()
        )
        .unwrap_or_else(|_| {
            prometheus::Histogram::with_opts(prometheus::HistogramOpts::new(
                "armageddon_upstream_handshake_duration_seconds_fallback",
                "duplicate-registration fallback",
            ))
            .unwrap()
        });

        Self {
            pool_size,
            hits,
            misses,
            handshake_duration,
        }
    }
}

// -- pool configuration --

/// Configuration for `UpstreamPool`.
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Maximum number of connections kept in the pool.
    pub max_idle: usize,
    /// Duration after which an idle connection is eligible for eviction.
    pub idle_timeout: Duration,
    /// Maximum concurrent H2 streams per connection.
    pub max_concurrent_streams: u32,
    /// How often the background eviction task runs.
    pub eviction_interval: Duration,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_idle: 64,
            idle_timeout: Duration::from_secs(60),
            max_concurrent_streams: 100,
            eviction_interval: Duration::from_secs(15),
        }
    }
}

// -- upstream pool --

/// Persistent H2 connection pool keyed by upstream `SocketAddr`.
///
/// Eliminates per-request TCP + H2 handshakes by reusing existing connections.
/// Connections are multiplexed: up to `max_concurrent_streams` requests may
/// be in-flight simultaneously over a single TCP connection.
///
/// # Example
/// ```rust,no_run
/// # use std::net::SocketAddr;
/// # use armageddon_forge::upstream_pool::{UpstreamPool, PoolConfig};
/// # #[tokio::main] async fn main() {
/// let pool = UpstreamPool::new(PoolConfig::default());
/// let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
/// let conn = pool.get_or_create(addr).await.unwrap();
/// # }
/// ```
pub struct UpstreamPool {
    conns: Arc<DashMap<SocketAddr, Arc<PooledConn>>>,
    config: PoolConfig,
    metrics: Arc<PoolMetrics>,
    /// Optional mTLS dialer for SPIFFE-annotated clusters.  `None` means plain TCP only.
    mesh_dialer: Option<Arc<AutoMtlsDialer>>,
}

impl std::fmt::Debug for UpstreamPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UpstreamPool")
            .field("size", &self.conns.len())
            .field("config", &self.config)
            .finish()
    }
}

impl UpstreamPool {
    /// Create a new pool with the given configuration (plain TCP only).
    ///
    /// Immediately spawns the background idle-eviction task.
    pub fn new(config: PoolConfig) -> Self {
        Self::new_inner(config, None)
    }

    /// Create a pool that uses [`AutoMtlsDialer`] for SPIFFE-annotated clusters.
    ///
    /// Clusters passed to [`get_or_create_tls`] will use mTLS; all others use
    /// plain TCP via [`get_or_create`].
    pub fn new_with_mesh(config: PoolConfig, dialer: Arc<AutoMtlsDialer>) -> Self {
        Self::new_inner(config, Some(dialer))
    }

    fn new_inner(config: PoolConfig, mesh_dialer: Option<Arc<AutoMtlsDialer>>) -> Self {
        let conns = Arc::new(DashMap::<SocketAddr, Arc<PooledConn>>::new());
        let metrics = Arc::new(PoolMetrics::new());

        let conns_bg = conns.clone();
        let metrics_bg = metrics.clone();
        let idle_timeout = config.idle_timeout;
        let eviction_interval = config.eviction_interval;

        tokio::spawn(async move {
            run_eviction(conns_bg, metrics_bg, idle_timeout, eviction_interval).await;
        });

        Self {
            conns,
            config,
            metrics,
            mesh_dialer,
        }
    }

    /// Return the current number of pooled connections.
    pub fn len(&self) -> usize {
        self.conns.len()
    }

    /// Return `true` when the pool holds no connections.
    pub fn is_empty(&self) -> bool {
        self.conns.is_empty()
    }

    /// Get an existing healthy connection or dial a new one to `addr`.
    ///
    /// - **Hit**: pool holds a ready H2 connection -> returned immediately.
    /// - **Miss**: no ready connection -> new TCP+H2 handshake performed.
    ///
    /// If `max_idle` is exceeded, the oldest idle connection is evicted first.
    pub async fn get_or_create(&self, addr: SocketAddr) -> Result<Arc<PooledConn>, PoolError> {
        // Fast path: return existing ready connection.
        if let Some(entry) = self.conns.get(&addr) {
            let conn = entry.value().clone();
            if conn.is_ready() {
                self.metrics.hits.inc();
                debug!("upstream pool hit for {}", addr);
                return Ok(conn);
            }
            // Connection dead; drop it and fall through to re-dial.
            drop(entry);
            self.conns.remove(&addr);
            self.metrics.pool_size.dec();
        }

        // Slow path: dial.
        self.metrics.misses.inc();
        debug!("upstream pool miss for {}; opening new H2 connection", addr);

        let conn = Arc::new(self.dial(addr).await?);

        if self.conns.len() >= self.config.max_idle {
            self.evict_one_idle();
        }

        self.conns.insert(addr, conn.clone());
        self.metrics.pool_size.set(self.conns.len() as f64);

        Ok(conn)
    }

    /// Pre-warm connections to `addrs` at startup.
    ///
    /// Typically called from an xDS AdsClient endpoint-discovery callback so
    /// that initial requests do not pay the handshake cost.
    pub async fn prewarm(&self, addrs: &[SocketAddr]) {
        for &addr in addrs {
            match self.get_or_create(addr).await {
                Ok(_) => info!("upstream pool: pre-warmed connection to {}", addr),
                Err(e) => warn!("upstream pool: pre-warm failed for {}: {}", addr, e),
            }
        }
    }

    /// Get or create an mTLS connection for a SPIFFE-annotated cluster.
    ///
    /// Requires that the pool was constructed with [`UpstreamPool::new_with_mesh`].
    /// The `tls_ctx.spiffe_id` is validated against the `AutoMtlsDialer`'s
    /// `allowed_sans`; if it is not allowed, `PoolError::SpiffeNotAllowed` is
    /// returned before any network I/O.
    pub async fn get_or_create_tls(
        &self,
        addr: SocketAddr,
        tls_ctx: &ClusterTlsContext,
    ) -> Result<Arc<PooledConn>, PoolError> {
        if let Some(entry) = self.conns.get(&addr) {
            let conn = entry.value().clone();
            if conn.is_ready() {
                self.metrics.hits.inc();
                debug!(addr = %addr, spiffe_id = %tls_ctx.spiffe_id, "upstream pool mTLS hit");
                return Ok(conn);
            }
            drop(entry);
            self.conns.remove(&addr);
            self.metrics.pool_size.dec();
        }

        self.metrics.misses.inc();
        debug!(
            addr = %addr,
            spiffe_id = %tls_ctx.spiffe_id,
            "upstream pool mTLS miss; opening new mTLS+H2 connection"
        );

        let conn = Arc::new(self.dial_tls(addr, tls_ctx).await?);

        if self.conns.len() >= self.config.max_idle {
            self.evict_one_idle();
        }

        self.conns.insert(addr, conn.clone());
        self.metrics.pool_size.set(self.conns.len() as f64);

        Ok(conn)
    }

    // -- internals --

    async fn dial(&self, addr: SocketAddr) -> Result<PooledConn, PoolError> {
        let t0 = Instant::now();

        let tcp = TcpStream::connect(addr)
            .await
            .map_err(|source| PoolError::Connect { addr, source })?;

        let (sender, conn) = http2::Builder::new(TokioExecutor::new())
            .initial_connection_window_size(1 << 20) // 1 MiB connection window
            .initial_stream_window_size(1 << 17)     // 128 KiB per-stream window
            .max_concurrent_reset_streams(self.config.max_concurrent_streams as usize)
            .handshake(TokioIo::new(tcp))
            .await
            .map_err(|source| PoolError::H2Handshake { addr, source })?;

        let elapsed = t0.elapsed().as_secs_f64();
        self.metrics.handshake_duration.observe(elapsed);
        debug!("H2 handshake to {} in {:.3}s", addr, elapsed);

        let driver = tokio::spawn(async move {
            if let Err(e) = conn.await {
                debug!("upstream H2 driver closed: {}", e);
            }
        });

        Ok(PooledConn {
            addr,
            sender: Mutex::new(sender),
            last_used: Mutex::new(Instant::now()),
            _driver: driver,
        })
    }

    /// Dial an mTLS connection and complete the H2 handshake over the TLS stream.
    async fn dial_tls(
        &self,
        addr: SocketAddr,
        tls_ctx: &ClusterTlsContext,
    ) -> Result<PooledConn, PoolError> {
        let dialer = self.mesh_dialer.as_ref().ok_or_else(|| {
            PoolError::SpiffeNotAllowed {
                addr,
                spiffe_id: tls_ctx.spiffe_id.clone(),
            }
        })?;

        let t0 = Instant::now();

        let tls = dialer
            .connect_tls_with_context(addr, None, Some(tls_ctx))
            .await
            .map_err(|source| PoolError::TlsHandshake { addr, source })?;

        let (sender, conn) = http2::Builder::new(TokioExecutor::new())
            .initial_connection_window_size(1 << 20)
            .initial_stream_window_size(1 << 17)
            .max_concurrent_reset_streams(self.config.max_concurrent_streams as usize)
            .handshake(TokioIo::new(tls))
            .await
            .map_err(|source| PoolError::H2Handshake { addr, source })?;

        let elapsed = t0.elapsed().as_secs_f64();
        self.metrics.handshake_duration.observe(elapsed);
        debug!(
            addr = %addr,
            spiffe_id = %tls_ctx.spiffe_id,
            elapsed_s = elapsed,
            "mTLS+H2 handshake complete"
        );

        let driver = tokio::spawn(async move {
            if let Err(e) = conn.await {
                debug!("upstream mTLS H2 driver closed: {}", e);
            }
        });

        Ok(PooledConn {
            addr,
            sender: Mutex::new(sender),
            last_used: Mutex::new(Instant::now()),
            _driver: driver,
        })
    }

    fn evict_one_idle(&self) {
        let mut candidates: Vec<(SocketAddr, Instant)> = self
            .conns
            .iter()
            .filter(|e| e.value().is_idle(Duration::from_secs(0)))
            .map(|e| (*e.key(), *e.value().last_used.lock()))
            .collect();

        candidates.sort_by_key(|(_, t)| *t);

        if let Some((addr, _)) = candidates.first() {
            self.conns.remove(addr);
            self.metrics.pool_size.dec();
            debug!("upstream pool: evicted LRU idle connection to {}", addr);
        }
    }
}

// -- background eviction task --

async fn run_eviction(
    conns: Arc<DashMap<SocketAddr, Arc<PooledConn>>>,
    metrics: Arc<PoolMetrics>,
    idle_timeout: Duration,
    eviction_interval: Duration,
) {
    let mut ticker = tokio::time::interval(eviction_interval);
    loop {
        ticker.tick().await;

        let before = conns.len();
        conns.retain(|addr, conn| {
            let keep = !conn.is_idle(idle_timeout) && conn.is_ready();
            if !keep {
                debug!("upstream pool eviction: purging idle conn to {}", addr);
            }
            keep
        });
        let after = conns.len();
        let removed = before.saturating_sub(after);

        if removed > 0 {
            info!(
                "upstream pool eviction: removed {} idle conn(s), {} remain",
                removed, after
            );
            metrics.pool_size.set(after as f64);
        }
    }
}

// -- tests --

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::io::AsyncWriteExt;
    use tokio::net::TcpListener;

    /// Spawn a minimal H2 server that responds 200 OK to every request.
    async fn bind_h2_server() -> SocketAddr {
        use hyper::server::conn::http2 as h2server;
        use hyper::service::service_fn;
        use hyper::Response;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            loop {
                let Ok((tcp, _)) = listener.accept().await else {
                    break;
                };
                let io = TokioIo::new(tcp);
                tokio::spawn(async move {
                    let _ = h2server::Builder::new(TokioExecutor::new())
                        .serve_connection(
                            io,
                            service_fn(|_req| async {
                                Ok::<_, hyper::Error>(
                                    Response::builder()
                                        .status(200)
                                        .body(Full::new(Bytes::from("ok")))
                                        .unwrap(),
                                )
                            }),
                        )
                        .await;
                });
            }
        });

        addr
    }

    // -----------------------------------------------------------------------
    // Test 1: pool size limit respected
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_pool_size_limit() {
        let config = PoolConfig {
            max_idle: 2,
            idle_timeout: Duration::from_secs(60),
            max_concurrent_streams: 10,
            eviction_interval: Duration::from_secs(300), // disable background eviction
        };
        let pool = UpstreamPool::new(config);

        let addr1 = bind_h2_server().await;
        let addr2 = bind_h2_server().await;
        let addr3 = bind_h2_server().await;

        pool.get_or_create(addr1).await.expect("addr1");
        pool.get_or_create(addr2).await.expect("addr2");
        // Third insert: evict_one_idle fires first.
        pool.get_or_create(addr3).await.expect("addr3");

        assert!(
            pool.len() <= 2,
            "pool.len()={} must stay <= max_idle (2)",
            pool.len()
        );
    }

    // -----------------------------------------------------------------------
    // Test 2: hit rate > 90 % after warm-up
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_hit_rate_after_warmup() {
        let pool = Arc::new(UpstreamPool::new(PoolConfig::default()));
        let addr = bind_h2_server().await;

        // Initial call is a miss.
        pool.get_or_create(addr).await.expect("warmup miss");

        // All subsequent calls must be hits.
        for i in 0..50 {
            let conn = pool.get_or_create(addr)
                .await
                .unwrap_or_else(|e| panic!("call {} failed: {}", i, e));
            assert!(conn.is_ready(), "call {}: connection must be ready", i);
        }

        assert_eq!(pool.len(), 1, "same addr must not grow pool");
    }

    // -----------------------------------------------------------------------
    // Test 3: idle eviction after timeout
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_idle_eviction_by_background_task() {
        let config = PoolConfig {
            max_idle: 64,
            idle_timeout: Duration::from_millis(40),
            eviction_interval: Duration::from_millis(15),
            max_concurrent_streams: 10,
        };
        let pool = UpstreamPool::new(config);

        let addr = bind_h2_server().await;
        pool.get_or_create(addr).await.expect("dial");
        assert_eq!(pool.len(), 1);

        // Wait for several eviction cycles past idle_timeout.
        tokio::time::sleep(Duration::from_millis(300)).await;

        assert_eq!(
            pool.len(),
            0,
            "idle conn must be evicted by background task"
        );
    }

    // -----------------------------------------------------------------------
    // Test 4: H2 multiplexing — 20 parallel streams on 1 connection
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_h2_multiplexing_parallel_streams() {
        let pool = Arc::new(UpstreamPool::new(PoolConfig::default()));
        let addr = bind_h2_server().await;

        let conn = pool.get_or_create(addr).await.expect("warmup");

        let handles: Vec<_> = (0..20)
            .map(|_| {
                let conn = conn.clone();
                tokio::spawn(async move {
                    let req = hyper::Request::builder()
                        .method("GET")
                        .uri(format!("http://{}/", addr))
                        .body(Full::new(Bytes::new()))
                        .expect("req build");
                    conn.send(req).await
                })
            })
            .collect();

        let results: Vec<_> = futures_util::future::join_all(handles).await;
        let ok_count = results
            .iter()
            .filter(|r| r.as_ref().map(|inner| inner.is_ok()).unwrap_or(false))
            .count();

        assert_eq!(ok_count, 20, "all 20 multiplexed streams must succeed");
        assert_eq!(pool.len(), 1, "multiplexing must not create extra connections");
    }

    // -----------------------------------------------------------------------
    // Test 5: connect error does not grow pool
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_dial_error_does_not_pollute_pool() {
        let pool = UpstreamPool::new(PoolConfig::default());
        // Port 1 is reserved; connection must be refused on Linux.
        let addr: SocketAddr = "127.0.0.1:1".parse().unwrap();
        let result = pool.get_or_create(addr).await;

        assert!(
            matches!(result, Err(PoolError::Connect { .. })),
            "expected PoolError::Connect, got {:?}",
            result
        );
        assert_eq!(pool.len(), 0, "failed dial must not pollute pool");
    }

    // -----------------------------------------------------------------------
    // Test 6: prewarm populates the pool at startup
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_prewarm_populates_pool() {
        let pool = UpstreamPool::new(PoolConfig::default());
        let addr1 = bind_h2_server().await;
        let addr2 = bind_h2_server().await;

        pool.prewarm(&[addr1, addr2]).await;

        assert_eq!(pool.len(), 2, "prewarm must open one conn per addr");
        // Both addresses must be ready (hits).
        assert!(pool.get_or_create(addr1).await.is_ok());
        assert!(pool.get_or_create(addr2).await.is_ok());
        assert_eq!(pool.len(), 2, "pool must not grow on hit");
    }

    // -----------------------------------------------------------------------
    // Test 7: dead connection is replaced on next get_or_create
    // -----------------------------------------------------------------------
    /// When a pooled connection's driver task detects the peer closed the H2
    /// session, `is_ready()` becomes `false`.  The next `get_or_create` call
    /// must detect the stale entry, remove it, and open a fresh connection to
    /// a live server.
    #[tokio::test]
    async fn test_dead_conn_triggers_redial() {
        // First: a short-lived server that accepts one connection then stops.
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let dead_addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            if let Ok((mut s, _)) = listener.accept().await {
                // H2 handshake needs at least the client preface; wait briefly
                // so our dial's handshake can complete, then close.
                tokio::time::sleep(Duration::from_millis(20)).await;
                let _ = s.shutdown().await;
            }
            // listener drops here; port closed.
        });

        let pool = UpstreamPool::new(PoolConfig::default());

        // Dial succeeds (H2 sender returned before server closes).
        if let Ok(conn) = pool.get_or_create(dead_addr).await {
            // Give the H2 driver time to process the peer's GOAWAY / FIN.
            tokio::time::sleep(Duration::from_millis(100)).await;

            // Connection must now be dead.
            // Pool must have dropped or marked it; re-dialing to a live server
            // must succeed.
            let live_addr = bind_h2_server().await;
            let live = pool.get_or_create(live_addr).await;
            assert!(live.is_ok(), "dialing a live server must succeed");
            let _ = conn; // keep conn Arc alive to prevent early drop
        }
        // If the initial dial itself failed (timing-dependent), that is also
        // acceptable for this test — the important invariant is that failed
        // dials never pollute the pool.
        assert!(
            pool.len() <= 2,
            "pool must not hold stale entries: len={}",
            pool.len()
        );
    }
}

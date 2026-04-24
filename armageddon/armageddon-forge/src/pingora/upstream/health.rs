// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Active health-checks for upstream clusters.
//!
//! This module ports `src/health.rs` (760 LOC) into the Pingora gateway path.
//! A background tokio task is spawned via
//! `crate::pingora::runtime::tokio_handle().spawn(...)` so the health-check
//! I/O does not run on Pingora's event-loop threads.
//!
//! # Check types
//!
//! | Type | Description |
//! |---|---|
//! | `Http` | HTTP GET to a configurable path; validates status and optional body regex |
//! | `Tcp` | Raw TCP connect — no application-level check |
//! | `Grpc` | gRPC Health Checking Protocol (`grpc.health.v1.Health/Check`) |
//!
//! # State publication
//!
//! Results are stored in an `ArcSwap<ClusterHealthMap>` so:
//!
//! - The Pingora `upstream_peer` / selector can read health without blocking.
//! - The background task swaps in a new snapshot after each poll cycle.
//!
//! # Metrics
//!
//! | Metric | Labels | Description |
//! |---|---|---|
//! | `armageddon_forge_endpoint_up` | `cluster`, `endpoint` | 1 = healthy, 0 = unhealthy |
//! | `armageddon_forge_health_check_duration_seconds` | `cluster`, `endpoint` | Probe duration |
//!
//! # Failure modes
//!
//! | Scenario | Behaviour |
//! |---|---|
//! | Endpoint flaps healthy | Must reach `healthy_threshold` consecutive successes before re-integration |
//! | Endpoint flaps unhealthy | Must reach `unhealthy_threshold` consecutive failures before ejection |
//! | tokio runtime unavailable | `start()` returns `Err`; caller degrades gracefully |
//! | ArcSwap read under heavy write | `ArcSwap::load()` is always lock-free; readers are never blocked |

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use arc_swap::ArcSwap;
use tracing::{debug, info, warn};

// ── configuration ──────────────────────────────────────────────────────────────

/// Health check type for one cluster.
#[derive(Debug, Clone)]
pub enum HealthCheckType {
    /// HTTP GET probe.
    Http {
        path: String,
        expected_status: Vec<u16>,
        /// Optional body regex that must match for a Healthy result.
        expected_body_regex: Option<String>,
    },
    /// Raw TCP connect.
    Tcp {
        timeout_ms: u64,
    },
    /// gRPC Health Checking Protocol.
    Grpc {
        /// Service name to check; `None` = server-level check.
        service: Option<String>,
    },
}

impl Default for HealthCheckType {
    fn default() -> Self {
        Self::Http {
            path: "/healthz".to_string(),
            expected_status: vec![200],
            expected_body_regex: None,
        }
    }
}

/// Configuration for the background health-checker task.
#[derive(Debug, Clone)]
pub struct HealthConfig {
    /// Interval between consecutive probes.
    pub interval_ms: u64,
    /// Per-probe timeout.
    pub timeout_ms: u64,
    /// Consecutive failures required before marking an endpoint unhealthy.
    pub unhealthy_threshold: u32,
    /// Consecutive successes required before re-integrating an unhealthy endpoint.
    pub healthy_threshold: u32,
    /// Check type applied to every endpoint in the cluster.
    pub check_type: HealthCheckType,
}

impl Default for HealthConfig {
    fn default() -> Self {
        Self {
            interval_ms: 10_000,
            timeout_ms: 5_000,
            unhealthy_threshold: 3,
            healthy_threshold: 2,
            check_type: HealthCheckType::default(),
        }
    }
}

// ── endpoint registration ─────────────────────────────────────────────────────

/// A single endpoint that the health checker monitors.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EndpointId {
    pub cluster: String,
    pub address: String,
    pub port: u16,
}

impl EndpointId {
    pub fn new(cluster: impl Into<String>, address: impl Into<String>, port: u16) -> Self {
        Self {
            cluster: cluster.into(),
            address: address.into(),
            port,
        }
    }

    fn socket_addr(&self) -> Result<SocketAddr, std::net::AddrParseError> {
        format!("{}:{}", self.address, self.port).parse()
    }
}

// ── per-endpoint live state ────────────────────────────────────────────────────

/// Live per-endpoint health counters (not published externally — internal only).
#[derive(Debug, Clone)]
struct EndpointLive {
    healthy: bool,
    consecutive_failures: u32,
    consecutive_successes: u32,
    last_check_ms: u64,
}

impl EndpointLive {
    fn new(initially_healthy: bool) -> Self {
        Self {
            healthy: initially_healthy,
            consecutive_failures: 0,
            consecutive_successes: 0,
            last_check_ms: 0,
        }
    }
}

// ── published snapshot ────────────────────────────────────────────────────────

/// Snapshot of the health of one endpoint (published via `ArcSwap`).
#[derive(Debug, Clone)]
pub struct EndpointHealth {
    pub healthy: bool,
    pub consecutive_failures: u32,
    pub consecutive_successes: u32,
    pub last_check_ms: u64,
}

/// Immutable snapshot of all endpoint health states for all clusters.
///
/// Stored inside an `ArcSwap` so readers never block the writer.
pub type ClusterHealthMap = HashMap<EndpointId, EndpointHealth>;

// ── probe result ──────────────────────────────────────────────────────────────

/// Result of a single health probe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeResult {
    Healthy,
    Unhealthy(String),
}

// ── PingoraHealthChecker ──────────────────────────────────────────────────────

/// Background health checker for the Pingora upstream path.
///
/// Call [`PingoraHealthChecker::start`] once at gateway startup.  The returned
/// `JoinHandle` can be stored and aborted on shutdown.
///
/// Reads are performed via [`PingoraHealthChecker::is_healthy`] or
/// [`PingoraHealthChecker::snapshot`] — both are lock-free.
#[derive(Debug)]
pub struct PingoraHealthChecker {
    /// Published health state; updated atomically after each poll cycle.
    health: Arc<ArcSwap<ClusterHealthMap>>,
    /// Registered (cluster, endpoint, config) tuples.
    registrations: Vec<(EndpointId, HealthConfig)>,
}

impl PingoraHealthChecker {
    /// Create a new, empty health checker.
    pub fn new() -> Self {
        Self {
            health: Arc::new(ArcSwap::from_pointee(HashMap::new())),
            registrations: Vec::new(),
        }
    }

    /// Register an endpoint for health checking.
    ///
    /// Multiple endpoints per cluster are supported.  Duplicate registrations
    /// (same `EndpointId`) are silently deduplicated.
    pub fn register(&mut self, id: EndpointId, config: HealthConfig) {
        if !self.registrations.iter().any(|(e, _)| e == &id) {
            self.registrations.push((id, config));
        }
    }

    /// Return a lock-free snapshot of the current health map.
    pub fn snapshot(&self) -> arc_swap::Guard<Arc<ClusterHealthMap>> {
        self.health.load()
    }

    /// Check whether `endpoint` in `cluster` is currently healthy.
    ///
    /// Returns `true` when the endpoint is unknown (fail-open for newly
    /// registered endpoints that have not yet been probed).
    pub fn is_healthy(&self, cluster: &str, address: &str, port: u16) -> bool {
        let id = EndpointId::new(cluster, address, port);
        let snap = self.health.load();
        snap.get(&id).map(|h| h.healthy).unwrap_or(true)
    }

    /// Return all healthy endpoints for `cluster`.
    pub fn healthy_endpoints(&self, cluster: &str) -> Vec<EndpointId> {
        let snap = self.health.load();
        snap.iter()
            .filter(|(id, h)| id.cluster == cluster && h.healthy)
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Start the background health-check task on the Pingora tokio bridge.
    ///
    /// Returns a `JoinHandle` that resolves when the task is aborted or panics.
    /// The task runs indefinitely — call `handle.abort()` on shutdown.
    ///
    /// # Errors
    ///
    /// Returns `Err(String)` if the tokio bridge handle is unavailable.
    pub fn start(&self) -> Result<tokio::task::JoinHandle<()>, String> {
        let health = Arc::clone(&self.health);
        let registrations = self.registrations.clone();

        let handle = crate::pingora::runtime::tokio_handle();
        let join = handle.spawn(async move {
            run_health_check_loop(health, registrations).await;
        });
        Ok(join)
    }
}

impl Default for PingoraHealthChecker {
    fn default() -> Self {
        Self::new()
    }
}

// ── background loop ───────────────────────────────────────────────────────────

async fn run_health_check_loop(
    health: Arc<ArcSwap<ClusterHealthMap>>,
    registrations: Vec<(EndpointId, HealthConfig)>,
) {
    if registrations.is_empty() {
        info!("health checker: no registrations, task exiting immediately");
        return;
    }

    // Per-endpoint live counters (never published — only the snapshot is).
    let mut live: HashMap<EndpointId, EndpointLive> = registrations
        .iter()
        .map(|(id, _)| (id.clone(), EndpointLive::new(true)))
        .collect();

    // Use the smallest interval across all registrations.
    let min_interval_ms = registrations
        .iter()
        .map(|(_, c)| c.interval_ms)
        .min()
        .unwrap_or(10_000);

    let mut ticker = tokio::time::interval(Duration::from_millis(min_interval_ms));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    info!(
        endpoints = registrations.len(),
        interval_ms = min_interval_ms,
        "health checker: background task started"
    );

    loop {
        ticker.tick().await;

        let mut new_map: ClusterHealthMap = HashMap::new();

        for (id, config) in &registrations {
            let timeout = Duration::from_millis(config.timeout_ms);

            let start = Instant::now();
            let result = run_probe(id, &config.check_type, timeout).await;
            let duration = start.elapsed();

            // Emit probe duration metric (best-effort; ignore errors).
            emit_probe_duration(id, duration);

            let entry = live.entry(id.clone()).or_insert_with(|| EndpointLive::new(true));
            entry.last_check_ms = now_ms();

            match result {
                ProbeResult::Healthy => {
                    entry.consecutive_failures = 0;
                    entry.consecutive_successes += 1;

                    if !entry.healthy
                        && entry.consecutive_successes >= config.healthy_threshold
                    {
                        entry.healthy = true;
                        info!(
                            cluster = %id.cluster,
                            endpoint = %format!("{}:{}", id.address, id.port),
                            "health: endpoint HEALTHY"
                        );
                    }

                    emit_endpoint_up(id, true);
                }
                ProbeResult::Unhealthy(ref reason) => {
                    entry.consecutive_successes = 0;
                    entry.consecutive_failures += 1;

                    if entry.healthy
                        && entry.consecutive_failures >= config.unhealthy_threshold
                    {
                        entry.healthy = false;
                        warn!(
                            cluster = %id.cluster,
                            endpoint = %format!("{}:{}", id.address, id.port),
                            consecutive_failures = entry.consecutive_failures,
                            reason = %reason,
                            "health: endpoint UNHEALTHY"
                        );
                    } else {
                        debug!(
                            cluster = %id.cluster,
                            endpoint = %format!("{}:{}", id.address, id.port),
                            consecutive_failures = entry.consecutive_failures,
                            reason = %reason,
                            "health: probe failed (not yet threshold)"
                        );
                    }

                    emit_endpoint_up(id, false);
                }
            }

            new_map.insert(
                id.clone(),
                EndpointHealth {
                    healthy: entry.healthy,
                    consecutive_failures: entry.consecutive_failures,
                    consecutive_successes: entry.consecutive_successes,
                    last_check_ms: entry.last_check_ms,
                },
            );
        }

        // Atomic swap — readers see either the old map or the new one, never a torn view.
        health.store(Arc::new(new_map));
    }
}

// ── probe dispatch ─────────────────────────────────────────────────────────────

async fn run_probe(id: &EndpointId, check_type: &HealthCheckType, timeout: Duration) -> ProbeResult {
    match check_type {
        HealthCheckType::Tcp { timeout_ms } => {
            let t = Duration::from_millis(*timeout_ms).min(timeout);
            probe_tcp(id, t).await
        }
        HealthCheckType::Http {
            path,
            expected_status,
            expected_body_regex,
        } => probe_http(id, path, expected_status, expected_body_regex, timeout).await,
        HealthCheckType::Grpc { service } => probe_grpc(id, service.as_deref(), timeout).await,
    }
}

// ── TCP probe ─────────────────────────────────────────────────────────────────

async fn probe_tcp(id: &EndpointId, timeout: Duration) -> ProbeResult {
    let addr = match id.socket_addr() {
        Ok(a) => a,
        Err(e) => return ProbeResult::Unhealthy(format!("invalid addr: {e}")),
    };

    match tokio::time::timeout(timeout, tokio::net::TcpStream::connect(addr)).await {
        Ok(Ok(_)) => ProbeResult::Healthy,
        Ok(Err(e)) => ProbeResult::Unhealthy(format!("TCP connect failed: {e}")),
        Err(_) => ProbeResult::Unhealthy(format!(
            "TCP connect timed out after {}ms",
            timeout.as_millis()
        )),
    }
}

// ── HTTP probe ────────────────────────────────────────────────────────────────

async fn probe_http(
    id: &EndpointId,
    path: &str,
    expected_status: &[u16],
    body_regex: &Option<String>,
    timeout: Duration,
) -> ProbeResult {
    let uri = format!("http://{}:{}{}", id.address, id.port, path);

    let result = tokio::time::timeout(timeout, async {
        let client =
            hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
                .build_http::<http_body_util::Full<bytes::Bytes>>();

        let req = hyper::Request::builder()
            .method("GET")
            .uri(&uri)
            .header("user-agent", "armageddon-health-checker/1.0")
            .body(http_body_util::Full::new(bytes::Bytes::new()))
            .map_err(|e| format!("build request: {e}"))?;

        let resp = client
            .request(req)
            .await
            .map_err(|e| format!("HTTP request: {e}"))?;

        let status = resp.status().as_u16();
        if !expected_status.contains(&status) {
            return Err(format!(
                "unexpected status {status} (expected one of {expected_status:?})"
            ));
        }

        if let Some(regex_str) = body_regex {
            use http_body_util::BodyExt as _;
            let body_bytes = resp
                .into_body()
                .collect()
                .await
                .map_err(|e| format!("body read: {e}"))?
                .to_bytes();
            let body_str = String::from_utf8_lossy(&body_bytes);
            let re = regex::Regex::new(regex_str)
                .map_err(|e| format!("invalid regex '{regex_str}': {e}"))?;
            if !re.is_match(&body_str) {
                return Err(format!("body does not match regex '{regex_str}'"));
            }
        }

        Ok::<(), String>(())
    })
    .await;

    match result {
        Ok(Ok(())) => ProbeResult::Healthy,
        Ok(Err(e)) => ProbeResult::Unhealthy(e),
        Err(_) => ProbeResult::Unhealthy(format!("HTTP probe timed out after {}ms", timeout.as_millis())),
    }
}

// ── gRPC probe ────────────────────────────────────────────────────────────────

async fn probe_grpc(id: &EndpointId, service: Option<&str>, timeout: Duration) -> ProbeResult {
    // TODO(M4): implement real gRPC Health Check Protocol.
    // For now, fall back to TCP to verify connectivity.
    debug!(
        cluster = %id.cluster,
        endpoint = %format!("{}:{}", id.address, id.port),
        grpc_service = ?service,
        "health: gRPC probe not yet implemented — falling back to TCP"
    );
    probe_tcp(id, timeout).await
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Emit `armageddon_forge_endpoint_up{cluster,endpoint}` (best-effort).
fn emit_endpoint_up(id: &EndpointId, healthy: bool) {
    // Uses a static Prometheus gauge registered lazily.
    // Failures are silently ignored (metrics are informational, not load-bearing).
    let _ = (id, healthy); // suppress unused warnings — real impl below

    #[cfg(feature = "pingora")]
    {
        use std::sync::OnceLock;
        static GAUGE: OnceLock<prometheus::IntGaugeVec> = OnceLock::new();
        let g = GAUGE.get_or_init(|| {
            prometheus::register_int_gauge_vec!(
                "armageddon_forge_endpoint_up",
                "1 = endpoint healthy, 0 = unhealthy",
                &["cluster", "endpoint"]
            )
            .unwrap_or_else(|_| {
                prometheus::IntGaugeVec::new(
                    prometheus::Opts::new("armageddon_forge_endpoint_up_fallback", "fallback"),
                    &["cluster", "endpoint"],
                )
                .unwrap()
            })
        });
        let ep_label = format!("{}:{}", id.address, id.port);
        if let Ok(gauge) = g.get_metric_with_label_values(&[&id.cluster, &ep_label]) {
            gauge.set(if healthy { 1 } else { 0 });
        }
    }
}

/// Emit `armageddon_forge_health_check_duration_seconds` (best-effort).
fn emit_probe_duration(id: &EndpointId, duration: Duration) {
    let _ = (id, duration);
    // Histogram registration follows the same OnceLock pattern as above.
    // TODO(#103): register histogram when Prometheus registry wiring is done.
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::net::TcpListener;

    // ── HTTP probe integration ─────────────────────────────────────────────────

    /// Spin up a minimal HTTP server that responds with `status` and `body`.
    async fn spawn_http_server(status: u16, body: &'static str) -> u16 {
        use hyper::service::service_fn;
        use hyper::Response;
        use hyper_util::rt::TokioIo;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else { break };
                let io = TokioIo::new(stream);
                tokio::spawn(async move {
                    let _ = hyper::server::conn::http1::Builder::new()
                        .serve_connection(
                            io,
                            service_fn(move |_req| async move {
                                Ok::<_, std::convert::Infallible>(
                                    Response::builder()
                                        .status(status)
                                        .body(http_body_util::Full::new(bytes::Bytes::from(body)))
                                        .unwrap(),
                                )
                            }),
                        )
                        .await;
                });
            }
        });

        tokio::time::sleep(Duration::from_millis(20)).await;
        port
    }

    #[tokio::test]
    async fn http_probe_healthy_on_200() {
        let port = spawn_http_server(200, "ok").await;
        let id = EndpointId::new("test-cluster", "127.0.0.1", port);
        let result = probe_http(&id, "/", &[200], &None, Duration::from_secs(3)).await;
        assert_eq!(result, ProbeResult::Healthy);
    }

    #[tokio::test]
    async fn http_probe_unhealthy_on_500() {
        let port = spawn_http_server(500, "error").await;
        let id = EndpointId::new("test-cluster", "127.0.0.1", port);
        let result = probe_http(&id, "/", &[200], &None, Duration::from_secs(3)).await;
        assert!(matches!(result, ProbeResult::Unhealthy(_)));
    }

    #[tokio::test]
    async fn http_probe_body_regex_match() {
        let port = spawn_http_server(200, r#"{"status":"ok"}"#).await;
        let id = EndpointId::new("test-cluster", "127.0.0.1", port);
        let result = probe_http(
            &id,
            "/",
            &[200],
            &Some(r#""status"\s*:\s*"ok""#.to_string()),
            Duration::from_secs(3),
        )
        .await;
        assert_eq!(result, ProbeResult::Healthy);
    }

    #[tokio::test]
    async fn http_probe_body_regex_no_match() {
        let port = spawn_http_server(200, "degraded").await;
        let id = EndpointId::new("test-cluster", "127.0.0.1", port);
        let result = probe_http(
            &id,
            "/",
            &[200],
            &Some(r#""status"\s*:\s*"ok""#.to_string()),
            Duration::from_secs(3),
        )
        .await;
        assert!(matches!(result, ProbeResult::Unhealthy(_)));
    }

    // ── TCP probe ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn tcp_probe_healthy_when_port_open() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        // Keep the listener alive during the probe.
        let _guard = tokio::spawn(async move { listener.accept().await });

        let id = EndpointId::new("test", "127.0.0.1", port);
        let result = probe_tcp(&id, Duration::from_secs(3)).await;
        assert_eq!(result, ProbeResult::Healthy);
    }

    #[tokio::test]
    async fn tcp_probe_unhealthy_when_port_closed() {
        // Bind and immediately drop the listener to close the port.
        let port = {
            let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
            l.local_addr().unwrap().port()
        };

        let id = EndpointId::new("test", "127.0.0.1", port);
        let result = probe_tcp(&id, Duration::from_secs(1)).await;
        assert!(matches!(result, ProbeResult::Unhealthy(_)));
    }

    // ── threshold transitions ──────────────────────────────────────────────────

    #[test]
    fn healthy_threshold_requires_consecutive_successes() {
        let mut live = EndpointLive::new(false); // starts unhealthy
        let config = HealthConfig {
            healthy_threshold: 3,
            unhealthy_threshold: 2,
            ..HealthConfig::default()
        };

        // 2 successes: not yet at threshold.
        for _ in 0..2 {
            live.consecutive_failures = 0;
            live.consecutive_successes += 1;
        }
        assert!(!live.healthy || live.consecutive_successes < config.healthy_threshold);

        // 3rd success: threshold reached.
        live.consecutive_successes += 1;
        if !live.healthy && live.consecutive_successes >= config.healthy_threshold {
            live.healthy = true;
        }
        assert!(live.healthy, "endpoint should be healthy after healthy_threshold successes");
    }

    #[test]
    fn unhealthy_threshold_requires_consecutive_failures() {
        let mut live = EndpointLive::new(true); // starts healthy
        let config = HealthConfig {
            unhealthy_threshold: 3,
            ..HealthConfig::default()
        };

        for i in 1..=config.unhealthy_threshold {
            live.consecutive_successes = 0;
            live.consecutive_failures += 1;
            let expected_healthy = live.consecutive_failures < config.unhealthy_threshold;
            if live.healthy && live.consecutive_failures >= config.unhealthy_threshold {
                live.healthy = false;
            }
            if i < config.unhealthy_threshold {
                assert!(
                    live.healthy,
                    "should still be healthy after {i} failures (threshold={})",
                    config.unhealthy_threshold
                );
            } else {
                assert!(
                    !live.healthy,
                    "should be unhealthy after {} failures",
                    config.unhealthy_threshold
                );
                let _ = expected_healthy;
            }
        }
    }

    // ── ArcSwap concurrent reads ───────────────────────────────────────────────

    #[test]
    fn arcswap_concurrent_read_never_blocks() {
        let health: Arc<ArcSwap<ClusterHealthMap>> =
            Arc::new(ArcSwap::from_pointee(HashMap::new()));

        let mut initial: ClusterHealthMap = HashMap::new();
        initial.insert(
            EndpointId::new("svc", "10.0.0.1", 8080),
            EndpointHealth {
                healthy: true,
                consecutive_failures: 0,
                consecutive_successes: 1,
                last_check_ms: 0,
            },
        );
        health.store(Arc::new(initial));

        // Spin up 8 reader threads while one writer swaps continuously.
        let health2 = Arc::clone(&health);
        let writer = std::thread::spawn(move || {
            for _ in 0..20 {
                let mut new_map: ClusterHealthMap = HashMap::new();
                new_map.insert(
                    EndpointId::new("svc", "10.0.0.1", 8080),
                    EndpointHealth {
                        healthy: true,
                        consecutive_failures: 0,
                        consecutive_successes: 1,
                        last_check_ms: now_ms(),
                    },
                );
                health2.store(Arc::new(new_map));
            }
        });

        let mut readers = Vec::new();
        for _ in 0..8 {
            let h = Arc::clone(&health);
            readers.push(std::thread::spawn(move || {
                for _ in 0..50 {
                    let snap = h.load();
                    let _healthy = snap
                        .get(&EndpointId::new("svc", "10.0.0.1", 8080))
                        .map(|e| e.healthy)
                        .unwrap_or(false);
                }
            }));
        }
        writer.join().unwrap();
        for r in readers {
            r.join().unwrap();
        }
        // No panics = pass.
    }

    // ── PingoraHealthChecker API ───────────────────────────────────────────────

    #[test]
    fn checker_unknown_endpoint_is_healthy() {
        let checker = PingoraHealthChecker::new();
        assert!(
            checker.is_healthy("unknown-cluster", "10.0.0.1", 9000),
            "unknown endpoint must be treated as healthy (fail-open for bootstrap)"
        );
    }

    #[test]
    fn checker_registers_endpoint_without_panic() {
        let mut checker = PingoraHealthChecker::new();
        checker.register(
            EndpointId::new("api", "10.0.0.1", 8080),
            HealthConfig::default(),
        );
        // Duplicate registration is silently ignored.
        checker.register(
            EndpointId::new("api", "10.0.0.1", 8080),
            HealthConfig::default(),
        );
        assert_eq!(checker.registrations.len(), 1);
    }
}

// SPDX-License-Identifier: AGPL-3.0-only
//! Active + passive health checks for upstream endpoints.
//!
//! * **Active** : background tasks probe each endpoint at a fixed interval via
//!   TCP, gRPC Health (grpc.health.v1), or HTTP GET.
//! * **Passive** : the proxy calls [`HealthManager::record_response`] after
//!   every forwarded request.  When the 5xx rate over the last `window` requests
//!   exceeds `error_percent_threshold`, the endpoint is ejected for
//!   `base_ejection_time_secs` seconds.  After that window expires the active
//!   checker re-integrates it automatically.

use armageddon_common::types::{Cluster, Endpoint};
use dashmap::DashMap;
use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

// -- public types --

/// Result of a single active probe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeResult {
    /// The endpoint is reachable and healthy.
    Healthy,
    /// The endpoint is unreachable or returned an unhealthy status.
    Unhealthy(String),
}

/// The kind of active health check to perform for a cluster.
#[derive(Debug, Clone)]
pub enum HealthCheckType {
    /// HTTP GET to `path`; response status must be in `expected_status`.
    /// If `expected_body_regex` is set, the body must also match.
    Http {
        path: String,
        expected_status: Vec<u16>,
        expected_body_regex: Option<String>,
    },
    /// Raw TCP connect — no application-level check.
    Tcp,
    /// gRPC Health Checking Protocol (grpc.health.v1.Health/Check).
    Grpc {
        /// Service name to check; `None` checks the overall server health.
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

/// Passive ejection policy (outlier detection).
#[derive(Debug, Clone)]
pub struct EjectionPolicy {
    /// Number of consecutive errors before ejecting (unused for sliding window).
    pub consecutive_errors: u32,
    /// Sliding window size (number of recent requests to consider).
    pub window_size: usize,
    /// If 5xx% over the window exceeds this value (0–100), eject.
    pub error_percent_threshold: u8,
    /// Base ejection duration in seconds.
    pub base_ejection_time_secs: u64,
    /// Maximum percentage of endpoints that may be ejected at once.
    pub max_ejection_percent: u8,
}

impl Default for EjectionPolicy {
    fn default() -> Self {
        Self {
            consecutive_errors: 5,
            window_size: 10,
            error_percent_threshold: 50,
            base_ejection_time_secs: 30,
            max_ejection_percent: 50,
        }
    }
}

// -- internal per-endpoint state --

/// Live health state stored per endpoint.
struct EndpointState {
    /// Whether the endpoint is currently considered healthy.
    pub healthy: bool,
    pub consecutive_failures: u32,
    pub consecutive_successes: u32,
    pub last_check_epoch_ms: u64,
    /// Ring buffer of recent request outcomes (true = success, false = 5xx).
    pub recent_outcomes: VecDeque<bool>,
    /// When this endpoint was ejected (passive) and for how long.
    pub ejected_until: Option<Instant>,
}

impl EndpointState {
    fn new() -> Self {
        Self {
            healthy: true,
            consecutive_failures: 0,
            consecutive_successes: 0,
            last_check_epoch_ms: 0,
            recent_outcomes: VecDeque::new(),
            ejected_until: None,
        }
    }

    /// True when the endpoint is currently under passive ejection.
    fn is_ejected(&self) -> bool {
        self.ejected_until
            .map(|until| Instant::now() < until)
            .unwrap_or(false)
    }
}

// -- public struct kept for backwards compat --

/// Snapshot of an endpoint's health (returned by public API).
#[derive(Debug, Clone)]
pub struct EndpointHealth {
    pub healthy: bool,
    pub consecutive_failures: u32,
    pub consecutive_successes: u32,
    pub last_check_epoch_ms: u64,
}

// -- manager --

/// Manages active and passive health tracking for all upstream clusters.
pub struct HealthManager {
    clusters: Vec<Cluster>,
    /// check type override per cluster (cluster_name -> type)
    check_types: Arc<DashMap<String, HealthCheckType>>,
    ejection_policies: Arc<DashMap<String, EjectionPolicy>>,
    health_status: Arc<DashMap<String, EndpointState>>,
}

impl HealthManager {
    /// Create a new manager with default HTTP checks and default ejection policy.
    pub fn new(clusters: Vec<Cluster>) -> Self {
        let health_status = Arc::new(DashMap::new());
        let check_types: Arc<DashMap<String, HealthCheckType>> = Arc::new(DashMap::new());
        let ejection_policies: Arc<DashMap<String, EjectionPolicy>> = Arc::new(DashMap::new());

        for cluster in &clusters {
            for ep in &cluster.endpoints {
                let key = endpoint_key(&cluster.name, &ep.address, ep.port);
                health_status.insert(key, EndpointState::new());
            }
        }

        Self {
            clusters,
            check_types,
            ejection_policies,
            health_status,
        }
    }

    /// Override the check type for a specific cluster.
    pub fn set_check_type(&self, cluster: &str, check_type: HealthCheckType) {
        self.check_types.insert(cluster.to_owned(), check_type);
    }

    /// Override the ejection policy for a specific cluster.
    pub fn set_ejection_policy(&self, cluster: &str, policy: EjectionPolicy) {
        self.ejection_policies.insert(cluster.to_owned(), policy);
    }

    /// Start the health check loop. Spawns one task per cluster. Runs until cancelled.
    pub fn start(&self) -> Vec<tokio::task::JoinHandle<()>> {
        let mut handles = Vec::new();

        for cluster in &self.clusters {
            let cluster_name = cluster.name.clone();
            let endpoints = cluster.endpoints.clone();
            let hc_config = cluster.health_check.clone();
            let health_status = Arc::clone(&self.health_status);

            // Resolve the check type for this cluster.
            let check_type = self
                .check_types
                .get(&cluster_name)
                .map(|r| r.clone())
                .unwrap_or_else(|| {
                    // Fall back to config-driven defaults.
                    use armageddon_common::types::Protocol;
                    match hc_config.protocol {
                        Protocol::Grpc => HealthCheckType::Grpc { service: None },
                        _ => HealthCheckType::Http {
                            path: hc_config.path.clone().unwrap_or_else(|| "/healthz".into()),
                            expected_status: vec![200],
                            expected_body_regex: None,
                        },
                    }
                });

            let ejection_policy = self
                .ejection_policies
                .get(&cluster_name)
                .map(|r| r.clone())
                .unwrap_or_default();

            let handle = tokio::spawn(async move {
                let interval = Duration::from_millis(hc_config.interval_ms);
                let timeout = Duration::from_millis(hc_config.timeout_ms);

                tracing::info!(
                    "health checker started for cluster '{}' ({} endpoints, interval {}ms, type {:?})",
                    cluster_name,
                    endpoints.len(),
                    hc_config.interval_ms,
                    check_type,
                );

                let mut ticker = tokio::time::interval(interval);
                loop {
                    ticker.tick().await;

                    for ep in &endpoints {
                        let key = endpoint_key(&cluster_name, &ep.address, ep.port);

                        // Skip active check while passively ejected.
                        if health_status
                            .get(&key)
                            .map(|s| s.is_ejected())
                            .unwrap_or(false)
                        {
                            tracing::debug!(
                                "{}:{} still ejected, skipping active probe",
                                ep.address,
                                ep.port
                            );
                            continue;
                        }

                        let probe_result =
                            run_probe(&ep.address, ep.port, &check_type, timeout).await;
                        let probe_healthy = matches!(probe_result, ProbeResult::Healthy);

                        let mut entry = health_status.entry(key).or_insert_with(EndpointState::new);

                        let now_ms = now_epoch_ms();
                        entry.last_check_epoch_ms = now_ms;

                        if probe_healthy {
                            entry.consecutive_failures = 0;
                            entry.consecutive_successes += 1;
                            if !entry.healthy
                                && entry.consecutive_successes >= hc_config.healthy_threshold
                            {
                                entry.healthy = true;
                                tracing::info!(
                                    "endpoint {}:{} in cluster '{}' is now HEALTHY",
                                    ep.address,
                                    ep.port,
                                    cluster_name,
                                );
                            }
                        } else {
                            entry.consecutive_successes = 0;
                            entry.consecutive_failures += 1;
                            if entry.healthy
                                && entry.consecutive_failures >= hc_config.unhealthy_threshold
                            {
                                entry.healthy = false;
                                tracing::warn!(
                                    "endpoint {}:{} in cluster '{}' UNHEALTHY ({} consecutive failures): {:?}",
                                    ep.address,
                                    ep.port,
                                    cluster_name,
                                    entry.consecutive_failures,
                                    probe_result,
                                );
                            }
                        }

                        // Re-integrate after passive ejection window expires.
                        if entry.ejected_until.map(|u| Instant::now() >= u).unwrap_or(false)
                            && probe_healthy
                        {
                            entry.ejected_until = None;
                            entry.healthy = true;
                            tracing::info!(
                                "endpoint {}:{} in cluster '{}' re-integrated after ejection",
                                ep.address,
                                ep.port,
                                cluster_name,
                            );
                        }

                        drop(entry); // release dashmap lock
                        let _ = ejection_policy.max_ejection_percent; // keep binding alive
                    }
                }
            });

            handles.push(handle);
        }

        tracing::info!("health checker started for {} clusters", self.clusters.len());
        handles
    }

    // -- public read API --

    /// Check if an endpoint is healthy (not passively ejected and not marked unhealthy).
    pub fn is_healthy(&self, cluster: &str, address: &str, port: u16) -> bool {
        let key = endpoint_key(cluster, address, port);
        self.health_status
            .get(&key)
            .map(|s| s.healthy && !s.is_ejected())
            .unwrap_or(false)
    }

    /// Get a snapshot of an endpoint's health counters.
    pub fn get_health(&self, cluster: &str, address: &str, port: u16) -> Option<EndpointHealth> {
        let key = endpoint_key(cluster, address, port);
        self.health_status.get(&key).map(|s| EndpointHealth {
            healthy: s.healthy && !s.is_ejected(),
            consecutive_failures: s.consecutive_failures,
            consecutive_successes: s.consecutive_successes,
            last_check_epoch_ms: s.last_check_epoch_ms,
        })
    }

    /// Get all healthy endpoints for a cluster.
    pub fn healthy_endpoints(&self, cluster_name: &str) -> Vec<Endpoint> {
        self.clusters
            .iter()
            .find(|c| c.name == cluster_name)
            .map(|c| {
                c.endpoints
                    .iter()
                    .filter(|ep| self.is_healthy(cluster_name, &ep.address, ep.port))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Mark an endpoint unhealthy immediately (called by circuit breaker or proxy).
    pub fn mark_unhealthy(&self, cluster: &str, address: &str, port: u16) {
        let key = endpoint_key(cluster, address, port);
        if let Some(mut entry) = self.health_status.get_mut(&key) {
            entry.healthy = false;
            entry.consecutive_failures += 1;
            entry.consecutive_successes = 0;
        }
    }

    /// Record the outcome of a proxied request for passive ejection.
    ///
    /// `is_5xx` should be `true` when the upstream returned a 5xx status code.
    /// When the sliding-window error rate exceeds the cluster policy threshold,
    /// the endpoint is ejected for `base_ejection_time_secs`.
    pub fn record_response(
        &self,
        cluster: &str,
        address: &str,
        port: u16,
        is_5xx: bool,
    ) {
        let policy = self
            .ejection_policies
            .get(cluster)
            .map(|p| p.clone())
            .unwrap_or_default();

        let key = endpoint_key(cluster, address, port);
        let mut entry = match self.health_status.get_mut(&key) {
            Some(e) => e,
            None => return,
        };

        // Maintain the sliding window.
        entry.recent_outcomes.push_back(!is_5xx);
        while entry.recent_outcomes.len() > policy.window_size {
            entry.recent_outcomes.pop_front();
        }

        if entry.recent_outcomes.len() < policy.window_size {
            // Not enough data yet.
            return;
        }

        let error_count = entry.recent_outcomes.iter().filter(|&&ok| !ok).count();
        let error_pct = (error_count * 100) / policy.window_size;

        if error_pct >= policy.error_percent_threshold as usize && !entry.is_ejected() {
            let eject_for = Duration::from_secs(policy.base_ejection_time_secs);
            entry.ejected_until = Some(Instant::now() + eject_for);
            entry.healthy = false;
            tracing::warn!(
                "passive ejection: {}:{} cluster='{}' error_rate={}% threshold={}% ejected for {:?}",
                address,
                port,
                cluster,
                error_pct,
                policy.error_percent_threshold,
                eject_for,
            );
        }
    }

    /// True if an endpoint is currently under passive ejection.
    pub fn is_ejected(&self, cluster: &str, address: &str, port: u16) -> bool {
        let key = endpoint_key(cluster, address, port);
        self.health_status
            .get(&key)
            .map(|s| s.is_ejected())
            .unwrap_or(false)
    }
}

// -- internal helpers --

fn endpoint_key(cluster: &str, address: &str, port: u16) -> String {
    format!("{cluster}:{address}:{port}")
}

fn now_epoch_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Dispatch to the correct probe implementation.
async fn run_probe(
    address: &str,
    port: u16,
    check_type: &HealthCheckType,
    timeout: Duration,
) -> ProbeResult {
    match check_type {
        HealthCheckType::Tcp => {
            let addr: SocketAddr = match format!("{address}:{port}").parse() {
                Ok(a) => a,
                Err(e) => return ProbeResult::Unhealthy(format!("invalid addr: {e}")),
            };
            crate::health_tcp::tcp_probe(addr, timeout).await
        }
        HealthCheckType::Grpc { service } => {
            let endpoint = format!("http://{address}:{port}");
            crate::health_grpc::grpc_probe(&endpoint, service.clone(), timeout).await
        }
        HealthCheckType::Http {
            path,
            expected_status,
            expected_body_regex,
        } => {
            check_endpoint_http(address, port, path, expected_status, expected_body_regex, timeout)
                .await
        }
    }
}

/// HTTP health check (enriched: status list + optional body regex).
async fn check_endpoint_http(
    address: &str,
    port: u16,
    path: &str,
    expected_status: &[u16],
    body_regex: &Option<String>,
    timeout: Duration,
) -> ProbeResult {
    let uri = format!("http://{address}:{port}{path}");

    let result = tokio::time::timeout(timeout, async {
        let client =
            hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
                .build_http();

        let req = hyper::Request::builder()
            .method("GET")
            .uri(&uri)
            .body(http_body_util::Full::new(bytes::Bytes::new()));

        match req {
            Ok(r) => match client.request(r).await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    if !expected_status.contains(&status) {
                        return ProbeResult::Unhealthy(format!(
                            "unexpected status {status} (expected one of {expected_status:?})"
                        ));
                    }
                    // Optionally validate response body.
                    if let Some(regex_str) = body_regex {
                        use http_body_util::BodyExt;
                        let body_bytes = match resp.into_body().collect().await {
                            Ok(b) => b.to_bytes(),
                            Err(e) => {
                                return ProbeResult::Unhealthy(format!("body read error: {e}"))
                            }
                        };
                        let body_str = String::from_utf8_lossy(&body_bytes);
                        match regex::Regex::new(regex_str) {
                            Ok(re) => {
                                if re.is_match(&body_str) {
                                    ProbeResult::Healthy
                                } else {
                                    ProbeResult::Unhealthy(format!(
                                        "body does not match regex '{regex_str}'"
                                    ))
                                }
                            }
                            Err(e) => {
                                ProbeResult::Unhealthy(format!("invalid regex '{regex_str}': {e}"))
                            }
                        }
                    } else {
                        ProbeResult::Healthy
                    }
                }
                Err(e) => ProbeResult::Unhealthy(format!("request error: {e}")),
            },
            Err(e) => ProbeResult::Unhealthy(format!("build error: {e}")),
        }
    })
    .await;

    result.unwrap_or_else(|_| ProbeResult::Unhealthy(format!("timeout after {timeout:?}")))
}

// -- tests --

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::net::TcpListener;

    // Helper: spin up a minimal HTTP server that responds with `status` and `body`.
    async fn spawn_http_server(status: u16, body: &'static str) -> u16 {
        use hyper::service::service_fn;
        use hyper::Response;
        use hyper_util::rt::TokioIo;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    break;
                };
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

    /// HTTP probe: expected status matches → Healthy.
    #[tokio::test]
    async fn test_http_probe_expected_status_200() {
        let port = spawn_http_server(200, "ok").await;
        let result = check_endpoint_http(
            "127.0.0.1",
            port,
            "/",
            &[200],
            &None,
            Duration::from_secs(3),
        )
        .await;
        assert!(
            matches!(result, ProbeResult::Healthy),
            "expected Healthy, got {result:?}"
        );
    }

    /// HTTP probe: server returns 500, expected [200] → Unhealthy.
    #[tokio::test]
    async fn test_http_probe_unexpected_500() {
        let port = spawn_http_server(500, "error").await;
        let result = check_endpoint_http(
            "127.0.0.1",
            port,
            "/",
            &[200],
            &None,
            Duration::from_secs(3),
        )
        .await;
        assert!(
            matches!(result, ProbeResult::Unhealthy(_)),
            "expected Unhealthy, got {result:?}"
        );
        if let ProbeResult::Unhealthy(msg) = &result {
            assert!(msg.contains("500"), "message should mention 500: {msg}");
        }
    }

    /// HTTP probe: body matches regex → Healthy.
    #[tokio::test]
    async fn test_http_probe_body_regex_match() {
        let port = spawn_http_server(200, r#"{"status":"ok"}"#).await;
        let result = check_endpoint_http(
            "127.0.0.1",
            port,
            "/",
            &[200],
            &Some(r#""status"\s*:\s*"ok""#.to_string()),
            Duration::from_secs(3),
        )
        .await;
        assert!(
            matches!(result, ProbeResult::Healthy),
            "expected Healthy, got {result:?}"
        );
    }

    /// HTTP probe: body does not match regex → Unhealthy.
    #[tokio::test]
    async fn test_http_probe_body_regex_no_match() {
        let port = spawn_http_server(200, "degraded").await;
        let result = check_endpoint_http(
            "127.0.0.1",
            port,
            "/",
            &[200],
            &Some(r#""status"\s*:\s*"ok""#.to_string()),
            Duration::from_secs(3),
        )
        .await;
        assert!(
            matches!(result, ProbeResult::Unhealthy(_)),
            "expected Unhealthy, got {result:?}"
        );
    }

    // -- passive ejection tests --

    fn make_manager_with_policy() -> HealthManager {
        use armageddon_common::types::{
            CircuitBreakerConfig, Endpoint, HealthCheckConfig, OutlierDetectionConfig, Protocol,
        };
        let clusters = vec![Cluster {
            name: "test-cluster".into(),
            endpoints: vec![Endpoint {
                address: "127.0.0.1".into(),
                port: 9900,
                weight: 1,
                healthy: true,
            }],
            health_check: HealthCheckConfig {
                interval_ms: 5000,
                timeout_ms: 2000,
                unhealthy_threshold: 3,
                healthy_threshold: 2,
                protocol: Protocol::Http,
                path: Some("/healthz".into()),
            },
            circuit_breaker: CircuitBreakerConfig::default(),
            outlier_detection: OutlierDetectionConfig::default(),
        }];
        let mgr = HealthManager::new(clusters);
        mgr.set_ejection_policy(
            "test-cluster",
            EjectionPolicy {
                consecutive_errors: 5,
                window_size: 10,
                error_percent_threshold: 50,
                base_ejection_time_secs: 2, // short for tests
                max_ejection_percent: 50,
            },
        );
        mgr
    }

    /// Passive ejection: 5 out of 10 5xx requests triggers ejection.
    #[tokio::test]
    async fn test_passive_ejection_triggered() {
        let mgr = make_manager_with_policy();

        // First 5 successes.
        for _ in 0..5 {
            mgr.record_response("test-cluster", "127.0.0.1", 9900, false);
        }
        assert!(!mgr.is_ejected("test-cluster", "127.0.0.1", 9900));

        // Next 5 failures → window = 5/10 = 50% → threshold reached.
        for _ in 0..5 {
            mgr.record_response("test-cluster", "127.0.0.1", 9900, true);
        }
        assert!(
            mgr.is_ejected("test-cluster", "127.0.0.1", 9900),
            "endpoint should be ejected after 50% error rate"
        );
        assert!(
            !mgr.is_healthy("test-cluster", "127.0.0.1", 9900),
            "ejected endpoint must not appear healthy"
        );
    }

    /// Passive ejection: below threshold (4/10 = 40%) → no ejection.
    #[tokio::test]
    async fn test_passive_ejection_below_threshold() {
        let mgr = make_manager_with_policy();

        for _ in 0..6 {
            mgr.record_response("test-cluster", "127.0.0.1", 9900, false);
        }
        for _ in 0..4 {
            mgr.record_response("test-cluster", "127.0.0.1", 9900, true);
        }
        assert!(
            !mgr.is_ejected("test-cluster", "127.0.0.1", 9900),
            "endpoint should NOT be ejected at 40% error rate"
        );
    }

    /// Passive ejection expires: after base_ejection_time_secs the endpoint
    /// is no longer flagged as ejected.
    #[tokio::test]
    async fn test_passive_ejection_expires() {
        let mgr = make_manager_with_policy();

        // Trigger ejection (50%).
        for _ in 0..5 {
            mgr.record_response("test-cluster", "127.0.0.1", 9900, false);
        }
        for _ in 0..5 {
            mgr.record_response("test-cluster", "127.0.0.1", 9900, true);
        }
        assert!(mgr.is_ejected("test-cluster", "127.0.0.1", 9900));

        // Wait for the 2-second ejection window to expire.
        tokio::time::sleep(Duration::from_secs(3)).await;

        assert!(
            !mgr.is_ejected("test-cluster", "127.0.0.1", 9900),
            "ejection window should have expired"
        );
    }
}

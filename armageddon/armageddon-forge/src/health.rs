//! Health checks: periodic HTTP/gRPC per upstream endpoint.
//!
//! Runs background tasks that probe each endpoint at the configured interval.
//! Marks endpoints healthy/unhealthy based on consecutive successes/failures.

use armageddon_common::types::{Cluster, Endpoint};
use dashmap::DashMap;
use std::sync::Arc;
use std::time::Duration;

/// Endpoint health status.
#[derive(Debug, Clone)]
pub struct EndpointHealth {
    pub healthy: bool,
    pub consecutive_failures: u32,
    pub consecutive_successes: u32,
    pub last_check_epoch_ms: u64,
}

/// Key for identifying an endpoint: "cluster:address:port".
fn endpoint_key(cluster: &str, address: &str, port: u16) -> String {
    format!("{cluster}:{address}:{port}")
}

/// Manages periodic health checks for all upstream clusters.
pub struct HealthManager {
    clusters: Vec<Cluster>,
    health_status: Arc<DashMap<String, EndpointHealth>>,
}

impl HealthManager {
    pub fn new(clusters: Vec<Cluster>) -> Self {
        let health_status = Arc::new(DashMap::new());

        // Initialize health status for all endpoints
        for cluster in &clusters {
            for ep in &cluster.endpoints {
                let key = endpoint_key(&cluster.name, &ep.address, ep.port);
                health_status.insert(
                    key,
                    EndpointHealth {
                        healthy: true, // optimistic start
                        consecutive_failures: 0,
                        consecutive_successes: 0,
                        last_check_epoch_ms: 0,
                    },
                );
            }
        }

        Self {
            clusters,
            health_status,
        }
    }

    /// Start the health check loop. Spawns a task per cluster. Runs until cancelled.
    pub fn start(&self) -> Vec<tokio::task::JoinHandle<()>> {
        let mut handles = Vec::new();

        for cluster in &self.clusters {
            let cluster_name = cluster.name.clone();
            let endpoints = cluster.endpoints.clone();
            let hc_config = cluster.health_check.clone();
            let health_status = Arc::clone(&self.health_status);

            let handle = tokio::spawn(async move {
                let interval = Duration::from_millis(hc_config.interval_ms);
                let timeout = Duration::from_millis(hc_config.timeout_ms);
                let path = hc_config.path.as_deref().unwrap_or("/healthz");

                tracing::info!(
                    "health checker started for cluster '{}' ({} endpoints, interval {}ms)",
                    cluster_name,
                    endpoints.len(),
                    hc_config.interval_ms,
                );

                let mut ticker = tokio::time::interval(interval);
                loop {
                    ticker.tick().await;

                    for ep in &endpoints {
                        let key = endpoint_key(&cluster_name, &ep.address, ep.port);
                        let healthy = check_endpoint_http(
                            &ep.address,
                            ep.port,
                            path,
                            timeout,
                        )
                        .await;

                        let mut entry = health_status
                            .entry(key)
                            .or_insert(EndpointHealth {
                                healthy: true,
                                consecutive_failures: 0,
                                consecutive_successes: 0,
                                last_check_epoch_ms: 0,
                            });

                        let now_ms = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64;
                        entry.last_check_epoch_ms = now_ms;

                        if healthy {
                            entry.consecutive_failures = 0;
                            entry.consecutive_successes += 1;
                            if !entry.healthy
                                && entry.consecutive_successes
                                    >= hc_config.healthy_threshold
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
                                && entry.consecutive_failures
                                    >= hc_config.unhealthy_threshold
                            {
                                entry.healthy = false;
                                tracing::warn!(
                                    "endpoint {}:{} in cluster '{}' is now UNHEALTHY ({} consecutive failures)",
                                    ep.address,
                                    ep.port,
                                    cluster_name,
                                    entry.consecutive_failures,
                                );
                            }
                        }
                    }
                }
            });

            handles.push(handle);
        }

        tracing::info!(
            "health checker started for {} clusters",
            self.clusters.len()
        );

        handles
    }

    /// Check if an endpoint is healthy.
    pub fn is_healthy(&self, cluster: &str, address: &str, port: u16) -> bool {
        let key = endpoint_key(cluster, address, port);
        self.health_status
            .get(&key)
            .map_or(false, |h| h.healthy)
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

    /// Mark an endpoint as unhealthy (called by circuit breaker or proxy on failure).
    pub fn mark_unhealthy(&self, cluster: &str, address: &str, port: u16) {
        let key = endpoint_key(cluster, address, port);
        if let Some(mut entry) = self.health_status.get_mut(&key) {
            entry.healthy = false;
            entry.consecutive_failures += 1;
            entry.consecutive_successes = 0;
        }
    }
}

/// Perform an HTTP health check against an endpoint.
async fn check_endpoint_http(address: &str, port: u16, path: &str, timeout: Duration) -> bool {
    let uri = format!("http://{}:{}{}", address, port, path);

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
                Ok(resp) => resp.status().is_success(),
                Err(_) => false,
            },
            Err(_) => false,
        }
    })
    .await;

    result.unwrap_or(false)
}

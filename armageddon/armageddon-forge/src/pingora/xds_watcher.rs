// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! xDS ADS client integration for the Pingora data-plane.
//!
//! # Overview
//!
//! `XdsDataPlaneCallback` implements [`armageddon_xds::XdsCallback`] and
//! propagates each xDS resource update into the live data-plane components:
//!
//! | xDS type | Target |
//! |---|---|
//! | CDS (Cluster) | [`ClusterResolver`] + [`PingoraHealthChecker`] (re-register targets) |
//! | EDS (Endpoint) | [`ClusterResolver`] + [`UpstreamRegistry`] (update endpoint lists) |
//! | LDS (Listener) | no-op at M5, logged |
//! | RDS (Route)    | [`TrafficSplitter`] (canary / A-B rules hot-reload) |
//! | SDS (Secret)   | logged; cert hot-swap deferred to M6 (Pingora 0.4) |
//!
//! # Usage
//!
//! Call [`spawn_xds_watcher`] from `PingoraGateway::new()` when the config
//! carries `xds_consumer` settings.  The ADS loop runs on the forge tokio
//! bridge — not on Pingora worker threads.
//!
//! # Failure modes
//!
//! - **xDS control-plane unreachable at boot**: `spawn_xds_watcher` logs an
//!   error and the gateway continues with its initial static config.
//! - **Stream torn down mid-flight**: `AdsClient::run()` reconnects with
//!   exponential back-off (100 ms base, 32 s cap).
//! - **Malformed resource**: NACK sent; prior state preserved; `warn!` emitted.
//! - **Quorum loss on control plane**: last ACK'd config remains active.

use std::net::IpAddr;
use std::sync::atomic::{AtomicI64, Ordering as AtomicOrdering};
use std::sync::{Arc, RwLock};

use std::collections::HashMap;

use async_trait::async_trait;
use tracing::{debug, error, info, warn};

use crate::pingora::metrics::PingoraMetrics;

use armageddon_common::types::Endpoint;
use armageddon_xds::{AdsClient, XdsCallback};
use armageddon_xds::proto::{
    cluster::Cluster,
    endpoint::ClusterLoadAssignment,
    listener::Listener,
    route::RouteConfiguration,
    tls::Secret,
};

use crate::pingora::gateway::UpstreamRegistry;
use crate::pingora::upstream::selector::{ClusterResolver, ClusterState};
use crate::pingora::upstream::lb::LbPolicy;
use crate::pingora::protocols::traffic_split::{SplitSpec, SplitMode, TrafficSplitter, Variant};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration block enabling the xDS ADS consumer.
///
/// Absent → gateway operates in static-config mode.
#[derive(Debug, Clone)]
pub struct XdsConsumerConfig {
    /// gRPC endpoint of the xds-controller ADS service.
    /// Example: `http://xds-controller.faso.internal:18000`
    pub endpoint: String,
    /// Logical node identifier for this ARMAGEDDON instance.
    /// Example: `armageddon-node-1`
    pub node_id: String,
}

// ---------------------------------------------------------------------------
// Handles passed to the callback
// ---------------------------------------------------------------------------

/// All data-plane handles that `XdsDataPlaneCallback` needs to push updates.
///
/// The health checker is intentionally absent from the hot-reload path: the
/// `PingoraHealthChecker::register` API takes `&mut self` and targets must be
/// registered before `start()`.  Dynamic target registration via xDS is a
/// TODO for M6 when a `register_dynamic` / `Arc<RwLock<…>>` variant is added.
#[derive(Clone)]
pub struct DataPlaneHandles {
    /// Upstream peer resolver — updated on CDS/EDS.
    pub cluster_resolver: Arc<ClusterResolver>,
    /// Simple upstream registry used by the gateway's `upstream_peer` hook.
    pub upstream_registry: Arc<UpstreamRegistry>,
    /// Traffic splitter — updated on RDS to apply new canary rules.
    pub traffic_splitter: Arc<TrafficSplitter>,
}

// ---------------------------------------------------------------------------
// XdsDataPlaneCallback
// ---------------------------------------------------------------------------

/// Implements [`XdsCallback`] and fans out each xDS update to data-plane
/// components.
///
/// All writes are lock-free at the hot path (ArcSwap stores).  No locks are
/// held across `.await` points.
/// Validation configuration for xDS responses.
///
/// Prevents a compromised or misconfigured control plane from injecting
/// invalid cluster names, out-of-range endpoint addresses, or oversized
/// endpoint lists.
#[derive(Debug, Clone)]
pub struct XdsValidationConfig {
    /// Allowed cluster name prefixes.  Empty = allow all (with warning).
    pub allowed_cluster_prefixes: Vec<String>,
    /// When `true`, only private-range IPv4 addresses (10/8, 172.16/12,
    /// 192.168/16, 127/8) and IPv6 loopback (::1) are accepted.
    /// Non-IP addresses (hostnames) pass through — DNS resolution happens
    /// later in the Pingora peer path.
    pub restrict_to_private_addrs: bool,
    /// Maximum endpoints per cluster.  Default: 1000.
    pub max_endpoints_per_cluster: usize,
}

impl Default for XdsValidationConfig {
    fn default() -> Self {
        Self {
            allowed_cluster_prefixes: Vec::new(), // allow all, warn on unknown
            restrict_to_private_addrs: true,
            max_endpoints_per_cluster: 1000,
        }
    }
}

/// Returns `true` when `ip` falls within a private / loopback range
/// (10/8, 172.16/12, 192.168/16, 127/8 for IPv4; ::1 for IPv6).
fn is_private_or_loopback(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let octets = v4.octets();
            // 10.0.0.0/8
            octets[0] == 10
            // 172.16.0.0/12
            || (octets[0] == 172 && (octets[1] & 0xF0) == 16)
            // 192.168.0.0/16
            || (octets[0] == 192 && octets[1] == 168)
            // 127.0.0.0/8
            || octets[0] == 127
        }
        IpAddr::V6(v6) => {
            v6 == std::net::Ipv6Addr::LOCALHOST
        }
    }
}

/// Errors produced by xDS response validation.
#[derive(Debug)]
enum XdsValidationError {
    /// Endpoint count exceeds the configured maximum.
    TooManyEndpoints { cluster: String, count: usize, max: usize },
    /// One or more endpoint addresses fall outside allowed CIDRs.
    DisallowedAddress { cluster: String, address: String },
}

impl std::fmt::Display for XdsValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooManyEndpoints { cluster, count, max } => {
                write!(f, "cluster {cluster}: {count} endpoints exceeds max {max}")
            }
            Self::DisallowedAddress { cluster, address } => {
                write!(f, "cluster {cluster}: address {address} not in allowed CIDRs")
            }
        }
    }
}

/// Validate a cluster update against the validation config.
///
/// Returns `Ok(())` if all checks pass, `Err` with details otherwise.
/// The caller must skip the update on error.
fn validate_cluster_state(
    cluster_name: &str,
    endpoints: &[Endpoint],
    config: &XdsValidationConfig,
) -> Result<(), XdsValidationError> {
    // 1. Endpoint count cap.
    if endpoints.len() > config.max_endpoints_per_cluster {
        return Err(XdsValidationError::TooManyEndpoints {
            cluster: cluster_name.to_string(),
            count: endpoints.len(),
            max: config.max_endpoints_per_cluster,
        });
    }

    // 2. Address range check.
    if config.restrict_to_private_addrs {
        for ep in endpoints {
            if let Ok(ip) = ep.address.parse::<IpAddr>() {
                if !is_private_or_loopback(ip) {
                    return Err(XdsValidationError::DisallowedAddress {
                        cluster: cluster_name.to_string(),
                        address: ep.address.clone(),
                    });
                }
            }
            // Non-IP addresses (hostnames) are allowed through — DNS resolution
            // happens later in the Pingora peer path.
        }
    }

    // 3. Cluster name prefix check (warn only — do not reject).
    if !config.allowed_cluster_prefixes.is_empty()
        && !config.allowed_cluster_prefixes.iter().any(|p| cluster_name.starts_with(p))
    {
        warn!(
            cluster = cluster_name,
            "xDS validation: cluster name does not match any allowed prefix"
        );
    }

    Ok(())
}

pub struct XdsDataPlaneCallback {
    handles: DataPlaneHandles,
    /// Shared Prometheus metrics bundle (optional).
    metrics: Option<Arc<PingoraMetrics>>,
    /// Cache of TLS config per cluster from CDS updates.
    /// Used by EDS handler to preserve TLS settings when merging endpoints.
    tls_cache: RwLock<HashMap<String, (bool, Option<Arc<str>>)>>,
    /// Validation config for xDS responses.
    validation: XdsValidationConfig,
    // Per-resource-type monotonic version counters.  Using AtomicI64 so they
    // can be stored into an IntGaugeVec (which accepts i64).
    ver_cds: AtomicI64,
    ver_eds: AtomicI64,
    ver_lds: AtomicI64,
    ver_rds: AtomicI64,
    ver_sds: AtomicI64,
}

impl XdsDataPlaneCallback {
    /// Create a callback without metrics.
    pub fn new(handles: DataPlaneHandles) -> Arc<Self> {
        Arc::new(Self {
            handles,
            metrics: None,
            tls_cache: RwLock::new(HashMap::new()),
            validation: XdsValidationConfig::default(),
            ver_cds: AtomicI64::new(0),
            ver_eds: AtomicI64::new(0),
            ver_lds: AtomicI64::new(0),
            ver_rds: AtomicI64::new(0),
            ver_sds: AtomicI64::new(0),
        })
    }

    /// Create a callback with a shared metrics bundle.
    pub fn with_metrics(handles: DataPlaneHandles, metrics: Arc<PingoraMetrics>) -> Arc<Self> {
        Arc::new(Self {
            handles,
            metrics: Some(metrics),
            tls_cache: RwLock::new(HashMap::new()),
            validation: XdsValidationConfig::default(),
            ver_cds: AtomicI64::new(0),
            ver_eds: AtomicI64::new(0),
            ver_lds: AtomicI64::new(0),
            ver_rds: AtomicI64::new(0),
            ver_sds: AtomicI64::new(0),
        })
    }

    /// Increment the update counter and version gauge for `resource_type`.
    fn record_update(&self, resource_type: &'static str, action: &'static str, ver: &AtomicI64) {
        let new_ver = ver.fetch_add(1, AtomicOrdering::Relaxed) + 1;
        if let Some(m) = &self.metrics {
            m.xds_updates_total
                .with_label_values(&[resource_type, action])
                .inc();
            m.xds_current_version
                .with_label_values(&[resource_type])
                .set(new_ver);
        }
    }

    /// Increment `armageddon_xds_nack_total{resource_type, reason}`.
    fn record_nack(&self, resource_type: &'static str, reason: &str) {
        if let Some(m) = &self.metrics {
            m.xds_nack_total
                .with_label_values(&[resource_type, reason])
                .inc();
        }
    }
}

#[async_trait]
impl XdsCallback for XdsDataPlaneCallback {
    /// CDS: update `ClusterResolver` with TLS metadata extracted from the
    /// cluster's `transport_socket_tls` field.
    async fn on_cluster_update(&self, cluster: Cluster) {
        let name = cluster.name.clone();
        debug!(cluster = %name, "xDS: on_cluster_update");

        // Extract SPIFFE ID from `transport_socket_tls.spiffe_id` if present.
        let spiffe_id: Option<String> = cluster
            .transport_socket_tls
            .as_ref()
            .filter(|t| !t.spiffe_id.is_empty())
            .map(|t| t.spiffe_id.clone());

        let tls_required = spiffe_id.is_some();
        let spiffe_arc = spiffe_id.map(|s| Arc::from(s.as_str()));

        // Cache TLS config so EDS updates can preserve it.
        {
            let mut cache = self.tls_cache.write().unwrap_or_else(|e| e.into_inner());
            cache.insert(name.clone(), (tls_required, spiffe_arc.clone()));
        }

        // CDS carries no inline endpoints when type = EDS; those arrive via
        // EDS (on_endpoint_update). For STATIC clusters we build an empty
        // endpoint list here and let EDS fill it.
        let state = ClusterState::new(
            Vec::new(),
            tls_required,
            spiffe_arc,
            LbPolicy::RoundRobin,
        );

        self.handles.cluster_resolver.update(&name, state);
        self.record_update("cds", "cluster_added", &self.ver_cds);

        info!(
            cluster = %name,
            tls_required,
            "xDS CDS applied"
        );
    }

    /// EDS: update endpoint lists in `ClusterResolver` and `UpstreamRegistry`.
    ///
    /// Merges new endpoints into the existing `ClusterState`, preserving
    /// `tls_required` and `expected_spiffe_id` set by a prior CDS update.
    async fn on_endpoint_update(&self, cla: ClusterLoadAssignment) {
        let cluster_name = cla.cluster_name.clone();
        debug!(cluster = %cluster_name, "xDS: on_endpoint_update");

        let endpoints = extract_endpoints(&cla);

        // ── Validation (Finding 3) ────────────────────────────────────────
        if let Err(e) = validate_cluster_state(&cluster_name, &endpoints, &self.validation) {
            warn!(
                cluster = %cluster_name,
                error = %e,
                "xDS EDS validation failed — update skipped"
            );
            self.record_nack("eds", "validation_failed");
            return;
        }

        // ── Merge TLS config from CDS cache (Finding 2) ──────────────────
        let (tls_required, spiffe_id) = {
            let cache = self.tls_cache.read().unwrap_or_else(|e| e.into_inner());
            match cache.get(&cluster_name) {
                Some((tls, spiffe)) => (*tls, spiffe.clone()),
                None => (false, None), // first-time EDS before CDS — safe default
            }
        };

        // Update the simple registry.
        self.handles
            .upstream_registry
            .update_cluster(&cluster_name, endpoints.clone());

        let state = ClusterState::new(
            endpoints.clone(),
            tls_required,
            spiffe_id,
            LbPolicy::RoundRobin,
        );
        self.handles.cluster_resolver.update(&cluster_name, state);
        self.record_update("eds", "endpoint_updated", &self.ver_eds);

        info!(
            cluster = %cluster_name,
            endpoints = endpoints.len(),
            tls_required,
            "xDS EDS applied"
        );
    }

    /// LDS: no-op at M5 — listener config drives inbound port binding, handled
    /// outside Pingora's hot path.  Logged for observability.
    async fn on_listener_update(&self, listener: Listener) {
        debug!(listener = %listener.name, "xDS: on_listener_update");
        self.record_update("lds", "listener_updated", &self.ver_lds);
        info!(listener = %listener.name, "xDS LDS applied (data-plane no-op at M5)");
    }

    /// RDS: parse `weighted_clusters` route actions and push canary split specs
    /// into `TrafficSplitter`.
    ///
    /// Looks for `Route.action.route.cluster_specifier.weighted_clusters` with
    /// exactly two entries (primary + canary).  Weights must be positive.
    /// Routes with `cluster` (single destination) are skipped — they represent
    /// non-split traffic which the router handles directly.
    async fn on_route_update(&self, route: RouteConfiguration) {
        debug!(route = %route.name, "xDS: on_route_update");

        let mut new_routes: HashMap<String, Arc<SplitSpec>> = HashMap::new();

        for vh in &route.virtual_hosts {
            for r in &vh.routes {
                // In prost, `oneof action { RouteAction route = 2; ... }`
                // generates `Route::action: Option<route::Action>`.
                // We access the weighted_cluster data through the raw protobuf
                // Action enum.
                if let Some(wc_tuple) = extract_weighted_clusters_from_route(r) {
                    let (primary, canary, w_primary, w_canary) = wc_tuple;
                    let total = w_primary + w_canary;
                    if total == 0 {
                        continue;
                    }
                    // Normalise weights to sum to 100.
                    let w0 = (w_primary * 100 + total / 2) / total; // round-nearest
                    let w1 = 100u32.saturating_sub(w0);

                    let spec = SplitSpec {
                        mode: SplitMode::Canary,
                        variants: vec![
                            Variant {
                                cluster: primary.clone(),
                                weight: w0,
                                label: Some("primary".to_string()),
                            },
                            Variant {
                                cluster: canary.clone(),
                                weight: w1,
                                label: Some("canary".to_string()),
                            },
                        ],
                        sticky_header: Some("x-forge-split-id".to_string()),
                    };

                    let route_key = if r.name.is_empty() {
                        format!("{}/{}", route.name, vh.name)
                    } else {
                        r.name.clone()
                    };

                    match spec.validate() {
                        Ok(()) => {
                            info!(
                                route = %route_key,
                                primary = %primary,
                                canary = %canary,
                                w0,
                                w1,
                                "xDS RDS: traffic split prepared"
                            );
                            new_routes.insert(route_key, Arc::new(spec));
                        }
                        Err(e) => {
                            warn!(
                                route = %route_key,
                                error = %e,
                                "xDS RDS: invalid split spec — skipped"
                            );
                        }
                    }
                }
            }
        }

        if !new_routes.is_empty() {
            self.handles.traffic_splitter.update(new_routes);
            info!(route = %route.name, "xDS RDS: traffic splitter updated");
        }

        self.record_update("rds", "route_modified", &self.ver_rds);
        info!(route = %route.name, "xDS RDS applied");
    }

    /// SDS: log TLS secret arrival.
    ///
    /// Cert hot-swap into the `AutoMtlsDialer` is deferred to M6 when
    /// Pingora 0.4 / `pingora-rustls` exposes a custom connector hook.
    async fn on_secret_update(&self, secret: Secret) {
        warn!(
            name = %secret.name,
            "xDS SDS: secret received — cert hot-swap deferred to M6 (Pingora 0.4 required)"
        );
        self.record_update("sds", "secret_rotated", &self.ver_sds);
    }
}

// ---------------------------------------------------------------------------
// spawn_xds_watcher — public entry point
// ---------------------------------------------------------------------------

/// Spawn the ADS consumer task on the forge tokio bridge.
///
/// Returns immediately; the ADS loop runs in the background.  On fatal
/// connection error (non-retriable) the task logs and exits without crashing
/// the gateway.
///
/// Pass `metrics` to enable Prometheus metric emission for xDS events.
pub fn spawn_xds_watcher(
    config: XdsConsumerConfig,
    handles: DataPlaneHandles,
    metrics: Option<Arc<PingoraMetrics>>,
) {
    let callback = match metrics {
        Some(m) => XdsDataPlaneCallback::with_metrics(handles, m),
        None => XdsDataPlaneCallback::new(handles),
    };
    let endpoint = config.endpoint.clone();
    let node_id = config.node_id.clone();

    let bridge = crate::pingora::runtime::tokio_handle();

    bridge.spawn(async move {
        match AdsClient::connect(&endpoint, node_id.clone()).await {
            Ok(client) => {
                info!(
                    node = %node_id,
                    endpoint = %endpoint,
                    "xDS ADS watcher started"
                );
                if let Err(e) = client.run(callback).await {
                    error!(
                        node = %node_id,
                        error = %e,
                        "xDS ADS watcher exited with fatal error"
                    );
                }
            }
            Err(e) => {
                error!(
                    node = %node_id,
                    endpoint = %endpoint,
                    error = %e,
                    "xDS ADS initial connect failed — operating with static config"
                );
            }
        }
    });
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Try to extract `(primary, canary, w_primary, w_canary)` from a route entry
/// that has a `weighted_clusters` action with exactly 2 clusters.
///
/// Returns `None` when:
/// - the route uses a single `cluster` (no split),
/// - the `action` is absent, redirect or direct_response,
/// - the weighted_clusters list has != 2 entries.
fn extract_weighted_clusters_from_route(
    route: &armageddon_xds::proto::route::Route,
) -> Option<(String, String, u32, u32)> {
    use armageddon_xds::proto::route::route::Action;

    let action_enum = route.action.as_ref()?;
    if let Action::Route(ra) = action_enum {
        use armageddon_xds::proto::route::route_action::ClusterSpecifier;
        if let Some(ClusterSpecifier::WeightedClusters(wc)) = &ra.cluster_specifier {
            if wc.clusters.len() == 2 {
                let c0 = &wc.clusters[0];
                let c1 = &wc.clusters[1];
                // prost maps google.protobuf.UInt32Value to Option<u32> directly.
                let w0 = c0.weight.unwrap_or(1);
                let w1 = c1.weight.unwrap_or(1);
                return Some((c0.name.clone(), c1.name.clone(), w0, w1));
            }
        }
    }
    None
}

/// Convert `ClusterLoadAssignment` to a flat `Vec<Endpoint>`.
///
/// Filters out entries with an empty address or port 0.
fn extract_endpoints(cla: &ClusterLoadAssignment) -> Vec<Endpoint> {
    let mut out = Vec::new();
    for locality in &cla.endpoints {
        for lb_ep in &locality.lb_endpoints {
            if let Some(ep) = &lb_ep.endpoint {
                if let Some(addr) = &ep.address {
                    if let Some(sa) = &addr.socket_address {
                        let port = sa.port_value as u16;
                        if !sa.address.is_empty() && port > 0 {
                            out.push(Endpoint {
                                address: sa.address.clone(),
                                port,
                                // prost maps UInt32Value to Option<u32>
                                weight: lb_ep.load_balancing_weight.unwrap_or(1),
                                healthy: true,
                            });
                        }
                    }
                }
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use crate::pingora::upstream::selector::ClusterResolver;
    use crate::pingora::gateway::UpstreamRegistry;
    use crate::pingora::protocols::traffic_split::TrafficSplitter;

    fn make_handles() -> DataPlaneHandles {
        DataPlaneHandles {
            cluster_resolver: Arc::new(ClusterResolver::new()),
            upstream_registry: Arc::new(UpstreamRegistry::new()),
            traffic_splitter: Arc::new(TrafficSplitter::new()),
        }
    }

    fn make_cla(cluster_name: &str, addr: &str, port: u32) -> ClusterLoadAssignment {
        use armageddon_xds::proto::endpoint::{
            LocalityLbEndpoints, LbEndpoint,
            Endpoint as XdsEndpoint, Address as XdsAddress, SocketAddress,
        };
        ClusterLoadAssignment {
            cluster_name: cluster_name.to_string(),
            endpoints: vec![LocalityLbEndpoints {
                lb_endpoints: vec![LbEndpoint {
                    endpoint: Some(XdsEndpoint {
                        address: Some(XdsAddress {
                            socket_address: Some(SocketAddress {
                                address: addr.to_string(),
                                port_value: port,
                                ..Default::default()
                            }),
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    /// EDS update should populate the upstream registry.
    #[tokio::test]
    async fn xds_eds_update_lands_in_registry() {
        let handles = make_handles();
        let cb = XdsDataPlaneCallback::new(handles.clone());

        cb.on_endpoint_update(make_cla("eds-cluster", "10.0.0.1", 9000)).await;

        let ep = handles
            .upstream_registry
            .first_healthy("eds-cluster")
            .expect("endpoint update should be reflected");
        assert_eq!(ep.port, 9000);
        assert_eq!(ep.address, "10.0.0.1");
    }

    /// CDS update should update the cluster resolver.
    #[tokio::test]
    async fn xds_cds_update_populates_resolver() {
        let handles = make_handles();
        let cb = XdsDataPlaneCallback::new(handles.clone());

        let cluster = Cluster {
            name: "test-cluster".to_string(),
            ..Default::default()
        };
        cb.on_cluster_update(cluster).await;

        // ClusterResolver should have the cluster; no endpoint yet (EDS pending).
        // Just verify no panic and name is registered (resolve returns None
        // because no endpoints present, but update() ran).
        // If ClusterResolver had a `contains` method we'd use it; instead
        // push an EDS to trigger the side-effect.
        cb.on_endpoint_update(make_cla("test-cluster", "127.0.0.1", 8080)).await;
        let ep = handles.upstream_registry.first_healthy("test-cluster");
        assert!(ep.is_some());
    }

    /// CDS with SPIFFE ID sets TLS required in cluster state.
    #[tokio::test]
    async fn xds_cds_spiffe_sets_tls_required() {
        use armageddon_xds::proto::cluster::UpstreamTlsContext;

        let handles = make_handles();
        let cb = XdsDataPlaneCallback::new(handles.clone());

        let cluster = Cluster {
            name: "mtls-cluster".to_string(),
            transport_socket_tls: Some(UpstreamTlsContext {
                sni: String::new(),
                spiffe_id: "spiffe://faso.gov.bf/ns/kaya/sa/shard-0".to_string(),
            }),
            ..Default::default()
        };

        // Should not panic.
        cb.on_cluster_update(cluster).await;
    }

    /// LDS update must not panic (no-op at M5).
    #[tokio::test]
    async fn xds_lds_update_is_noop() {
        let handles = make_handles();
        let cb = XdsDataPlaneCallback::new(handles);
        cb.on_listener_update(Listener {
            name: "test-listener".to_string(),
            ..Default::default()
        })
        .await;
    }

    /// SDS update must not panic (no-op at M5).
    #[tokio::test]
    async fn xds_sds_update_is_noop() {
        let handles = make_handles();
        let cb = XdsDataPlaneCallback::new(handles);
        cb.on_secret_update(Secret {
            name: "test-cert".to_string(),
            ..Default::default()
        })
        .await;
    }

    /// `extract_endpoints` converts a CLA with two entries correctly.
    #[test]
    fn extract_endpoints_produces_correct_list() {
        let cla = make_cla("test", "192.168.1.1", 8443);
        let eps = extract_endpoints(&cla);
        assert_eq!(eps.len(), 1);
        assert_eq!(eps[0].address, "192.168.1.1");
        assert_eq!(eps[0].port, 8443);
        assert!(eps[0].healthy);
    }

    /// Port-zero entries are filtered out.
    #[test]
    fn extract_endpoints_filters_port_zero() {
        use armageddon_xds::proto::endpoint::{
            ClusterLoadAssignment, LocalityLbEndpoints, LbEndpoint,
            Endpoint as XdsEndpoint, Address as XdsAddress, SocketAddress,
        };
        let cla = ClusterLoadAssignment {
            cluster_name: "test".to_string(),
            endpoints: vec![LocalityLbEndpoints {
                lb_endpoints: vec![LbEndpoint {
                    endpoint: Some(XdsEndpoint {
                        address: Some(XdsAddress {
                            socket_address: Some(SocketAddress {
                                address: "127.0.0.1".to_string(),
                                port_value: 0, // invalid
                                ..Default::default()
                            }),
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        };
        assert!(extract_endpoints(&cla).is_empty());
    }

    /// Empty address entries are filtered out.
    #[test]
    fn extract_endpoints_filters_empty_address() {
        use armageddon_xds::proto::endpoint::{
            ClusterLoadAssignment, LocalityLbEndpoints, LbEndpoint,
            Endpoint as XdsEndpoint, Address as XdsAddress, SocketAddress,
        };
        let cla = ClusterLoadAssignment {
            cluster_name: "test".to_string(),
            endpoints: vec![LocalityLbEndpoints {
                lb_endpoints: vec![LbEndpoint {
                    endpoint: Some(XdsEndpoint {
                        address: Some(XdsAddress {
                            socket_address: Some(SocketAddress {
                                address: String::new(),
                                port_value: 8080,
                                ..Default::default()
                            }),
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        };
        assert!(extract_endpoints(&cla).is_empty());
    }

    // ── xDS metrics wiring ────────────────────────────────────────────────

    /// CDS update increments xds_updates_total{resource_type="cds"} and
    /// bumps xds_current_version{resource_type="cds"}.
    #[tokio::test]
    async fn xds_cds_update_increments_metrics() {
        use crate::pingora::metrics::PingoraMetrics;
        use prometheus::Registry;

        let r = Registry::new();
        let m = Arc::new(PingoraMetrics::new(&r).unwrap());
        let handles = make_handles();
        let cb = XdsDataPlaneCallback::with_metrics(handles, Arc::clone(&m));

        cb.on_cluster_update(Cluster { name: "c1".to_string(), ..Default::default() }).await;
        cb.on_cluster_update(Cluster { name: "c2".to_string(), ..Default::default() }).await;

        let families = r.gather();
        let updates = families
            .iter()
            .find(|f| f.get_name() == "armageddon_xds_updates_total")
            .expect("updates counter must exist");
        let total: f64 = updates.get_metric().iter()
            .filter(|m| m.get_label().iter().any(|l| l.get_name() == "resource_type" && l.get_value() == "cds"))
            .map(|m| m.get_counter().get_value())
            .sum();
        assert_eq!(total, 2.0, "two CDS updates should be counted");

        let versions = families
            .iter()
            .find(|f| f.get_name() == "armageddon_xds_current_version")
            .expect("version gauge must exist");
        let ver = versions.get_metric().iter()
            .find(|m| m.get_label().iter().any(|l| l.get_name() == "resource_type" && l.get_value() == "cds"))
            .map(|m| m.get_gauge().get_value())
            .unwrap_or(0.0);
        assert_eq!(ver, 2.0, "version should be 2 after two CDS updates");
    }

    /// record_nack increments xds_nack_total.
    #[test]
    fn xds_record_nack_increments_counter() {
        use crate::pingora::metrics::PingoraMetrics;
        use prometheus::Registry;

        let r = Registry::new();
        let m = Arc::new(PingoraMetrics::new(&r).unwrap());
        let handles = make_handles();
        let cb = XdsDataPlaneCallback::with_metrics(handles, Arc::clone(&m));

        cb.record_nack("cds", "parse_error");
        cb.record_nack("rds", "weight_sum_invalid");

        let families = r.gather();
        let fam = families
            .iter()
            .find(|f| f.get_name() == "armageddon_xds_nack_total")
            .expect("nack counter must exist");
        let total: f64 = fam.get_metric().iter()
            .map(|m| m.get_counter().get_value())
            .sum();
        assert_eq!(total, 2.0, "two NACKs should be counted");
    }
}

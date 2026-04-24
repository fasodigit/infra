// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Delta xDS ADS consumer — bidirectional gRPC stream using
//! `DeltaAggregatedResources` (incremental protocol).
//!
//! # Overview
//!
//! Delta xDS (also called "Incremental xDS") drastically reduces bandwidth for
//! large deployments.  Instead of re-sending the complete resource list on every
//! change, the server sends only:
//!
//! * `resources` — added or updated entries (name + version + payload)
//! * `removed_resources` — names of deleted entries
//!
//! # Protocol handshake
//!
//! 1. Client opens `DeltaAggregatedResources` and sends the first
//!    `DeltaDiscoveryRequest` with:
//!    - `node` identifier
//!    - `type_url` of the resource type
//!    - `resource_names_subscribe: []` (wildcard = all)
//!    - `initial_resource_versions: {}` (empty on fresh start)
//!
//! 2. Server responds with `DeltaDiscoveryResponse`:
//!    - `resources` — only resources the client does not yet have
//!    - `removed_resources` — names the server no longer exposes
//!    - `nonce` — opaque correlation token
//!
//! 3. Client applies the delta and sends ACK (`response_nonce = <nonce>`,
//!    `error_detail` absent).
//!    On any decode failure the client sends NACK (`error_detail` populated,
//!    `response_nonce = <nonce>`) and does NOT advance its local state.
//!
//! 4. The server will push further `DeltaDiscoveryResponse` frames whenever
//!    its configuration changes.
//!
//! # Failure modes
//!
//! * **Control-plane restart**: stream error triggers reconnect with
//!   exponential back-off (100 ms base, 32 s cap).  On reconnect the client
//!   re-sends `initial_resource_versions` so the server can skip already-known
//!   resources.
//!
//! * **Malformed resource**: decode failure triggers NACK; `ResourceCache` is
//!   NOT updated; the resource stays at its previously applied version.
//!
//! * **Idle timeout (30 s)**: same as SOTW path — stream is torn down and
//!   reconnect activates.
//!
//! * **Remove without prior add**: treated as a no-op (resource was never
//!   in cache, ignore silently).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use prost::Message as ProstMessage;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_stream::wrappers::ReceiverStream;
use tonic::transport::Channel;
use tonic::Request;
use tracing::{debug, error, info, warn};

use crate::error::XdsError;
use crate::proto::{
    cluster::Cluster,
    discovery::{
        aggregated_discovery_service_client::AggregatedDiscoveryServiceClient,
        DeltaDiscoveryRequest, DeltaDiscoveryResponse, Node,
    },
    endpoint::ClusterLoadAssignment,
    google_rpc,
    listener::Listener,
    route::RouteConfiguration,
    tls::Secret,
    type_urls,
};
use crate::resources::ResourceCache;

// ---------------------------------------------------------------------------
// Metrics
// ---------------------------------------------------------------------------

use prometheus::{IntCounterVec, Opts};
use std::sync::OnceLock;

static DELTA_RECEIVED_TOTAL: OnceLock<IntCounterVec> = OnceLock::new();
static DELTA_ACK_NACK_TOTAL: OnceLock<IntCounterVec> = OnceLock::new();

fn delta_received_total() -> &'static IntCounterVec {
    DELTA_RECEIVED_TOTAL.get_or_init(|| {
        IntCounterVec::new(
            Opts::new(
                "armageddon_xds_delta_received_total",
                "Total Delta xDS resources received by action and resource_type",
            ),
            &["resource_type", "action"],
        )
        .and_then(|c| {
            prometheus::register(Box::new(c.clone()))?;
            Ok(c)
        })
        .unwrap_or_else(|_| {
            // Fallback: create unregistered counter (for tests).
            IntCounterVec::new(
                Opts::new("armageddon_xds_delta_received_total_unregistered", ""),
                &["resource_type", "action"],
            )
            .expect("metric creation")
        })
    })
}

fn delta_ack_nack_total() -> &'static IntCounterVec {
    DELTA_ACK_NACK_TOTAL.get_or_init(|| {
        IntCounterVec::new(
            Opts::new(
                "armageddon_xds_delta_ack_nack_total",
                "Total Delta xDS ACKs and NACKs sent by resource_type and kind",
            ),
            &["resource_type", "kind"],
        )
        .and_then(|c| {
            prometheus::register(Box::new(c.clone()))?;
            Ok(c)
        })
        .unwrap_or_else(|_| {
            IntCounterVec::new(
                Opts::new("armageddon_xds_delta_ack_nack_total_unregistered", ""),
                &["resource_type", "kind"],
            )
            .expect("metric creation")
        })
    })
}

// Convenience wrappers.
fn inc_received(type_url: &str, action: &str) {
    let label = short_type(type_url);
    let _ = delta_received_total().get_metric_with_label_values(&[label, action]);
    delta_received_total()
        .with_label_values(&[label, action])
        .inc();
}

fn inc_ack_nack(type_url: &str, kind: &str) {
    let label = short_type(type_url);
    delta_ack_nack_total()
        .with_label_values(&[label, kind])
        .inc();
}

fn short_type(type_url: &str) -> &str {
    match type_url {
        type_urls::CLUSTER => "cluster",
        type_urls::ENDPOINT => "endpoint",
        type_urls::LISTENER => "listener",
        type_urls::ROUTE => "route",
        type_urls::SECRET => "secret",
        _ => "unknown",
    }
}

// ---------------------------------------------------------------------------
// Delta per-type subscription state
// ---------------------------------------------------------------------------

/// Per-type-url Delta subscription state for the client.
///
/// Tracks which resources the client knows and their last accepted version,
/// so on reconnect we can send `initial_resource_versions` to the server.
#[derive(Debug, Clone, Default)]
pub struct DeltaSubscription {
    /// type_url for this entry.
    pub type_url: String,
    /// name → last ACK'd version (content-hash from server).
    pub known_versions: HashMap<String, String>,
    /// True once we've sent the first subscription request.
    pub subscribed: bool,
}

impl DeltaSubscription {
    pub fn new(type_url: impl Into<String>) -> Self {
        Self {
            type_url: type_url.into(),
            known_versions: HashMap::new(),
            subscribed: false,
        }
    }

    /// Apply an ACK'd delta response — upsert / remove from known_versions.
    pub fn apply_ack(&mut self, resp: &DeltaDiscoveryResponse) {
        for r in &resp.resources {
            self.known_versions.insert(r.name.clone(), r.version.clone());
        }
        for name in &resp.removed_resources {
            self.known_versions.remove(name);
        }
    }
}

/// Map of all Delta subscriptions keyed by type_url.
#[derive(Debug, Default)]
pub struct DeltaSubscriptionMap {
    inner: HashMap<String, DeltaSubscription>,
}

impl DeltaSubscriptionMap {
    /// Build a map with all five xDS resource types.
    pub fn new_all_types() -> Self {
        let mut map = HashMap::new();
        for &url in type_urls::ALL {
            map.insert(url.to_string(), DeltaSubscription::new(url));
        }
        Self { inner: map }
    }

    pub fn get_mut(&mut self, type_url: &str) -> Option<&mut DeltaSubscription> {
        self.inner.get_mut(type_url)
    }

    pub fn all_mut(&mut self) -> impl Iterator<Item = &mut DeltaSubscription> {
        self.inner.values_mut()
    }
}

// ---------------------------------------------------------------------------
// DeltaXdsCallback
// ---------------------------------------------------------------------------

/// Callback interface invoked by `DeltaAdsClient` for each resource event.
///
/// Separate `on_*_removed` hooks are provided so downstream consumers can
/// clean up routing tables on removal without needing to track the diff
/// themselves.
#[async_trait]
pub trait DeltaXdsCallback: Send + Sync + 'static {
    async fn on_cluster_upsert(&self, cluster: Cluster);
    async fn on_cluster_removed(&self, name: String);

    async fn on_endpoint_upsert(&self, cla: ClusterLoadAssignment);
    async fn on_endpoint_removed(&self, name: String);

    async fn on_listener_upsert(&self, listener: Listener);
    async fn on_listener_removed(&self, name: String);

    async fn on_route_upsert(&self, route: RouteConfiguration);
    async fn on_route_removed(&self, name: String);

    async fn on_secret_upsert(&self, secret: Secret);
    async fn on_secret_removed(&self, name: String);
}

// ---------------------------------------------------------------------------
// DeltaAdsClient
// ---------------------------------------------------------------------------

/// Active Delta xDS ADS consumer.
///
/// Runs `DeltaAggregatedResources` over gRPC, applying incremental updates to
/// `ResourceCache`.  Reconnects with exponential back-off on any stream error.
///
/// # Construction
///
/// ```rust,no_run
/// # async fn doc() -> Result<(), armageddon_xds::XdsError> {
/// let client = armageddon_xds::delta_ads_client::DeltaAdsClient::connect_delta(
///     "http://xds-controller.faso.internal:18000",
///     "armageddon-node-1".to_string(),
/// ).await?;
/// # Ok(())
/// # }
/// ```
pub struct DeltaAdsClient {
    endpoint: String,
    node_id: String,
    pub resources: Arc<ResourceCache>,
}

impl DeltaAdsClient {
    /// Connect to the xds-controller at `endpoint`.
    ///
    /// Establishes the underlying TCP/HTTP-2 channel eagerly so that transport
    /// errors surface here rather than inside `run_delta`.
    pub async fn connect_delta(endpoint: &str, node_id: String) -> Result<Self, XdsError> {
        Channel::from_shared(endpoint.to_string())
            .map_err(|e| XdsError::Connection {
                endpoint: endpoint.to_string(),
                source: Box::new(e),
            })?
            .connect()
            .await
            .map_err(|e| XdsError::Connection {
                endpoint: endpoint.to_string(),
                source: Box::new(e),
            })?;

        info!(node = %node_id, endpoint = %endpoint, "Delta xDS ADS channel established");

        Ok(Self {
            endpoint: endpoint.to_string(),
            node_id,
            resources: Arc::new(ResourceCache::new()),
        })
    }

    /// Run the Delta ADS consume loop until a non-retriable error occurs or
    /// the task is cancelled.
    ///
    /// Reconnects with exponential back-off on any stream error.
    pub async fn run_delta(
        self,
        callback: Arc<dyn DeltaXdsCallback>,
    ) -> Result<(), XdsError> {
        let mut subs = DeltaSubscriptionMap::new_all_types();
        let mut attempt: u32 = 0;

        const BASE_MS: u64 = 100;
        const CAP_MS: u64 = 32_000;

        loop {
            match self.run_delta_stream(&mut subs, callback.clone()).await {
                Ok(()) => {
                    info!(node = %self.node_id, "Delta xDS ADS stream closed cleanly");
                    return Ok(());
                }
                Err(XdsError::IdleTimeout { secs }) => {
                    warn!(
                        node = %self.node_id,
                        secs,
                        "Delta xDS ADS idle timeout — reconnecting"
                    );
                }
                Err(XdsError::StreamBroken(status)) => {
                    warn!(
                        node = %self.node_id,
                        code = ?status.code(),
                        "Delta xDS ADS stream broken — reconnecting"
                    );
                }
                Err(e) => {
                    error!(node = %self.node_id, error = %e, "Delta xDS ADS fatal error");
                    return Err(e);
                }
            }

            let delay_ms = (BASE_MS * (1u64 << attempt.min(8))).min(CAP_MS);
            attempt = attempt.saturating_add(1);
            info!(
                node = %self.node_id,
                attempt,
                delay_ms,
                "Delta xDS reconnect back-off"
            );
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        }
    }

    /// Open one Delta ADS stream and consume until error or clean close.
    async fn run_delta_stream(
        &self,
        subs: &mut DeltaSubscriptionMap,
        callback: Arc<dyn DeltaXdsCallback>,
    ) -> Result<(), XdsError> {
        // Re-establish channel so DNS changes are picked up on reconnect.
        let channel = Channel::from_shared(self.endpoint.clone())
            .map_err(|e| XdsError::Connection {
                endpoint: self.endpoint.clone(),
                source: Box::new(e),
            })?
            .connect()
            .await
            .map_err(|e| XdsError::Connection {
                endpoint: self.endpoint.clone(),
                source: Box::new(e),
            })?;

        let mut client = AggregatedDiscoveryServiceClient::new(channel);

        let (tx, rx) = mpsc::channel::<DeltaDiscoveryRequest>(64);
        let outbound = ReceiverStream::new(rx);

        let response_stream = client
            .delta_aggregated_resources(Request::new(outbound))
            .await
            .map_err(XdsError::StreamBroken)?
            .into_inner();

        tokio::pin!(response_stream);

        let node = self.build_node();

        // Send initial subscription for all types.
        // Re-send `initial_resource_versions` so the server can skip
        // already-known resources on reconnect.
        for sub in subs.all_mut() {
            sub.subscribed = true;
            let req = DeltaDiscoveryRequest {
                node: Some(node.clone()),
                type_url: sub.type_url.clone(),
                resource_names_subscribe: vec![], // wildcard
                resource_names_unsubscribe: vec![],
                initial_resource_versions: sub.known_versions.clone(),
                response_nonce: String::new(),
                error_detail: None,
            };
            if tx.send(req).await.is_err() {
                return Err(XdsError::StreamBroken(tonic::Status::internal(
                    "outbound channel closed before first request",
                )));
            }
        }

        // ----------------------------------------------------------------
        // Main receive loop
        // ----------------------------------------------------------------
        use tokio_stream::StreamExt as _;
        let idle_dur = Duration::from_secs(crate::ads_client::IDLE_TIMEOUT_SECS);

        loop {
            let maybe_resp =
                timeout(idle_dur, response_stream.next()).await;

            let resp: DeltaDiscoveryResponse = match maybe_resp {
                Err(_elapsed) => {
                    return Err(XdsError::IdleTimeout {
                        secs: crate::ads_client::IDLE_TIMEOUT_SECS,
                    });
                }
                Ok(None) => return Ok(()), // server closed cleanly
                Ok(Some(Err(status))) => return Err(XdsError::StreamBroken(status)),
                Ok(Some(Ok(r))) => r,
            };

            let type_url = resp.type_url.clone();
            let nonce = resp.nonce.clone();

            debug!(
                node = %self.node_id,
                type_url = %type_url,
                nonce = %nonce,
                added = resp.resources.len(),
                removed = resp.removed_resources.len(),
                "Delta xDS DeltaDiscoveryResponse received"
            );

            // Attempt to apply the delta to the cache and invoke callbacks.
            match self.dispatch_delta(&resp, callback.as_ref()).await {
                Ok(()) => {
                    // ACK: advance known_versions.
                    if let Some(sub) = subs.get_mut(&type_url) {
                        sub.apply_ack(&resp);
                    }
                    inc_ack_nack(&type_url, "ack");
                    let ack = self.build_delta_ack(&node, &type_url, &nonce, None);
                    let _ = tx.send(ack).await;
                    debug!(
                        node = %self.node_id,
                        type_url = %type_url,
                        "Delta xDS ACK sent"
                    );
                }
                Err(ref e) => {
                    warn!(
                        node = %self.node_id,
                        type_url = %type_url,
                        error = %e,
                        "Delta xDS NACK — resource cache NOT updated"
                    );
                    inc_ack_nack(&type_url, "nack");
                    let error_status = google_rpc::Status {
                        code: 3, // INVALID_ARGUMENT
                        message: e.to_string(),
                        details: vec![],
                    };
                    let nack_req =
                        self.build_delta_ack(&node, &type_url, &nonce, Some(error_status));
                    let _ = tx.send(nack_req).await;
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    fn build_node(&self) -> Node {
        Node {
            id: self.node_id.clone(),
            cluster: "armageddon".to_string(),
            user_agent_name: "armageddon-xds-delta".to_string(),
            user_agent_version: env!("CARGO_PKG_VERSION").to_string(),
            ..Default::default()
        }
    }

    fn build_delta_ack(
        &self,
        node: &Node,
        type_url: &str,
        nonce: &str,
        error_detail: Option<google_rpc::Status>,
    ) -> DeltaDiscoveryRequest {
        DeltaDiscoveryRequest {
            node: Some(node.clone()),
            type_url: type_url.to_string(),
            resource_names_subscribe: vec![],
            resource_names_unsubscribe: vec![],
            initial_resource_versions: HashMap::new(),
            response_nonce: nonce.to_string(),
            error_detail,
        }
    }

    /// Apply a `DeltaDiscoveryResponse` to the `ResourceCache` and invoke the
    /// appropriate callbacks.
    ///
    /// Returns `Err` on the first decode failure — the ENTIRE response is then
    /// NACK'd, leaving the cache unchanged.
    ///
    /// # Atomicity
    ///
    /// Each resource type's cache update is applied atomically (via the
    /// `ResourceCache` write-lock).  Individual resources within the same
    /// response are applied together.  There is no cross-type atomicity.
    async fn dispatch_delta(
        &self,
        resp: &DeltaDiscoveryResponse,
        callback: &dyn DeltaXdsCallback,
    ) -> Result<(), XdsError> {
        let type_url = resp.type_url.as_str();

        match type_url {
            type_urls::CLUSTER => {
                let mut adds: HashMap<String, Cluster> = HashMap::new();
                for delta_r in &resp.resources {
                    let any = delta_r.resource.as_ref().ok_or_else(|| {
                        XdsError::DecodeFailure {
                            type_url: type_url.to_string(),
                            source: prost::DecodeError::new("missing resource Any payload"),
                        }
                    })?;
                    let cluster = Cluster::decode(any.value.as_ref()).map_err(|e| {
                        XdsError::DecodeFailure {
                            type_url: type_url.to_string(),
                            source: e,
                        }
                    })?;
                    inc_received(type_url, "upsert");
                    callback.on_cluster_upsert(cluster.clone()).await;
                    adds.insert(cluster.name.clone(), cluster);
                }
                for name in &resp.removed_resources {
                    inc_received(type_url, "remove");
                    callback.on_cluster_removed(name.clone()).await;
                }
                // Apply to cache: upsert + remove.
                self.resources.upsert_remove_clusters(adds, &resp.removed_resources);
            }

            type_urls::ENDPOINT => {
                let mut adds: HashMap<String, ClusterLoadAssignment> = HashMap::new();
                for delta_r in &resp.resources {
                    let any = delta_r.resource.as_ref().ok_or_else(|| {
                        XdsError::DecodeFailure {
                            type_url: type_url.to_string(),
                            source: prost::DecodeError::new("missing resource Any payload"),
                        }
                    })?;
                    let cla =
                        ClusterLoadAssignment::decode(any.value.as_ref()).map_err(|e| {
                            XdsError::DecodeFailure {
                                type_url: type_url.to_string(),
                                source: e,
                            }
                        })?;
                    inc_received(type_url, "upsert");
                    callback.on_endpoint_upsert(cla.clone()).await;
                    adds.insert(cla.cluster_name.clone(), cla);
                }
                for name in &resp.removed_resources {
                    inc_received(type_url, "remove");
                    callback.on_endpoint_removed(name.clone()).await;
                }
                self.resources.upsert_remove_endpoints(adds, &resp.removed_resources);
            }

            type_urls::LISTENER => {
                let mut adds: HashMap<String, Listener> = HashMap::new();
                for delta_r in &resp.resources {
                    let any = delta_r.resource.as_ref().ok_or_else(|| {
                        XdsError::DecodeFailure {
                            type_url: type_url.to_string(),
                            source: prost::DecodeError::new("missing resource Any payload"),
                        }
                    })?;
                    let listener = Listener::decode(any.value.as_ref()).map_err(|e| {
                        XdsError::DecodeFailure {
                            type_url: type_url.to_string(),
                            source: e,
                        }
                    })?;
                    inc_received(type_url, "upsert");
                    callback.on_listener_upsert(listener.clone()).await;
                    adds.insert(listener.name.clone(), listener);
                }
                for name in &resp.removed_resources {
                    inc_received(type_url, "remove");
                    callback.on_listener_removed(name.clone()).await;
                }
                self.resources.upsert_remove_listeners(adds, &resp.removed_resources);
            }

            type_urls::ROUTE => {
                let mut adds: HashMap<String, RouteConfiguration> = HashMap::new();
                for delta_r in &resp.resources {
                    let any = delta_r.resource.as_ref().ok_or_else(|| {
                        XdsError::DecodeFailure {
                            type_url: type_url.to_string(),
                            source: prost::DecodeError::new("missing resource Any payload"),
                        }
                    })?;
                    let route = RouteConfiguration::decode(any.value.as_ref()).map_err(|e| {
                        XdsError::DecodeFailure {
                            type_url: type_url.to_string(),
                            source: e,
                        }
                    })?;
                    inc_received(type_url, "upsert");
                    callback.on_route_upsert(route.clone()).await;
                    adds.insert(route.name.clone(), route);
                }
                for name in &resp.removed_resources {
                    inc_received(type_url, "remove");
                    callback.on_route_removed(name.clone()).await;
                }
                self.resources.upsert_remove_routes(adds, &resp.removed_resources);
            }

            type_urls::SECRET => {
                let mut adds: HashMap<String, Secret> = HashMap::new();
                for delta_r in &resp.resources {
                    let any = delta_r.resource.as_ref().ok_or_else(|| {
                        XdsError::DecodeFailure {
                            type_url: type_url.to_string(),
                            source: prost::DecodeError::new("missing resource Any payload"),
                        }
                    })?;
                    let secret = Secret::decode(any.value.as_ref()).map_err(|e| {
                        XdsError::DecodeFailure {
                            type_url: type_url.to_string(),
                            source: e,
                        }
                    })?;
                    inc_received(type_url, "upsert");
                    callback.on_secret_upsert(secret.clone()).await;
                    adds.insert(secret.name.clone(), secret);
                }
                for name in &resp.removed_resources {
                    inc_received(type_url, "remove");
                    callback.on_secret_removed(name.clone()).await;
                }
                self.resources.upsert_remove_secrets(adds, &resp.removed_resources);
            }

            unknown => {
                warn!(type_url = %unknown, "Delta xDS: received unknown type_url, NACKing");
                return Err(XdsError::UnsupportedResourceType(unknown.to_string()));
            }
        }

        Ok(())
    }
}

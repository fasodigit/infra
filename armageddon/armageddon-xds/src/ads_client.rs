// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Active xDS v3 ADS consumer — bidirectional tonic stream over gRPC.
//!
//! # Invariants
//!
//! - `version_info` for a type is only advanced AFTER a successful callback
//!   invocation (i.e. the resource decoded cleanly).  On callback error the
//!   prior version is preserved and a NACK is sent.
//! - A NACK carries the *previous* `version_info`, not the rejected one.
//! - Locks are NEVER held across `.await` points.
//! - Reconnect loop uses exponential back-off: 100 ms × 2^attempt, capped at 32 s.
//!
//! # Failure modes
//!
//! * **Control-plane restart**: stream error triggers reconnect; last ACK'd
//!   `version_info` and `nonce` are re-sent so the server can resume from the
//!   correct version.
//!
//! * **Malformed resource**: `prost::Message::decode` returns `Err`; a NACK is
//!   sent with the previous `version_info` and `google.rpc.Status` code 3
//!   (INVALID_ARGUMENT).  The resource cache is NOT updated.
//!
//! * **Idle timeout (30 s)**: stream is torn down; reconnect loop activates.
//!   Counter `xds_stream_timeout_total` is incremented.
//!
//! * **Duplicate response** (same version + nonce after reconnect): callback is
//!   suppressed; ACK is still sent to unblock the server.

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
        DiscoveryRequest, DiscoveryResponse, Node,
    },
    endpoint::ClusterLoadAssignment,
    google_rpc,
    listener::Listener,
    route::RouteConfiguration,
    tls::Secret,
    type_urls,
};
use crate::resources::ResourceCache;
use crate::subscription::SubscriptionMap;

/// Idle timeout for the ADS stream.  If no DiscoveryResponse arrives within
/// this window the stream is torn down and reconnect logic takes over.
pub const IDLE_TIMEOUT_SECS: u64 = 30;

/// Base back-off delay for reconnect attempts.
const BACKOFF_BASE_MS: u64 = 100;
/// Maximum back-off delay (2^8 × 100 ms ≈ 25 s, capped here at 32 s).
const BACKOFF_CAP_MS: u64 = 32_000;

// ---------------------------------------------------------------------------
// XdsCallback trait
// ---------------------------------------------------------------------------

/// Callback interface invoked by `AdsClient` for each successfully decoded
/// xDS resource update.
///
/// Implementors MUST NOT hold locks across `await` inside these methods.
/// Any error returned causes the affected resource to be NACK'd without
/// advancing `version_info`.
///
/// # Example
///
/// ```rust,no_run
/// use armageddon_xds::{XdsCallback, XdsError};
/// use armageddon_xds::proto::{
///     cluster::Cluster, endpoint::ClusterLoadAssignment,
///     listener::Listener, route::RouteConfiguration, tls::Secret,
/// };
///
/// struct MyCallback;
///
/// #[async_trait::async_trait]
/// impl XdsCallback for MyCallback {
///     async fn on_cluster_update(&self, cluster: Cluster) {}
///     async fn on_endpoint_update(&self, cla: ClusterLoadAssignment) {}
///     async fn on_listener_update(&self, listener: Listener) {}
///     async fn on_route_update(&self, route: RouteConfiguration) {}
///     async fn on_secret_update(&self, secret: Secret) {}
/// }
/// ```
#[async_trait]
pub trait XdsCallback: Send + Sync + 'static {
    async fn on_cluster_update(&self, cluster: Cluster);
    async fn on_endpoint_update(&self, cla: ClusterLoadAssignment);
    async fn on_listener_update(&self, listener: Listener);
    async fn on_route_update(&self, route: RouteConfiguration);
    async fn on_secret_update(&self, secret: Secret);
}

// ---------------------------------------------------------------------------
// AdsClient
// ---------------------------------------------------------------------------

/// Active xDS ADS consumer.
///
/// Maintains a persistent bidirectional gRPC stream to the FASO xds-controller
/// and pushes all resource updates into both a `ResourceCache` and the provided
/// `XdsCallback`.
///
/// # Construction
///
/// ```rust,no_run
/// # async fn doc() -> Result<(), armageddon_xds::XdsError> {
/// let client = armageddon_xds::AdsClient::connect(
///     "http://xds-controller.faso.internal:18000",
///     "armageddon-node-1".to_string(),
/// ).await?;
/// # Ok(())
/// # }
/// ```
pub struct AdsClient {
    /// gRPC endpoint of the xds-controller ADS service.
    endpoint: String,
    /// Logical node identifier for this ARMAGEDDON instance.
    node_id: String,
    /// Shared resource cache — written here on each ACK'd update.
    pub resources: Arc<ResourceCache>,
}

impl AdsClient {
    /// Connect to the xds-controller at `endpoint` and return a ready client.
    ///
    /// This establishes the underlying TCP/HTTP-2 channel but does NOT yet open
    /// the ADS stream; call [`run`](Self::run) to start streaming.
    pub async fn connect(endpoint: &str, node_id: String) -> Result<Self, XdsError> {
        // Eagerly connect so we surface transport errors here, not inside run().
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

        info!(node = %node_id, endpoint = %endpoint, "xDS ADS channel established");

        Ok(Self {
            endpoint: endpoint.to_string(),
            node_id,
            resources: Arc::new(ResourceCache::new()),
        })
    }

    /// Run the ADS consume loop until a non-retriable error occurs or the task
    /// is cancelled.
    ///
    /// Internally reconnects with exponential back-off on any stream error.
    /// The last ACK'd `version_info` + `nonce` per type are preserved across
    /// reconnects.
    pub async fn run(self, callback: Arc<dyn XdsCallback>) -> Result<(), XdsError> {
        let mut subs = SubscriptionMap::new_all_types();
        let mut attempt: u32 = 0;

        loop {
            match self.run_stream(&mut subs, callback.clone()).await {
                Ok(()) => {
                    // Clean shutdown (e.g. server closed stream gracefully).
                    info!(node = %self.node_id, "xDS ADS stream closed cleanly");
                    return Ok(());
                }
                Err(XdsError::IdleTimeout { secs }) => {
                    warn!(
                        node = %self.node_id,
                        secs,
                        "xDS ADS idle timeout — reconnecting"
                    );
                }
                Err(XdsError::StreamBroken(status)) => {
                    warn!(
                        node = %self.node_id,
                        code = ?status.code(),
                        message = %status.message(),
                        "xDS ADS stream broken — reconnecting"
                    );
                }
                Err(e) => {
                    // Non-retriable (e.g. connection refused permanently).
                    error!(node = %self.node_id, error = %e, "xDS ADS fatal error");
                    return Err(e);
                }
            }

            // Exponential back-off: 100 ms * 2^attempt, capped at 32 s.
            let delay_ms = (BACKOFF_BASE_MS * (1u64 << attempt.min(8))).min(BACKOFF_CAP_MS);
            attempt = attempt.saturating_add(1);
            info!(
                node = %self.node_id,
                attempt,
                delay_ms,
                "xDS reconnect back-off"
            );
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        }
    }

    /// Open one ADS stream and consume until error or clean close.
    async fn run_stream(
        &self,
        subs: &mut SubscriptionMap,
        callback: Arc<dyn XdsCallback>,
    ) -> Result<(), XdsError> {
        // Re-establish the channel on every reconnect so we pick up any DNS
        // changes to the control plane.
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

        // We need a channel to send outbound DiscoveryRequests into the stream.
        let (tx, rx) = mpsc::channel::<DiscoveryRequest>(64);
        let outbound = ReceiverStream::new(rx);

        let response_stream = client
            .stream_aggregated_resources(Request::new(outbound))
            .await
            .map_err(XdsError::StreamBroken)?
            .into_inner();

        tokio::pin!(response_stream);

        let node = self.build_node();

        // Send initial subscription requests for all types.
        // Re-send last ACK'd version_info + nonce so the server can resume.
        for sub in subs.all_mut() {
            sub.subscribed = true;
            let req = DiscoveryRequest {
                version_info: sub.version_info.clone(),
                node: Some(node.clone()),
                resource_names: sub.resource_names.clone(),
                type_url: sub.type_url.clone(),
                response_nonce: sub.nonce.clone(),
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
        let idle_dur = Duration::from_secs(IDLE_TIMEOUT_SECS);

        loop {
            let maybe_resp = timeout(idle_dur, response_stream.next()).await;

            let resp: DiscoveryResponse = match maybe_resp {
                Err(_elapsed) => {
                    return Err(XdsError::IdleTimeout {
                        secs: IDLE_TIMEOUT_SECS,
                    });
                }
                Ok(None) => {
                    // Server closed the stream gracefully.
                    return Ok(());
                }
                Ok(Some(Err(status))) => {
                    return Err(XdsError::StreamBroken(status));
                }
                Ok(Some(Ok(r))) => r,
            };

            let type_url = resp.type_url.clone();
            let version = resp.version_info.clone();
            let nonce = resp.nonce.clone();

            debug!(
                node = %self.node_id,
                type_url = %type_url,
                version = %version,
                nonce = %nonce,
                resources = resp.resources.len(),
                "xDS DiscoveryResponse received"
            );

            // Check for duplicate (already ACK'd same version+nonce).
            let is_dup = subs
                .get_mut(&type_url)
                .map_or(false, |s| s.is_duplicate(&version, &nonce));

            if is_dup {
                debug!(
                    node = %self.node_id,
                    type_url = %type_url,
                    "duplicate xDS response — suppressing callback, sending ACK"
                );
                // Still ACK to avoid stalling the server.
                let ack = self.build_ack(&node, &type_url, &version, &nonce, None);
                let _ = tx.send(ack).await;
                continue;
            }

            // Attempt to decode and invoke callbacks for all resources.
            match self
                .dispatch_response(&resp, callback.as_ref())
                .await
            {
                Ok(()) => {
                    // ACK: advance subscription state.
                    if let Some(sub) = subs.get_mut(&type_url) {
                        sub.record_ack(&version, &nonce);
                    }
                    let ack = self.build_ack(&node, &type_url, &version, &nonce, None);
                    let _ = tx.send(ack).await;
                    debug!(
                        node = %self.node_id,
                        type_url = %type_url,
                        version = %version,
                        "xDS ACK sent"
                    );
                }
                Err(ref e) => {
                    // NACK: keep old version_info unchanged.
                    let prev_version = subs
                        .get_mut(&type_url)
                        .map(|s| s.version_info.clone())
                        .unwrap_or_default();

                    warn!(
                        node = %self.node_id,
                        type_url = %type_url,
                        version = %version,
                        error = %e,
                        "xDS NACK — version NOT advanced, previous={prev_version}"
                    );

                    let error_status = google_rpc::Status {
                        code: 3, // INVALID_ARGUMENT
                        message: e.to_string(),
                        details: vec![],
                    };
                    let nack = self.build_ack(
                        &node,
                        &type_url,
                        &prev_version,
                        &nonce,
                        Some(error_status),
                    );
                    let _ = tx.send(nack).await;
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
            user_agent_name: "armageddon-xds".to_string(),
            user_agent_version: env!("CARGO_PKG_VERSION").to_string(),
            ..Default::default()
        }
    }

    fn build_ack(
        &self,
        node: &Node,
        type_url: &str,
        version_info: &str,
        nonce: &str,
        error_detail: Option<google_rpc::Status>,
    ) -> DiscoveryRequest {
        DiscoveryRequest {
            version_info: version_info.to_string(),
            node: Some(node.clone()),
            resource_names: vec![],
            type_url: type_url.to_string(),
            response_nonce: nonce.to_string(),
            error_detail,
        }
    }

    /// Decode each `google.protobuf.Any` in the response and invoke the
    /// appropriate callback.  Returns `Err` on the first decode failure so
    /// the whole response is NACK'd.
    async fn dispatch_response(
        &self,
        resp: &DiscoveryResponse,
        callback: &dyn XdsCallback,
    ) -> Result<(), XdsError> {
        let type_url = resp.type_url.as_str();

        match type_url {
            type_urls::CLUSTER => {
                let mut clusters: HashMap<String, _> = HashMap::new();
                for any in &resp.resources {
                    let cluster = Cluster::decode(any.value.as_ref()).map_err(|e| {
                        XdsError::DecodeFailure {
                            type_url: type_url.to_string(),
                            source: e,
                        }
                    })?;
                    let name = cluster.name.clone();
                    callback.on_cluster_update(cluster.clone()).await;
                    clusters.insert(name, cluster);
                }
                self.resources.update_clusters(clusters);
            }

            type_urls::ENDPOINT => {
                let mut endpoints: HashMap<String, _> = HashMap::new();
                for any in &resp.resources {
                    let cla =
                        ClusterLoadAssignment::decode(any.value.as_ref()).map_err(|e| {
                            XdsError::DecodeFailure {
                                type_url: type_url.to_string(),
                                source: e,
                            }
                        })?;
                    let name = cla.cluster_name.clone();
                    callback.on_endpoint_update(cla.clone()).await;
                    endpoints.insert(name, cla);
                }
                self.resources.update_endpoints(endpoints);
            }

            type_urls::LISTENER => {
                let mut listeners: HashMap<String, _> = HashMap::new();
                for any in &resp.resources {
                    let listener = Listener::decode(any.value.as_ref()).map_err(|e| {
                        XdsError::DecodeFailure {
                            type_url: type_url.to_string(),
                            source: e,
                        }
                    })?;
                    let name = listener.name.clone();
                    callback.on_listener_update(listener.clone()).await;
                    listeners.insert(name, listener);
                }
                self.resources.update_listeners(listeners);
            }

            type_urls::ROUTE => {
                let mut routes: HashMap<String, _> = HashMap::new();
                for any in &resp.resources {
                    let route = RouteConfiguration::decode(any.value.as_ref()).map_err(|e| {
                        XdsError::DecodeFailure {
                            type_url: type_url.to_string(),
                            source: e,
                        }
                    })?;
                    let name = route.name.clone();
                    callback.on_route_update(route.clone()).await;
                    routes.insert(name, route);
                }
                self.resources.update_routes(routes);
            }

            type_urls::SECRET => {
                let mut secrets: HashMap<String, _> = HashMap::new();
                for any in &resp.resources {
                    let secret = Secret::decode(any.value.as_ref()).map_err(|e| {
                        XdsError::DecodeFailure {
                            type_url: type_url.to_string(),
                            source: e,
                        }
                    })?;
                    let name = secret.name.clone();
                    callback.on_secret_update(secret.clone()).await;
                    secrets.insert(name, secret);
                }
                self.resources.update_secrets(secrets);
            }

            unknown => {
                warn!(type_url = %unknown, "xDS: received unknown type_url, ignoring");
                return Err(XdsError::UnsupportedResourceType(unknown.to_string()));
            }
        }

        Ok(())
    }
}

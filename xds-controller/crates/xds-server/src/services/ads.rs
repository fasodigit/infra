// Aggregated Discovery Service (ADS) implementation.
//
// This is the primary entry point for ARMAGEDDON. A single bidirectional
// gRPC stream carries all resource types (CDS, EDS, RDS, LDS, SDS).
//
// Flow:
//   1. ARMAGEDDON opens StreamAggregatedResources
//   2. ARMAGEDDON sends DiscoveryRequest for each type it wants
//   3. xDS Controller sends DiscoveryResponse with current resources
//   4. When ConfigStore changes, Controller pushes new DiscoveryResponse
//   5. ARMAGEDDON ACKs or NACKs each response via response_nonce
//   6. On NACK, Controller can implement instant rollback

use crate::config::ServerConfig;
use crate::convert;
use crate::generated::envoy::service::discovery::v3::{
    aggregated_discovery_service_server::AggregatedDiscoveryService, ControlPlane,
    DeltaDiscoveryRequest, DeltaDiscoveryResponse, DiscoveryRequest, DiscoveryResponse,
};
use crate::subscription::{type_urls, SubscriptionManager};

use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;
use tonic::{Request, Response, Status, Streaming};
use tracing::{debug, info, warn};
use uuid::Uuid;
use xds_store::ConfigStore;

/// ADS service implementation that serves all xDS resource types
/// over a single aggregated stream to ARMAGEDDON.
pub struct AdsService {
    store: ConfigStore,
    subscriptions: SubscriptionManager,
    config: Arc<ServerConfig>,
}

impl AdsService {
    pub fn new(store: ConfigStore, config: Arc<ServerConfig>) -> Self {
        Self {
            store,
            subscriptions: SubscriptionManager::new(),
            config,
        }
    }

}

#[tonic::async_trait]
impl AggregatedDiscoveryService for AdsService {
    type StreamAggregatedResourcesStream =
        Pin<Box<ReceiverStream<Result<DiscoveryResponse, Status>>>>;

    /// Main ADS streaming RPC.
    /// ARMAGEDDON calls this to receive all xDS resources over a single stream.
    async fn stream_aggregated_resources(
        &self,
        request: Request<Streaming<DiscoveryRequest>>,
    ) -> Result<Response<Self::StreamAggregatedResourcesStream>, Status> {
        let remote_addr = request
            .remote_addr()
            .map(|a| a.to_string())
            .unwrap_or_else(|| "unknown".to_string());

        info!(remote = %remote_addr, "ARMAGEDDON connected via ADS stream");

        let (tx, rx) = mpsc::channel(64);
        let mut inbound = request.into_inner();

        // Clone what we need for the spawned task
        let store = self.store.clone();
        let subscriptions = self.subscriptions.clone();
        let config = self.config.clone();

        // Generate a temporary node ID until we get the first request
        let temp_node_id = format!("pending-{}", Uuid::new_v4());
        subscriptions.register_node(&temp_node_id);

        tokio::spawn(async move {
            let mut node_id = temp_node_id.clone();
            let mut change_rx = store.subscribe();

            loop {
                tokio::select! {
                    // Handle inbound DiscoveryRequests from ARMAGEDDON
                    msg = inbound.next() => {
                        match msg {
                            Some(Ok(req)) => {
                                // Extract node ID from first request
                                if let Some(node) = &req.node {
                                    if node_id.starts_with("pending-") {
                                        let real_id = if node.id.is_empty() {
                                            remote_addr.clone()
                                        } else {
                                            node.id.clone()
                                        };
                                        // Re-register with real node ID
                                        subscriptions.unregister_node(&node_id);
                                        node_id = real_id;
                                        subscriptions.register_node(&node_id);
                                        info!(node = %node_id, "identified ARMAGEDDON node");
                                    }
                                }

                                let type_url = &req.type_url;

                                // Handle ACK/NACK
                                if !req.response_nonce.is_empty() {
                                    if req.error_detail.is_some() {
                                        warn!(
                                            node = %node_id,
                                            type_url = %type_url,
                                            nonce = %req.response_nonce,
                                            "NACK received from ARMAGEDDON - rollback may be needed"
                                        );
                                    } else {
                                        subscriptions.record_ack(
                                            &node_id,
                                            type_url,
                                            &req.version_info,
                                            &req.response_nonce,
                                        );
                                    }
                                }

                                // Update subscription
                                subscriptions.update_subscription(
                                    &node_id,
                                    type_url,
                                    req.resource_names.clone(),
                                );

                                // Send current state for this type
                                let version = store.snapshot().version.as_string();
                                let response = build_response_static(
                                    &store, &subscriptions, &config,
                                    type_url, &version, &node_id,
                                );

                                if tx.send(Ok(response)).await.is_err() {
                                    debug!(node = %node_id, "client disconnected during send");
                                    break;
                                }
                            }
                            Some(Err(e)) => {
                                warn!(node = %node_id, error = %e, "stream error from ARMAGEDDON");
                                break;
                            }
                            None => {
                                info!(node = %node_id, "ARMAGEDDON disconnected");
                                break;
                            }
                        }
                    }

                    // Handle configuration changes -> push to ARMAGEDDON
                    Ok(()) = change_rx.changed() => {
                        // Clone the notification out of the borrow to avoid
                        // holding the non-Send watch::Ref across an await.
                        let notification = change_rx.borrow_and_update().clone();

                        if let Some(notification) = notification.as_ref() {
                            let version = notification.version.as_string();

                            // Determine which type_urls to push based on what changed
                            let type_url = match notification.resource_type {
                                xds_store::store::ResourceType::Cluster => type_urls::CLUSTER,
                                xds_store::store::ResourceType::Endpoint => type_urls::ENDPOINT,
                                xds_store::store::ResourceType::Route => type_urls::ROUTE,
                                xds_store::store::ResourceType::Listener => type_urls::LISTENER,
                                xds_store::store::ResourceType::Certificate => type_urls::SECRET,
                            };

                            // Only push if this node is subscribed to this type
                            if subscriptions.get_subscribed_resources(&node_id, type_url).is_some() {
                                let response = build_response_static(
                                    &store, &subscriptions, &config,
                                    type_url, &version, &node_id,
                                );

                                if tx.send(Ok(response)).await.is_err() {
                                    debug!(node = %node_id, "client disconnected during push");
                                    break;
                                }

                                debug!(
                                    node = %node_id,
                                    type_url = %type_url,
                                    version = %version,
                                    "pushed update to ARMAGEDDON"
                                );
                            }
                        }
                    }
                }
            }

            // Clean up on disconnect
            subscriptions.unregister_node(&node_id);
            info!(node = %node_id, "ADS stream closed");
        });

        Ok(Response::new(Box::pin(ReceiverStream::new(rx))))
    }

    type DeltaAggregatedResourcesStream =
        Pin<Box<ReceiverStream<Result<DeltaDiscoveryResponse, Status>>>>;

    /// Delta ADS (incremental) - not yet implemented.
    /// ARMAGEDDON currently uses full-state ADS.
    async fn delta_aggregated_resources(
        &self,
        _request: Request<Streaming<DeltaDiscoveryRequest>>,
    ) -> Result<Response<Self::DeltaAggregatedResourcesStream>, Status> {
        Err(Status::unimplemented(
            "Delta ADS not yet implemented. Use StreamAggregatedResources.",
        ))
    }
}

/// Static helper to build a response (avoids borrowing &self in spawned task).
fn build_response_static(
    store: &ConfigStore,
    subscriptions: &SubscriptionManager,
    config: &ServerConfig,
    type_url: &str,
    version: &str,
    node_id: &str,
) -> DiscoveryResponse {
    let snapshot = store.snapshot();
    let nonce = Uuid::new_v4().to_string();

    let resources = match type_url {
        type_urls::CLUSTER => snapshot
            .clusters
            .values()
            .map(|c| convert::cluster_to_any(c))
            .collect(),

        type_urls::ENDPOINT => {
            let subscribed = subscriptions.get_subscribed_resources(node_id, type_url);
            snapshot
                .endpoints
                .iter()
                .filter(|(name, _)| {
                    subscribed
                        .as_ref()
                        .map_or(true, |s| s.is_empty() || s.contains(*name))
                })
                .map(|(name, eps)| convert::endpoints_to_any(name, eps))
                .collect()
        }

        type_urls::ROUTE => snapshot
            .routes
            .values()
            .map(|r| convert::route_to_any(r))
            .collect(),

        type_urls::LISTENER => snapshot
            .listeners
            .values()
            .map(|l| convert::listener_to_any(l))
            .collect(),

        type_urls::SECRET => snapshot
            .certificates
            .values()
            .map(|c| convert::certificate_to_any(c))
            .collect(),

        _ => vec![],
    };

    subscriptions.record_nonce(node_id, type_url, &nonce);

    DiscoveryResponse {
        version_info: version.to_string(),
        resources,
        canary: false,
        type_url: type_url.to_string(),
        nonce,
        control_plane: Some(ControlPlane {
            identifier: config.control_plane_id.clone(),
        }),
    }
}

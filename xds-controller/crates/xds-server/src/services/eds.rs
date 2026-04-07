// Endpoint Discovery Service (EDS) implementation.
//
// Allows ARMAGEDDON to discover instances (IP:port) per cluster.

use crate::config::ServerConfig;
use crate::convert;
use crate::generated::envoy::service::discovery::v3::{
    ControlPlane, DeltaDiscoveryRequest, DeltaDiscoveryResponse, DiscoveryRequest,
    DiscoveryResponse,
};
use crate::generated::envoy::service::endpoint::v3::endpoint_discovery_service_server::EndpointDiscoveryService;
use crate::subscription::type_urls;

use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status, Streaming};
use tracing::info;
use uuid::Uuid;
use xds_store::ConfigStore;

pub struct EdsService {
    store: ConfigStore,
    config: Arc<ServerConfig>,
}

impl EdsService {
    pub fn new(store: ConfigStore, config: Arc<ServerConfig>) -> Self {
        Self { store, config }
    }
}

#[tonic::async_trait]
impl EndpointDiscoveryService for EdsService {
    type StreamEndpointsStream = Pin<Box<ReceiverStream<Result<DiscoveryResponse, Status>>>>;

    async fn stream_endpoints(
        &self,
        _request: Request<Streaming<DiscoveryRequest>>,
    ) -> Result<Response<Self::StreamEndpointsStream>, Status> {
        let (_tx, rx) = mpsc::channel(16);
        info!("EDS stream opened (standalone mode)");
        Ok(Response::new(Box::pin(ReceiverStream::new(rx))))
    }

    type DeltaEndpointsStream = Pin<Box<ReceiverStream<Result<DeltaDiscoveryResponse, Status>>>>;

    async fn delta_endpoints(
        &self,
        _request: Request<Streaming<DeltaDiscoveryRequest>>,
    ) -> Result<Response<Self::DeltaEndpointsStream>, Status> {
        Err(Status::unimplemented("Delta EDS not implemented"))
    }

    async fn fetch_endpoints(
        &self,
        request: Request<DiscoveryRequest>,
    ) -> Result<Response<DiscoveryResponse>, Status> {
        let req = request.into_inner();
        let snapshot = self.store.snapshot();
        let version = snapshot.version.as_string();
        let nonce = Uuid::new_v4().to_string();

        // If resource_names specified, only return those clusters' endpoints
        let resources: Vec<_> = if req.resource_names.is_empty() {
            snapshot
                .endpoints
                .iter()
                .map(|(name, eps)| convert::endpoints_to_any(name, eps))
                .collect()
        } else {
            req.resource_names
                .iter()
                .filter_map(|name| {
                    snapshot
                        .endpoints
                        .get(name)
                        .map(|eps| convert::endpoints_to_any(name, eps))
                })
                .collect()
        };

        Ok(Response::new(DiscoveryResponse {
            version_info: version,
            resources,
            canary: false,
            type_url: type_urls::ENDPOINT.to_string(),
            nonce,
            control_plane: Some(ControlPlane {
                identifier: self.config.control_plane_id.clone(),
            }),
        }))
    }
}

// Route Discovery Service (RDS) implementation.
//
// Allows ARMAGEDDON to receive dynamic routing rules.

use crate::config::ServerConfig;
use crate::convert;
use crate::generated::envoy::service::discovery::v3::{
    ControlPlane, DeltaDiscoveryRequest, DeltaDiscoveryResponse, DiscoveryRequest,
    DiscoveryResponse,
};
use crate::generated::envoy::service::route::v3::route_discovery_service_server::RouteDiscoveryService;
use crate::subscription::type_urls;

use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status, Streaming};
use tracing::info;
use uuid::Uuid;
use xds_store::ConfigStore;

pub struct RdsService {
    store: ConfigStore,
    config: Arc<ServerConfig>,
}

impl RdsService {
    pub fn new(store: ConfigStore, config: Arc<ServerConfig>) -> Self {
        Self { store, config }
    }
}

#[tonic::async_trait]
impl RouteDiscoveryService for RdsService {
    type StreamRoutesStream = Pin<Box<ReceiverStream<Result<DiscoveryResponse, Status>>>>;

    async fn stream_routes(
        &self,
        _request: Request<Streaming<DiscoveryRequest>>,
    ) -> Result<Response<Self::StreamRoutesStream>, Status> {
        let (_tx, rx) = mpsc::channel(16);
        info!("RDS stream opened (standalone mode)");
        Ok(Response::new(Box::pin(ReceiverStream::new(rx))))
    }

    type DeltaRoutesStream = Pin<Box<ReceiverStream<Result<DeltaDiscoveryResponse, Status>>>>;

    async fn delta_routes(
        &self,
        _request: Request<Streaming<DeltaDiscoveryRequest>>,
    ) -> Result<Response<Self::DeltaRoutesStream>, Status> {
        Err(Status::unimplemented("Delta RDS not implemented"))
    }

    async fn fetch_routes(
        &self,
        _request: Request<DiscoveryRequest>,
    ) -> Result<Response<DiscoveryResponse>, Status> {
        let snapshot = self.store.snapshot();
        let version = snapshot.version.as_string();
        let nonce = Uuid::new_v4().to_string();

        let resources = snapshot
            .routes
            .values()
            .map(|r| convert::route_to_any(r))
            .collect();

        Ok(Response::new(DiscoveryResponse {
            version_info: version,
            resources,
            canary: false,
            type_url: type_urls::ROUTE.to_string(),
            nonce,
            control_plane: Some(ControlPlane {
                identifier: self.config.control_plane_id.clone(),
            }),
        }))
    }
}

// Listener Discovery Service (LDS) implementation.
//
// Allows ARMAGEDDON to receive listener configurations and filter chains.

use crate::config::ServerConfig;
use crate::convert;
use crate::generated::envoy::service::discovery::v3::{
    ControlPlane, DeltaDiscoveryRequest, DeltaDiscoveryResponse, DiscoveryRequest,
    DiscoveryResponse,
};
use crate::generated::envoy::service::listener::v3::listener_discovery_service_server::ListenerDiscoveryService;
use crate::subscription::type_urls;

use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status, Streaming};
use tracing::info;
use uuid::Uuid;
use xds_store::ConfigStore;

pub struct LdsService {
    store: ConfigStore,
    config: Arc<ServerConfig>,
}

impl LdsService {
    pub fn new(store: ConfigStore, config: Arc<ServerConfig>) -> Self {
        Self { store, config }
    }
}

#[tonic::async_trait]
impl ListenerDiscoveryService for LdsService {
    type StreamListenersStream = Pin<Box<ReceiverStream<Result<DiscoveryResponse, Status>>>>;

    async fn stream_listeners(
        &self,
        _request: Request<Streaming<DiscoveryRequest>>,
    ) -> Result<Response<Self::StreamListenersStream>, Status> {
        let (_tx, rx) = mpsc::channel(16);
        info!("LDS stream opened (standalone mode)");
        Ok(Response::new(Box::pin(ReceiverStream::new(rx))))
    }

    type DeltaListenersStream = Pin<Box<ReceiverStream<Result<DeltaDiscoveryResponse, Status>>>>;

    async fn delta_listeners(
        &self,
        _request: Request<Streaming<DeltaDiscoveryRequest>>,
    ) -> Result<Response<Self::DeltaListenersStream>, Status> {
        Err(Status::unimplemented("Delta LDS not implemented"))
    }

    async fn fetch_listeners(
        &self,
        _request: Request<DiscoveryRequest>,
    ) -> Result<Response<DiscoveryResponse>, Status> {
        let snapshot = self.store.snapshot();
        let version = snapshot.version.as_string();
        let nonce = Uuid::new_v4().to_string();

        let resources = snapshot
            .listeners
            .values()
            .map(|l| convert::listener_to_any(l))
            .collect();

        Ok(Response::new(DiscoveryResponse {
            version_info: version,
            resources,
            canary: false,
            type_url: type_urls::LISTENER.to_string(),
            nonce,
            control_plane: Some(ControlPlane {
                identifier: self.config.control_plane_id.clone(),
            }),
        }))
    }
}

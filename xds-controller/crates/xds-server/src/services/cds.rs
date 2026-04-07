// Cluster Discovery Service (CDS) implementation.
//
// Allows ARMAGEDDON to discover backend clusters independently of ADS.
// In practice, ARMAGEDDON uses ADS, but CDS is available for debugging
// and compatibility with other xDS clients.

use crate::config::ServerConfig;
use crate::convert;
use crate::generated::envoy::service::discovery::v3::{
    ControlPlane, DeltaDiscoveryRequest, DeltaDiscoveryResponse, DiscoveryRequest,
    DiscoveryResponse,
};
use crate::generated::envoy::service::cluster::v3::cluster_discovery_service_server::ClusterDiscoveryService;
use crate::subscription::type_urls;

use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status, Streaming};
use tracing::info;
use uuid::Uuid;
use xds_store::ConfigStore;

pub struct CdsService {
    store: ConfigStore,
    config: Arc<ServerConfig>,
}

impl CdsService {
    pub fn new(store: ConfigStore, config: Arc<ServerConfig>) -> Self {
        Self { store, config }
    }
}

#[tonic::async_trait]
impl ClusterDiscoveryService for CdsService {
    type StreamClustersStream = Pin<Box<ReceiverStream<Result<DiscoveryResponse, Status>>>>;

    async fn stream_clusters(
        &self,
        _request: Request<Streaming<DiscoveryRequest>>,
    ) -> Result<Response<Self::StreamClustersStream>, Status> {
        // Delegate to ADS in production; this is a standalone fallback.
        let (_tx, rx) = mpsc::channel(16);
        info!("CDS stream opened (standalone mode)");
        Ok(Response::new(Box::pin(ReceiverStream::new(rx))))
    }

    type DeltaClustersStream = Pin<Box<ReceiverStream<Result<DeltaDiscoveryResponse, Status>>>>;

    async fn delta_clusters(
        &self,
        _request: Request<Streaming<DeltaDiscoveryRequest>>,
    ) -> Result<Response<Self::DeltaClustersStream>, Status> {
        Err(Status::unimplemented("Delta CDS not implemented"))
    }

    async fn fetch_clusters(
        &self,
        _request: Request<DiscoveryRequest>,
    ) -> Result<Response<DiscoveryResponse>, Status> {
        let snapshot = self.store.snapshot();
        let version = snapshot.version.as_string();
        let nonce = Uuid::new_v4().to_string();

        let resources = snapshot
            .clusters
            .values()
            .map(|c| convert::cluster_to_any(c))
            .collect();

        Ok(Response::new(DiscoveryResponse {
            version_info: version,
            resources,
            canary: false,
            type_url: type_urls::CLUSTER.to_string(),
            nonce,
            control_plane: Some(ControlPlane {
                identifier: self.config.control_plane_id.clone(),
            }),
        }))
    }
}

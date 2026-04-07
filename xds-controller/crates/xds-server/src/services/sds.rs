// Secret Discovery Service (SDS) implementation.
//
// Distributes TLS certificates from SPIRE to ARMAGEDDON.
// Certificates are fetched by xds-spire and stored in the ConfigStore.

use crate::config::ServerConfig;
use crate::convert;
use crate::generated::envoy::service::discovery::v3::{
    ControlPlane, DeltaDiscoveryRequest, DeltaDiscoveryResponse, DiscoveryRequest,
    DiscoveryResponse,
};
use crate::generated::envoy::service::secret::v3::secret_discovery_service_server::SecretDiscoveryService;
use crate::subscription::type_urls;

use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status, Streaming};
use tracing::info;
use uuid::Uuid;
use xds_store::ConfigStore;

pub struct SdsService {
    store: ConfigStore,
    config: Arc<ServerConfig>,
}

impl SdsService {
    pub fn new(store: ConfigStore, config: Arc<ServerConfig>) -> Self {
        Self { store, config }
    }
}

#[tonic::async_trait]
impl SecretDiscoveryService for SdsService {
    type StreamSecretsStream = Pin<Box<ReceiverStream<Result<DiscoveryResponse, Status>>>>;

    async fn stream_secrets(
        &self,
        _request: Request<Streaming<DiscoveryRequest>>,
    ) -> Result<Response<Self::StreamSecretsStream>, Status> {
        let (_tx, rx) = mpsc::channel(16);
        info!("SDS stream opened (standalone mode)");
        Ok(Response::new(Box::pin(ReceiverStream::new(rx))))
    }

    type DeltaSecretsStream = Pin<Box<ReceiverStream<Result<DeltaDiscoveryResponse, Status>>>>;

    async fn delta_secrets(
        &self,
        _request: Request<Streaming<DeltaDiscoveryRequest>>,
    ) -> Result<Response<Self::DeltaSecretsStream>, Status> {
        Err(Status::unimplemented("Delta SDS not implemented"))
    }

    async fn fetch_secrets(
        &self,
        request: Request<DiscoveryRequest>,
    ) -> Result<Response<DiscoveryResponse>, Status> {
        let req = request.into_inner();
        let snapshot = self.store.snapshot();
        let version = snapshot.version.as_string();
        let nonce = Uuid::new_v4().to_string();

        // Filter by requested SPIFFE IDs if specified
        let resources: Vec<_> = if req.resource_names.is_empty() {
            snapshot
                .certificates
                .values()
                .map(|c| convert::certificate_to_any(c))
                .collect()
        } else {
            req.resource_names
                .iter()
                .filter_map(|id| {
                    snapshot
                        .certificates
                        .get(id)
                        .map(|c| convert::certificate_to_any(c))
                })
                .collect()
        };

        Ok(Response::new(DiscoveryResponse {
            version_info: version,
            resources,
            canary: false,
            type_url: type_urls::SECRET.to_string(),
            nonce,
            control_plane: Some(ControlPlane {
                identifier: self.config.control_plane_id.clone(),
            }),
        }))
    }
}

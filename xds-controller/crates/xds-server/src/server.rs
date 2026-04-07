// XdsServer: the main entry point that assembles and runs the gRPC server.
//
// Registers all xDS services (ADS, CDS, EDS, RDS, LDS, SDS) on a single
// tonic server listening on port 18000 (configurable).

use crate::config::ServerConfig;
use crate::generated::envoy::service::cluster::v3::cluster_discovery_service_server::ClusterDiscoveryServiceServer;
use crate::generated::envoy::service::discovery::v3::aggregated_discovery_service_server::AggregatedDiscoveryServiceServer;
use crate::generated::envoy::service::endpoint::v3::endpoint_discovery_service_server::EndpointDiscoveryServiceServer;
use crate::generated::envoy::service::listener::v3::listener_discovery_service_server::ListenerDiscoveryServiceServer;
use crate::generated::envoy::service::route::v3::route_discovery_service_server::RouteDiscoveryServiceServer;
use crate::generated::envoy::service::secret::v3::secret_discovery_service_server::SecretDiscoveryServiceServer;
use crate::services::{AdsService, CdsService, EdsService, LdsService, RdsService, SdsService};

use std::net::SocketAddr;
use std::sync::Arc;
use tonic::transport::Server;
use tracing::info;
use xds_store::ConfigStore;

/// The xDS Controller gRPC server.
///
/// Serves all xDS v3 APIs to ARMAGEDDON instances.
pub struct XdsServer {
    config: Arc<ServerConfig>,
    store: ConfigStore,
}

impl XdsServer {
    /// Create a new xDS server with the given configuration and store.
    pub fn new(config: ServerConfig, store: ConfigStore) -> Self {
        Self {
            config: Arc::new(config),
            store,
        }
    }

    /// Run the gRPC server. Blocks until shutdown.
    pub async fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        let addr: SocketAddr = format!("{}:{}", self.config.listen_addr, self.config.listen_port)
            .parse()
            .map_err(|e| format!("invalid listen address: {e}"))?;

        // Create service instances, all sharing the same ConfigStore
        let ads = AdsService::new(self.store.clone(), self.config.clone());
        let cds = CdsService::new(self.store.clone(), self.config.clone());
        let eds = EdsService::new(self.store.clone(), self.config.clone());
        let rds = RdsService::new(self.store.clone(), self.config.clone());
        let lds = LdsService::new(self.store.clone(), self.config.clone());
        let sds = SdsService::new(self.store.clone(), self.config.clone());

        info!(
            address = %addr,
            control_plane = %self.config.control_plane_id,
            "starting xDS Controller gRPC server for ARMAGEDDON"
        );

        Server::builder()
            .add_service(AggregatedDiscoveryServiceServer::new(ads))
            .add_service(ClusterDiscoveryServiceServer::new(cds))
            .add_service(EndpointDiscoveryServiceServer::new(eds))
            .add_service(RouteDiscoveryServiceServer::new(rds))
            .add_service(ListenerDiscoveryServiceServer::new(lds))
            .add_service(SecretDiscoveryServiceServer::new(sds))
            .serve(addr)
            .await?;

        Ok(())
    }
}

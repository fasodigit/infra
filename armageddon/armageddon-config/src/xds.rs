//! xDS client: receives dynamic configuration from the xDS Controller via gRPC ADS.
//!
//! Implements Aggregated Discovery Service (ADS) to receive:
//! - CDS (Cluster Discovery Service)
//! - EDS (Endpoint Discovery Service)
//! - LDS (Listener Discovery Service)
//! - RDS (Route Discovery Service)

use async_trait::async_trait;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum XdsError {
    #[error("xDS connection failed: {0}")]
    Connection(String),

    #[error("xDS stream broken: {0}")]
    StreamBroken(String),

    #[error("invalid xDS response: {0}")]
    InvalidResponse(String),

    #[error("resource type not supported: {0}")]
    UnsupportedResource(String),
}

/// xDS resource types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XdsResourceType {
    /// Cluster Discovery Service.
    Cluster,
    /// Endpoint Discovery Service.
    Endpoint,
    /// Listener Discovery Service.
    Listener,
    /// Route Discovery Service.
    Route,
}

impl XdsResourceType {
    pub fn type_url(&self) -> &'static str {
        match self {
            XdsResourceType::Cluster => "type.googleapis.com/envoy.config.cluster.v3.Cluster",
            XdsResourceType::Endpoint => {
                "type.googleapis.com/envoy.config.endpoint.v3.ClusterLoadAssignment"
            }
            XdsResourceType::Listener => "type.googleapis.com/envoy.config.listener.v3.Listener",
            XdsResourceType::Route => {
                "type.googleapis.com/envoy.config.route.v3.RouteConfiguration"
            }
        }
    }
}

/// Callback for xDS updates.
#[async_trait]
pub trait XdsCallback: Send + Sync + 'static {
    /// Called when clusters are updated.
    async fn on_cluster_update(&self, version: &str, resources: &[Vec<u8>])
        -> Result<(), XdsError>;

    /// Called when endpoints are updated.
    async fn on_endpoint_update(
        &self,
        version: &str,
        resources: &[Vec<u8>],
    ) -> Result<(), XdsError>;

    /// Called when routes are updated.
    async fn on_route_update(&self, version: &str, resources: &[Vec<u8>]) -> Result<(), XdsError>;
}

/// xDS ADS client that connects to the xDS Controller.
pub struct XdsClient {
    address: String,
    port: u16,
    node_id: String,
    cluster_name: String,
}

impl XdsClient {
    pub fn new(address: &str, port: u16, node_id: &str, cluster_name: &str) -> Self {
        Self {
            address: address.to_string(),
            port,
            node_id: node_id.to_string(),
            cluster_name: cluster_name.to_string(),
        }
    }

    /// Start the ADS stream. This runs until cancelled.
    pub async fn run(&self, _callback: impl XdsCallback) -> Result<(), XdsError> {
        tracing::info!(
            "xDS client connecting to {}:{} (node={}, cluster={})",
            self.address,
            self.port,
            self.node_id,
            self.cluster_name
        );
        // TODO: implement gRPC ADS stream via tonic
        Ok(())
    }
}

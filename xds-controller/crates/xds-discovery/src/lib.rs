// xds-discovery: Service discovery backends for the xDS Controller.
//
// Supports multiple discovery sources:
//   - DNS: resolve hostnames to endpoint IPs
//   - Consul: query Consul service catalog
//   - YAML: static file-based endpoint definitions
//
// Each backend implements the ServiceDiscovery trait and feeds
// discovered endpoints into the ConfigStore.

pub mod consul;
pub mod dns;
pub mod error;
pub mod manager;
pub mod yaml;

pub use error::DiscoveryError;
pub use manager::DiscoveryManager;

use async_trait::async_trait;

/// A discovered service endpoint.
#[derive(Debug, Clone)]
pub struct DiscoveredEndpoint {
    pub address: String,
    pub port: u16,
    pub weight: u32,
    pub metadata: std::collections::HashMap<String, String>,
}

/// Trait for service discovery backends.
/// Each backend polls or watches a source and returns endpoints.
#[async_trait]
pub trait ServiceDiscovery: Send + Sync + 'static {
    /// Discover current endpoints for a service.
    async fn discover(&self, service_name: &str) -> Result<Vec<DiscoveredEndpoint>, DiscoveryError>;

    /// Human-readable name of this discovery backend.
    fn backend_name(&self) -> &str;
}

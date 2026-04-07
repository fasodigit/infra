// Consul-based service discovery.
//
// Queries the Consul HTTP API to discover healthy service instances.

use crate::{DiscoveredEndpoint, DiscoveryError, ServiceDiscovery};
use async_trait::async_trait;
use serde::Deserialize;
use tracing::debug;

/// Consul service discovery backend.
pub struct ConsulDiscovery {
    /// Consul HTTP API base URL (e.g. "http://consul.service.consul:8500").
    base_url: String,
}

impl ConsulDiscovery {
    pub fn new(base_url: String) -> Self {
        Self { base_url }
    }
}

#[async_trait]
impl ServiceDiscovery for ConsulDiscovery {
    async fn discover(&self, service_name: &str) -> Result<Vec<DiscoveredEndpoint>, DiscoveryError> {
        debug!(
            service = %service_name,
            backend = "consul",
            url = %self.base_url,
            "querying consul for service"
        );

        // TODO: Implement Consul HTTP API query.
        //
        // Production implementation will:
        //   1. GET /v1/health/service/<name>?passing=true
        //   2. Parse ConsulServiceEntry responses
        //   3. Extract address:port and metadata from each entry
        //   4. Support Consul watches for real-time updates

        Ok(Vec::new())
    }

    fn backend_name(&self) -> &str {
        "consul"
    }
}

/// Consul service health entry (for future HTTP API deserialization).
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct ConsulServiceEntry {
    #[serde(rename = "Service")]
    pub service: ConsulService,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct ConsulService {
    #[serde(rename = "ID")]
    pub id: String,
    #[serde(rename = "Service")]
    pub service: String,
    #[serde(rename = "Address")]
    pub address: String,
    #[serde(rename = "Port")]
    pub port: u16,
    #[serde(rename = "Tags")]
    pub tags: Option<Vec<String>>,
    #[serde(rename = "Meta")]
    pub meta: Option<std::collections::HashMap<String, String>>,
}

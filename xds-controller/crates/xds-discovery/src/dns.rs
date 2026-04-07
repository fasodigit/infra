// DNS-based service discovery.
//
// Resolves hostnames to IP addresses for endpoint discovery.
// Supports SRV records for port discovery.

use crate::{DiscoveredEndpoint, DiscoveryError, ServiceDiscovery};
use async_trait::async_trait;
use std::collections::HashMap;
use tracing::debug;

/// DNS service discovery backend.
pub struct DnsDiscovery {
    /// Default port when SRV records are not available.
    default_port: u16,
}

impl DnsDiscovery {
    pub fn new(default_port: u16) -> Self {
        Self { default_port }
    }
}

#[async_trait]
impl ServiceDiscovery for DnsDiscovery {
    async fn discover(&self, service_name: &str) -> Result<Vec<DiscoveredEndpoint>, DiscoveryError> {
        debug!(service = %service_name, backend = "dns", "resolving service");

        // TODO: Implement actual DNS resolution using tokio::net or trust-dns.
        // For now, return an empty result to allow compilation.
        //
        // Production implementation will:
        //   1. Resolve SRV records for _<service>._tcp.<domain>
        //   2. Fall back to A/AAAA records if no SRV
        //   3. Use resolved IPs as endpoint addresses

        let _host = service_name;
        let _port = self.default_port;

        Ok(Vec::new())
    }

    fn backend_name(&self) -> &str {
        "dns"
    }
}

/// Parse a service name into host and port components.
pub fn parse_service_address(service: &str) -> (String, Option<u16>) {
    if let Some((host, port_str)) = service.rsplit_once(':') {
        if let Ok(port) = port_str.parse::<u16>() {
            return (host.to_string(), Some(port));
        }
    }
    (service.to_string(), None)
}

impl DiscoveredEndpoint {
    /// Create a new endpoint with default weight.
    pub fn new(address: String, port: u16) -> Self {
        Self {
            address,
            port,
            weight: 1,
            metadata: HashMap::new(),
        }
    }
}

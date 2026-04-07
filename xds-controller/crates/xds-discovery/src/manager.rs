// DiscoveryManager: orchestrates service discovery across multiple backends.
//
// Periodically polls all registered discovery backends and updates
// the ConfigStore with discovered endpoints.

use crate::{DiscoveredEndpoint, DiscoveryError, ServiceDiscovery};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::time::{self, Duration};
use tracing::{error, info, warn};
use xds_store::{ConfigStore, EndpointEntry, HealthStatus};

/// Manages service discovery across multiple backends.
pub struct DiscoveryManager {
    /// Registered discovery backends.
    backends: Vec<Arc<dyn ServiceDiscovery>>,

    /// Mapping of cluster name -> (backend_index, service_name).
    service_mappings: HashMap<String, (usize, String)>,

    /// Configuration store to update with discovered endpoints.
    store: ConfigStore,

    /// Poll interval for discovery refresh.
    poll_interval: Duration,
}

impl DiscoveryManager {
    pub fn new(store: ConfigStore, poll_interval: Duration) -> Self {
        Self {
            backends: Vec::new(),
            service_mappings: HashMap::new(),
            store,
            poll_interval,
        }
    }

    /// Register a discovery backend.
    pub fn add_backend(&mut self, backend: Arc<dyn ServiceDiscovery>) -> usize {
        let idx = self.backends.len();
        info!(backend = backend.backend_name(), index = idx, "registered discovery backend");
        self.backends.push(backend);
        idx
    }

    /// Map a cluster to a discovery backend and service name.
    pub fn map_service(
        &mut self,
        cluster_name: String,
        backend_index: usize,
        service_name: String,
    ) {
        info!(
            cluster = %cluster_name,
            backend = backend_index,
            service = %service_name,
            "mapped cluster to discovery service"
        );
        self.service_mappings
            .insert(cluster_name, (backend_index, service_name));
    }

    /// Run the discovery loop. Polls all mapped services at the configured interval.
    pub async fn run(self) -> Result<(), DiscoveryError> {
        info!(
            interval_ms = self.poll_interval.as_millis() as u64,
            mappings = self.service_mappings.len(),
            "starting discovery manager"
        );

        let mut interval = time::interval(self.poll_interval);

        loop {
            interval.tick().await;

            for (cluster_name, (backend_idx, service_name)) in &self.service_mappings {
                let backend = match self.backends.get(*backend_idx) {
                    Some(b) => b,
                    None => {
                        warn!(
                            cluster = %cluster_name,
                            backend_index = backend_idx,
                            "backend index out of range, skipping"
                        );
                        continue;
                    }
                };

                match backend.discover(service_name).await {
                    Ok(discovered) => {
                        let endpoints: Vec<EndpointEntry> = discovered
                            .into_iter()
                            .map(|d| to_endpoint_entry(cluster_name, d))
                            .collect();

                        if let Err(e) = self.store.set_endpoints(cluster_name, endpoints) {
                            warn!(
                                cluster = %cluster_name,
                                error = %e,
                                "failed to update endpoints in store"
                            );
                        }
                    }
                    Err(e) => {
                        error!(
                            cluster = %cluster_name,
                            service = %service_name,
                            backend = backend.backend_name(),
                            error = %e,
                            "discovery failed"
                        );
                    }
                }
            }
        }
    }
}

/// Convert a discovered endpoint into a store EndpointEntry.
fn to_endpoint_entry(cluster_name: &str, ep: DiscoveredEndpoint) -> EndpointEntry {
    EndpointEntry {
        cluster_name: cluster_name.to_string(),
        address: ep.address,
        port: ep.port,
        health_status: HealthStatus::Unknown,
        weight: ep.weight,
        locality: None,
        metadata: ep.metadata,
        last_health_check: None,
        updated_at: chrono::Utc::now(),
    }
}

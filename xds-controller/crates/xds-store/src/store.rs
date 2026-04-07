// ConfigStore: the primary interface for reading and writing xDS configuration.
//
// This is backed by KAYA Collections in production. The current implementation
// uses DashMap (concurrent HashMap) as an in-memory stand-in that mirrors
// the KAYA Collection API patterns.
//
// Production TODO: Replace DashMap with kaya::Collection<T> calls.

use crate::error::StoreError;
use crate::model::*;
use crate::snapshot::{ConfigSnapshot, SnapshotVersion, VersionGenerator};
use chrono::Utc;
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::watch;
use tracing::{debug, info};

/// Notification sent when the configuration changes.
#[derive(Debug, Clone)]
pub struct ChangeNotification {
    pub version: SnapshotVersion,
    pub resource_type: ResourceType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceType {
    Cluster,
    Endpoint,
    Route,
    Listener,
    Certificate,
}

impl std::fmt::Display for ResourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResourceType::Cluster => write!(f, "cluster"),
            ResourceType::Endpoint => write!(f, "endpoint"),
            ResourceType::Route => write!(f, "route"),
            ResourceType::Listener => write!(f, "listener"),
            ResourceType::Certificate => write!(f, "certificate"),
        }
    }
}

/// Main configuration store backed by KAYA Collections.
///
/// Thread-safe: all methods take &self and use interior mutability.
#[derive(Clone)]
pub struct ConfigStore {
    // KAYA COLLECTION: clusters
    clusters: Arc<DashMap<String, ClusterEntry>>,

    // KAYA COLLECTION: endpoints (grouped by cluster)
    endpoints: Arc<DashMap<String, Vec<EndpointEntry>>>,

    // KAYA COLLECTION: routes
    routes: Arc<DashMap<String, RouteEntry>>,

    // KAYA COLLECTION: listeners
    listeners: Arc<DashMap<String, ListenerEntry>>,

    // KAYA COLLECTION: certificates
    certificates: Arc<DashMap<String, CertificateEntry>>,

    // Version generator for snapshots
    version_gen: Arc<VersionGenerator>,

    // Watch channel for change notifications (triggers xDS push to ARMAGEDDON)
    change_tx: Arc<watch::Sender<Option<ChangeNotification>>>,
    change_rx: watch::Receiver<Option<ChangeNotification>>,
}

impl ConfigStore {
    /// Create a new empty ConfigStore.
    pub fn new() -> Self {
        let (change_tx, change_rx) = watch::channel(None);
        Self {
            clusters: Arc::new(DashMap::new()),
            endpoints: Arc::new(DashMap::new()),
            routes: Arc::new(DashMap::new()),
            listeners: Arc::new(DashMap::new()),
            certificates: Arc::new(DashMap::new()),
            version_gen: Arc::new(VersionGenerator::new()),
            change_tx: Arc::new(change_tx),
            change_rx,
        }
    }

    /// Subscribe to configuration change notifications.
    /// Used by xds-server to trigger pushes to ARMAGEDDON.
    pub fn subscribe(&self) -> watch::Receiver<Option<ChangeNotification>> {
        self.change_rx.clone()
    }

    /// Build a consistent snapshot of all current configuration.
    /// This is what gets serialized into xDS DiscoveryResponse resources.
    pub fn snapshot(&self) -> ConfigSnapshot {
        let version = self.version_gen.current();

        let clusters = self
            .clusters
            .iter()
            .map(|r| (r.key().clone(), r.value().clone()))
            .collect();

        let endpoints = self
            .endpoints
            .iter()
            .map(|r| (r.key().clone(), r.value().clone()))
            .collect();

        let routes = self
            .routes
            .iter()
            .map(|r| (r.key().clone(), r.value().clone()))
            .collect();

        let listeners = self
            .listeners
            .iter()
            .map(|r| (r.key().clone(), r.value().clone()))
            .collect();

        let certificates = self
            .certificates
            .iter()
            .map(|r| (r.key().clone(), r.value().clone()))
            .collect();

        ConfigSnapshot {
            version,
            clusters,
            endpoints,
            routes,
            listeners,
            certificates,
        }
    }

    // -----------------------------------------------------------------------
    // Cluster operations (KAYA COLLECTION: clusters)
    // -----------------------------------------------------------------------

    /// Add or update a cluster.
    pub fn set_cluster(&self, mut cluster: ClusterEntry) -> Result<SnapshotVersion, StoreError> {
        cluster.updated_at = Utc::now();
        let name = cluster.name.clone();
        self.clusters.insert(name.clone(), cluster);
        let version = self.notify(ResourceType::Cluster);
        info!(cluster = %name, version = %version, "cluster set");
        Ok(version)
    }

    /// Remove a cluster and its endpoints.
    pub fn remove_cluster(&self, name: &str) -> Result<SnapshotVersion, StoreError> {
        self.clusters
            .remove(name)
            .ok_or_else(|| StoreError::NotFound {
                kind: "cluster".into(),
                name: name.into(),
            })?;
        self.endpoints.remove(name);
        let version = self.notify(ResourceType::Cluster);
        info!(cluster = %name, version = %version, "cluster removed");
        Ok(version)
    }

    /// Get a cluster by name.
    pub fn get_cluster(&self, name: &str) -> Option<ClusterEntry> {
        self.clusters.get(name).map(|r| r.value().clone())
    }

    /// List all clusters.
    pub fn list_clusters(&self) -> Vec<ClusterEntry> {
        self.clusters.iter().map(|r| r.value().clone()).collect()
    }

    // -----------------------------------------------------------------------
    // Endpoint operations (KAYA COLLECTION: endpoints)
    // -----------------------------------------------------------------------

    /// Set endpoints for a cluster (full replacement).
    pub fn set_endpoints(
        &self,
        cluster_name: &str,
        endpoints: Vec<EndpointEntry>,
    ) -> Result<SnapshotVersion, StoreError> {
        if !self.clusters.contains_key(cluster_name) {
            return Err(StoreError::NotFound {
                kind: "cluster".into(),
                name: cluster_name.into(),
            });
        }
        let count = endpoints.len();
        self.endpoints.insert(cluster_name.to_string(), endpoints);
        let version = self.notify(ResourceType::Endpoint);
        debug!(cluster = %cluster_name, count, version = %version, "endpoints set");
        Ok(version)
    }

    /// Get endpoints for a cluster.
    pub fn get_endpoints(&self, cluster_name: &str) -> Vec<EndpointEntry> {
        self.endpoints
            .get(cluster_name)
            .map(|r| r.value().clone())
            .unwrap_or_default()
    }

    /// Get only healthy endpoints (KAYA VIEW: active_endpoints).
    pub fn active_endpoints(&self, cluster_name: &str) -> Vec<EndpointEntry> {
        self.get_endpoints(cluster_name)
            .into_iter()
            .filter(|e| {
                matches!(
                    e.health_status,
                    HealthStatus::Healthy | HealthStatus::Unknown
                )
            })
            .collect()
    }

    // -----------------------------------------------------------------------
    // Route operations (KAYA COLLECTION: routes)
    // -----------------------------------------------------------------------

    /// Add or update a route configuration.
    pub fn set_route(&self, mut route: RouteEntry) -> Result<SnapshotVersion, StoreError> {
        route.updated_at = Utc::now();
        let name = route.name.clone();
        self.routes.insert(name.clone(), route);
        let version = self.notify(ResourceType::Route);
        info!(route = %name, version = %version, "route set");
        Ok(version)
    }

    /// Remove a route configuration.
    pub fn remove_route(&self, name: &str) -> Result<SnapshotVersion, StoreError> {
        self.routes
            .remove(name)
            .ok_or_else(|| StoreError::NotFound {
                kind: "route".into(),
                name: name.into(),
            })?;
        let version = self.notify(ResourceType::Route);
        info!(route = %name, version = %version, "route removed");
        Ok(version)
    }

    /// Get a route configuration by name.
    pub fn get_route(&self, name: &str) -> Option<RouteEntry> {
        self.routes.get(name).map(|r| r.value().clone())
    }

    /// List all route configurations.
    pub fn list_routes(&self) -> Vec<RouteEntry> {
        self.routes.iter().map(|r| r.value().clone()).collect()
    }

    // -----------------------------------------------------------------------
    // Listener operations (KAYA COLLECTION: listeners)
    // -----------------------------------------------------------------------

    /// Add or update a listener.
    pub fn set_listener(
        &self,
        mut listener: ListenerEntry,
    ) -> Result<SnapshotVersion, StoreError> {
        listener.updated_at = Utc::now();
        let name = listener.name.clone();
        self.listeners.insert(name.clone(), listener);
        let version = self.notify(ResourceType::Listener);
        info!(listener = %name, version = %version, "listener set");
        Ok(version)
    }

    /// Remove a listener.
    pub fn remove_listener(&self, name: &str) -> Result<SnapshotVersion, StoreError> {
        self.listeners
            .remove(name)
            .ok_or_else(|| StoreError::NotFound {
                kind: "listener".into(),
                name: name.into(),
            })?;
        let version = self.notify(ResourceType::Listener);
        info!(listener = %name, version = %version, "listener removed");
        Ok(version)
    }

    /// List all listeners.
    pub fn list_listeners(&self) -> Vec<ListenerEntry> {
        self.listeners.iter().map(|r| r.value().clone()).collect()
    }

    // -----------------------------------------------------------------------
    // Certificate operations (KAYA COLLECTION: certificates)
    // -----------------------------------------------------------------------

    /// Add or update a certificate (from SPIRE).
    pub fn set_certificate(
        &self,
        mut cert: CertificateEntry,
    ) -> Result<SnapshotVersion, StoreError> {
        cert.updated_at = Utc::now();
        let id = cert.spiffe_id.clone();
        self.certificates.insert(id.clone(), cert);
        let version = self.notify(ResourceType::Certificate);
        info!(spiffe_id = %id, version = %version, "certificate set");
        Ok(version)
    }

    /// Remove a certificate.
    pub fn remove_certificate(&self, spiffe_id: &str) -> Result<SnapshotVersion, StoreError> {
        self.certificates
            .remove(spiffe_id)
            .ok_or_else(|| StoreError::NotFound {
                kind: "certificate".into(),
                name: spiffe_id.into(),
            })?;
        let version = self.notify(ResourceType::Certificate);
        info!(spiffe_id = %spiffe_id, version = %version, "certificate removed");
        Ok(version)
    }

    /// Get a certificate by SPIFFE ID.
    pub fn get_certificate(&self, spiffe_id: &str) -> Option<CertificateEntry> {
        self.certificates.get(spiffe_id).map(|r| r.value().clone())
    }

    /// List all certificates.
    pub fn list_certificates(&self) -> Vec<CertificateEntry> {
        self.certificates
            .iter()
            .map(|r| r.value().clone())
            .collect()
    }

    // -----------------------------------------------------------------------
    // Internal
    // -----------------------------------------------------------------------

    /// Bump version and notify watchers (triggers xDS push to ARMAGEDDON).
    fn notify(&self, resource_type: ResourceType) -> SnapshotVersion {
        let version = self.version_gen.next();
        let _ = self.change_tx.send(Some(ChangeNotification {
            version,
            resource_type,
        }));
        version
    }
}

impl Default for ConfigStore {
    fn default() -> Self {
        Self::new()
    }
}

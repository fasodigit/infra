// Configuration snapshot: a consistent, versioned view of all xDS resources.
//
// The snapshot is what gets served to ARMAGEDDON via the xDS gRPC stream.
// Each mutation to the store produces a new snapshot version, which triggers
// a push to all connected ARMAGEDDON instances.

use crate::model::{CertificateEntry, ClusterEntry, EndpointEntry, ListenerEntry, RouteEntry};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Monotonically increasing snapshot version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SnapshotVersion(pub u64);

impl SnapshotVersion {
    pub fn as_string(&self) -> String {
        self.0.to_string()
    }
}

impl std::fmt::Display for SnapshotVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A consistent point-in-time view of all xDS resources.
/// Immutable once created -- new mutations produce a new snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSnapshot {
    /// Snapshot version (monotonically increasing).
    pub version: SnapshotVersion,

    /// All clusters keyed by name.
    pub clusters: HashMap<String, ClusterEntry>,

    /// All endpoints keyed by "cluster_name/address:port".
    pub endpoints: HashMap<String, Vec<EndpointEntry>>,

    /// All route configurations keyed by name.
    pub routes: HashMap<String, RouteEntry>,

    /// All listeners keyed by name.
    pub listeners: HashMap<String, ListenerEntry>,

    /// All certificates keyed by SPIFFE ID.
    pub certificates: HashMap<String, CertificateEntry>,
}

impl ConfigSnapshot {
    /// Create an empty snapshot at version 0.
    pub fn empty() -> Self {
        Self {
            version: SnapshotVersion(0),
            clusters: HashMap::new(),
            endpoints: HashMap::new(),
            routes: HashMap::new(),
            listeners: HashMap::new(),
            certificates: HashMap::new(),
        }
    }

    /// Return only healthy endpoints for a cluster.
    /// This corresponds to the KAYA VIEW active_endpoints.
    pub fn active_endpoints(&self, cluster_name: &str) -> Vec<&EndpointEntry> {
        self.endpoints
            .get(cluster_name)
            .map(|eps| {
                eps.iter()
                    .filter(|e| {
                        matches!(
                            e.health_status,
                            crate::model::HealthStatus::Healthy
                                | crate::model::HealthStatus::Unknown
                        )
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// Generates monotonically increasing snapshot versions.
#[derive(Debug)]
pub struct VersionGenerator {
    counter: Arc<AtomicU64>,
}

impl VersionGenerator {
    pub fn new() -> Self {
        Self {
            counter: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Generate the next snapshot version.
    pub fn next(&self) -> SnapshotVersion {
        let v = self.counter.fetch_add(1, Ordering::SeqCst) + 1;
        SnapshotVersion(v)
    }

    /// Get the current version without incrementing.
    pub fn current(&self) -> SnapshotVersion {
        SnapshotVersion(self.counter.load(Ordering::SeqCst))
    }
}

impl Default for VersionGenerator {
    fn default() -> Self {
        Self::new()
    }
}

// xds-store: Configuration state stored in KAYA (typed Collections).
//
// KAYA Collections:
//   - clusters: name, endpoints, health status, weights
//   - routes: path prefix, cluster, headers match
//   - certificates: SPIRE SVID, expiration, rotation
//   - VIEW active_endpoints: materialized view of healthy endpoints
//
// In this scaffold, we use an in-memory implementation that mirrors
// the KAYA collection API. The production version will use the KAYA
// Rust client to persist state.

pub mod error;
pub mod model;
pub mod snapshot;
pub mod store;

pub use error::StoreError;
pub use model::{
    CertificateEntry, ClusterEntry, EndpointEntry, HealthStatus, ListenerEntry, RouteEntry,
};
pub use snapshot::{ConfigSnapshot, SnapshotVersion};
pub use store::ConfigStore;

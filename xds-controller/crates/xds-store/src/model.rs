// Domain models for xDS resources stored in KAYA Collections.
//
// These types are the canonical internal representation. They get
// converted to/from protobuf types in xds-server.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// KAYA COLLECTION: clusters
// ---------------------------------------------------------------------------

/// A backend service cluster that ARMAGEDDON routes traffic to.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterEntry {
    /// Unique cluster name (KAYA primary key).
    pub name: String,

    /// How endpoints are discovered.
    pub discovery_type: DiscoveryType,

    /// Load balancing policy.
    pub lb_policy: LbPolicy,

    /// Connection timeout in milliseconds.
    pub connect_timeout_ms: u64,

    /// Health check configuration.
    pub health_check: Option<HealthCheckConfig>,

    /// Circuit breaker thresholds.
    pub circuit_breaker: Option<CircuitBreakerConfig>,

    /// SPIFFE ID for mTLS (via SPIRE).
    pub spiffe_id: Option<String>,

    /// Arbitrary metadata.
    pub metadata: std::collections::HashMap<String, String>,

    /// Last modification timestamp.
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveryType {
    Static,
    StrictDns,
    LogicalDns,
    Eds,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LbPolicy {
    RoundRobin,
    LeastRequest,
    RingHash,
    Random,
    Maglev,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckConfig {
    pub timeout_ms: u64,
    pub interval_ms: u64,
    pub unhealthy_threshold: u32,
    pub healthy_threshold: u32,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    pub max_connections: u32,
    pub max_pending_requests: u32,
    pub max_requests: u32,
    pub max_retries: u32,
}

// ---------------------------------------------------------------------------
// KAYA COLLECTION: endpoints (part of clusters collection in KAYA)
// ---------------------------------------------------------------------------

/// An individual backend instance (IP:port) within a cluster.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointEntry {
    /// Cluster this endpoint belongs to.
    pub cluster_name: String,

    /// IP address or hostname.
    pub address: String,

    /// Port number.
    pub port: u16,

    /// Health status.
    pub health_status: HealthStatus,

    /// Load balancing weight (higher = more traffic).
    pub weight: u32,

    /// Locality for zone-aware routing.
    pub locality: Option<Locality>,

    /// Arbitrary metadata.
    pub metadata: std::collections::HashMap<String, String>,

    /// Last health check timestamp.
    pub last_health_check: Option<DateTime<Utc>>,

    /// Last modification timestamp.
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    Unknown,
    Healthy,
    Unhealthy,
    Draining,
    Timeout,
    Degraded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Locality {
    pub region: String,
    pub zone: String,
    pub sub_zone: Option<String>,
}

// ---------------------------------------------------------------------------
// KAYA COLLECTION: routes
// ---------------------------------------------------------------------------

/// A routing rule that ARMAGEDDON applies to incoming requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteEntry {
    /// Route configuration name (KAYA primary key).
    pub name: String,

    /// Virtual hosts within this route configuration.
    pub virtual_hosts: Vec<VirtualHostEntry>,

    /// Last modification timestamp.
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VirtualHostEntry {
    /// Virtual host name.
    pub name: String,

    /// Domains to match (e.g. "api.faso.bf").
    pub domains: Vec<String>,

    /// Routes within this virtual host.
    pub routes: Vec<RouteRuleEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteRuleEntry {
    /// Route name for debugging.
    pub name: Option<String>,

    /// Path match (prefix, exact, or regex).
    pub path_match: PathMatch,

    /// Header matchers for fine-grained routing.
    pub header_matchers: Vec<HeaderMatchEntry>,

    /// Target cluster.
    pub cluster: String,

    /// Weighted clusters for traffic splitting.
    pub weighted_clusters: Option<Vec<WeightedClusterEntry>>,

    /// Timeout in milliseconds.
    pub timeout_ms: Option<u64>,

    /// Retry policy.
    pub retry_policy: Option<RetryPolicyEntry>,

    /// Prefix rewrite.
    pub prefix_rewrite: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum PathMatch {
    Prefix(String),
    Exact(String),
    Regex(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeaderMatchEntry {
    pub name: String,
    pub value: String,
    pub match_type: HeaderMatchType,
    pub invert: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeaderMatchType {
    Exact,
    Prefix,
    Suffix,
    Contains,
    Regex,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeightedClusterEntry {
    pub name: String,
    pub weight: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicyEntry {
    pub retry_on: String,
    pub num_retries: u32,
    pub per_try_timeout_ms: Option<u64>,
}

// ---------------------------------------------------------------------------
// KAYA COLLECTION: listeners
// ---------------------------------------------------------------------------

/// A network listener configuration for ARMAGEDDON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListenerEntry {
    /// Unique listener name (KAYA primary key).
    pub name: String,

    /// Bind address.
    pub address: String,

    /// Bind port.
    pub port: u16,

    /// Filter chains.
    pub filter_chains: Vec<FilterChainEntry>,

    /// SPIFFE ID for downstream mTLS.
    pub spiffe_id: Option<String>,

    /// Last modification timestamp.
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterChainEntry {
    /// Filter chain name.
    pub name: Option<String>,

    /// SNI server names to match.
    pub server_names: Vec<String>,

    /// Route configuration name (for HTTP connection manager).
    pub route_config_name: Option<String>,
}

// ---------------------------------------------------------------------------
// KAYA COLLECTION: certificates
// ---------------------------------------------------------------------------

/// A TLS certificate entry managed via SPIRE.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertificateEntry {
    /// SPIFFE ID (KAYA primary key).
    pub spiffe_id: String,

    /// PEM-encoded certificate chain.
    pub certificate_chain: String,

    /// PEM-encoded private key.
    pub private_key: String,

    /// Trusted CA bundle.
    pub trusted_ca: Option<String>,

    /// Certificate expiration.
    pub expires_at: DateTime<Utc>,

    /// When this certificate was last rotated.
    pub rotated_at: DateTime<Utc>,

    /// Last modification timestamp.
    pub updated_at: DateTime<Utc>,
}

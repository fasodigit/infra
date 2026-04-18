//! Gateway (proxy) configuration: listeners, routes, clusters, TLS.

use armageddon_common::types::{AuthMode, Cluster, CorsConfig, JwtConfig, KratosConfig, Route};
use serde::{Deserialize, Serialize};

/// Full gateway configuration replacing Envoy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    /// Listener bindings.
    pub listeners: Vec<ListenerConfig>,

    /// Route table.
    pub routes: Vec<Route>,

    /// Upstream clusters.
    pub clusters: Vec<Cluster>,

    /// Authentication mode (jwt, session, dual).
    #[serde(default = "default_auth_mode")]
    pub auth_mode: AuthMode,

    /// JWT authentication config.
    pub jwt: JwtConfig,

    /// Kratos session validation config.
    #[serde(default)]
    pub kratos: KratosConfig,

    /// CORS per-platform config.
    pub cors: Vec<CorsEntry>,

    /// ext_authz (OPA) configuration.
    pub ext_authz: ExtAuthzConfig,

    /// xDS controller endpoint for dynamic config.
    pub xds: XdsEndpoint,

    /// Webhook configurations.
    #[serde(default)]
    pub webhooks: WebhooksConfig,

    // -- Vague 1 extensions --

    /// HTTP/3 QUIC listener configuration. Omit to disable HTTP/3.
    #[serde(default)]
    pub quic: Option<QuicConfig>,

    /// SPIFFE/SPIRE mTLS mesh configuration. Omit to disable mTLS mesh.
    #[serde(default)]
    pub mesh: Option<MeshConfig>,

    /// Active xDS consumer configuration (extends the static `xds` field).
    /// When set, a live ADS stream is opened toward the control plane.
    #[serde(default)]
    pub xds_consumer: Option<XdsConsumerConfig>,

    /// Load-balancer algorithm selection.
    #[serde(default)]
    pub lb: LbConfig,

    /// Retry / timeout / budget settings.
    #[serde(default)]
    pub retry: RetryConfig,

    /// Response cache backed by KAYA.  Omit to disable caching.
    #[serde(default)]
    pub cache: Option<CacheConfig>,

    /// Admin HTTP API.  Omit to disable (strongly discouraged in production).
    #[serde(default)]
    pub admin: Option<AdminConfig>,

    /// Enable WebSocket + raw TCP L4 proxying.
    #[serde(default)]
    pub websocket_enabled: bool,

    /// Enable gRPC-Web transcoding filter.
    #[serde(default)]
    pub grpc_web_enabled: bool,
}

fn default_auth_mode() -> AuthMode {
    AuthMode::Jwt
}

/// A listener binding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListenerConfig {
    pub name: String,
    pub address: String,
    pub port: u16,
    pub tls: Option<TlsConfig>,
    pub protocol: ListenerProtocol,
}

/// TLS configuration for a listener.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    pub cert_path: String,
    pub key_path: String,
    pub ca_path: Option<String>,
    pub min_version: String,
    pub alpn: Vec<String>,
}

/// Protocol supported by a listener.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ListenerProtocol {
    #[serde(rename = "http")]
    Http,
    #[serde(rename = "https")]
    Https,
    #[serde(rename = "grpc")]
    Grpc,
}

/// Named CORS config for a platform.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorsEntry {
    pub platform: String,
    pub config: CorsConfig,
}

/// ext_authz (OPA sidecar) configuration. Fail-closed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtAuthzConfig {
    pub enabled: bool,
    pub grpc_address: String,
    pub timeout_ms: u64,
    pub fail_closed: bool,
}

impl Default for ExtAuthzConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            grpc_address: "127.0.0.1:9191".to_string(),
            timeout_ms: 500,
            fail_closed: true,
        }
    }
}

/// xDS Controller endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XdsEndpoint {
    pub address: String,
    pub port: u16,
    pub node_id: String,
    pub cluster_name: String,
}

impl Default for XdsEndpoint {
    fn default() -> Self {
        Self {
            address: "xds-controller".to_string(),
            port: 18000,
            node_id: "armageddon-0".to_string(),
            cluster_name: "armageddon".to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Webhook configuration
// ---------------------------------------------------------------------------

/// Top-level webhooks section.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhooksConfig {
    /// GitHub webhook configuration.
    #[serde(default)]
    pub github: GithubWebhookConfig,
}

impl Default for WebhooksConfig {
    fn default() -> Self {
        Self {
            github: GithubWebhookConfig::default(),
        }
    }
}

/// GitHub-specific webhook configuration.
///
/// Serialized under `gateway.webhooks.github` in `armageddon.yaml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubWebhookConfig {
    /// Enable or disable GitHub webhook ingestion.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Environment variable that holds the HMAC-SHA256 secret.
    #[serde(default = "default_github_secret_env")]
    pub secret_env: String,

    /// Redpanda/Kafka broker list.
    #[serde(default = "default_kafka_brokers")]
    pub kafka_brokers: Vec<String>,

    /// Redpanda topic where GitHub events are published.
    #[serde(default = "default_github_topic")]
    pub topic: String,

    /// Maximum requests per source IP per minute before rate-limiting.
    #[serde(default = "default_rate_limit")]
    pub rate_limit_per_ip_per_min: i64,
}

impl Default for GithubWebhookConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            secret_env: default_github_secret_env(),
            kafka_brokers: default_kafka_brokers(),
            topic: default_github_topic(),
            rate_limit_per_ip_per_min: default_rate_limit(),
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_github_secret_env() -> String {
    "ARMAGEDDON_GITHUB_WEBHOOK_SECRET".to_string()
}

fn default_kafka_brokers() -> Vec<String> {
    vec!["redpanda:9092".to_string()]
}

fn default_github_topic() -> String {
    "github.events.v1".to_string()
}

fn default_rate_limit() -> i64 {
    1000
}

// ---------------------------------------------------------------------------
// Vague 1 config structs
// ---------------------------------------------------------------------------

/// HTTP/3 QUIC listener configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuicConfig {
    /// Bind address (default: 0.0.0.0).
    #[serde(default = "default_quic_address")]
    pub address: String,

    /// UDP port for QUIC (default: 4433).
    #[serde(default = "default_quic_port")]
    pub port: u16,

    /// PEM-encoded TLS certificate path.
    pub cert_path: String,

    /// PEM-encoded TLS private key path.
    pub key_path: String,

    /// Maximum concurrent QUIC streams per connection.
    #[serde(default = "default_quic_max_streams")]
    pub max_concurrent_streams: u64,
}

impl Default for QuicConfig {
    fn default() -> Self {
        Self {
            address: default_quic_address(),
            port: default_quic_port(),
            cert_path: "/etc/armageddon/tls/server.crt".to_string(),
            key_path: "/etc/armageddon/tls/server.key".to_string(),
            max_concurrent_streams: default_quic_max_streams(),
        }
    }
}

fn default_quic_address() -> String {
    "0.0.0.0".to_string()
}

fn default_quic_port() -> u16 {
    4433
}

fn default_quic_max_streams() -> u64 {
    100
}

/// SPIFFE/SPIRE mTLS mesh configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeshConfig {
    /// Path to the SPIRE agent workload-API Unix socket.
    #[serde(default = "default_mesh_socket")]
    pub socket_path: String,

    /// PEM-encoded CA trust bundle (inline or from a file path prefix `file:`).
    #[serde(default)]
    pub ca_bundle_pem: Option<String>,

    /// Expected peer SPIFFE ID.
    #[serde(default = "default_mesh_peer_id")]
    pub peer_id: String,
}

impl Default for MeshConfig {
    fn default() -> Self {
        Self {
            socket_path: default_mesh_socket(),
            ca_bundle_pem: None,
            peer_id: default_mesh_peer_id(),
        }
    }
}

fn default_mesh_socket() -> String {
    "/run/spire/sockets/agent.sock".to_string()
}

fn default_mesh_peer_id() -> String {
    "spiffe://faso.gov.bf/ns/default/sa/armageddon".to_string()
}

/// Active xDS ADS consumer configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct XdsConsumerConfig {
    /// gRPC endpoint of the xds-controller.
    #[serde(default = "default_xds_endpoint")]
    pub endpoint: String,

    /// Logical node identifier for this ARMAGEDDON instance.
    #[serde(default = "default_xds_node_id")]
    pub node_id: String,
}

impl Default for XdsConsumerConfig {
    fn default() -> Self {
        Self {
            endpoint: default_xds_endpoint(),
            node_id: default_xds_node_id(),
        }
    }
}

fn default_xds_endpoint() -> String {
    "http://xds-controller:18000".to_string()
}

fn default_xds_node_id() -> String {
    "armageddon-default".to_string()
}

/// Load balancer algorithm selection.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LbConfig {
    /// Algorithm name: round_robin | least_conn | p2c | ring_hash | maglev | weighted | random.
    #[serde(default = "default_lb_algorithm")]
    pub algorithm: String,
}

impl Default for LbConfig {
    fn default() -> Self {
        Self {
            algorithm: default_lb_algorithm(),
        }
    }
}

fn default_lb_algorithm() -> String {
    "round_robin".to_string()
}

/// Retry / timeout / budget settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetryConfig {
    /// Maximum number of retry attempts (not counting the original).
    #[serde(default = "default_retry_max")]
    pub max_retries: u32,

    /// Per-try timeout in milliseconds.
    #[serde(default = "default_retry_per_try_ms")]
    pub per_try_timeout_ms: u64,

    /// Overall timeout across all attempts in milliseconds.
    #[serde(default = "default_retry_overall_ms")]
    pub overall_timeout_ms: u64,

    /// Budget: maximum fraction of active requests that may be retries (0–100).
    #[serde(default = "default_retry_budget_percent")]
    pub budget_percent: f32,

    /// Minimum retry concurrency regardless of active-request count.
    #[serde(default = "default_retry_min_concurrency")]
    pub min_concurrency: u32,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: default_retry_max(),
            per_try_timeout_ms: default_retry_per_try_ms(),
            overall_timeout_ms: default_retry_overall_ms(),
            budget_percent: default_retry_budget_percent(),
            min_concurrency: default_retry_min_concurrency(),
        }
    }
}

fn default_retry_max() -> u32 {
    2
}

fn default_retry_per_try_ms() -> u64 {
    15_000
}

fn default_retry_overall_ms() -> u64 {
    45_000
}

fn default_retry_budget_percent() -> f32 {
    20.0
}

fn default_retry_min_concurrency() -> u32 {
    10
}

/// Response cache configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CacheConfig {
    /// Whether caching is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Default TTL in seconds when no `max-age` directive is present.
    #[serde(default = "default_cache_ttl")]
    pub default_ttl_secs: u64,

    /// Maximum response body size (bytes) to cache.
    #[serde(default = "default_cache_max_body")]
    pub max_body_size: usize,

    /// KAYA key prefix for all cache entries.
    #[serde(default = "default_cache_prefix")]
    pub kaya_prefix: String,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_ttl_secs: default_cache_ttl(),
            max_body_size: default_cache_max_body(),
            kaya_prefix: default_cache_prefix(),
        }
    }
}

fn default_cache_ttl() -> u64 {
    60
}

fn default_cache_max_body() -> usize {
    1_048_576
}

fn default_cache_prefix() -> String {
    "armageddon:resp:".to_string()
}

/// Admin API configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdminConfig {
    /// Enable the admin API.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Bind address (strongly recommended: 127.0.0.1).
    #[serde(default = "default_admin_addr")]
    pub bind_addr: String,

    /// TCP port.
    #[serde(default = "default_admin_port")]
    pub port: u16,

    /// Optional constant-time admin token for `X-Admin-Token` auth.
    #[serde(default)]
    pub admin_token: Option<String>,
}

impl Default for AdminConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            bind_addr: default_admin_addr(),
            port: default_admin_port(),
            admin_token: None,
        }
    }
}

fn default_admin_addr() -> String {
    "127.0.0.1".to_string()
}

fn default_admin_port() -> u16 {
    9901
}

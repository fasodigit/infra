//! Gateway (proxy) configuration: listeners, routes, clusters, TLS.
//!
//! ## Shadow mode sink configuration (`gateway.shadow_mode.sink`)
//!
//! Controls where divergence events detected during shadow-mode parity
//! validation are persisted.  Supported backends:
//!
//! | `type` | Backend |
//! |--------|---------|
//! | `"redpanda"` | Produce JSON to `armageddon.shadow.diffs.v1` |
//! | `"sqlite"` | Write to a local SQLite file (dev / no-broker fallback) |
//! | `"multi"` | Fan-out to Redpanda **and** SQLite simultaneously |
//! | `"noop"` | Discard all events (unit tests / CI) |
//!
//! Example `armageddon.yaml` snippet:
//!
//! ```yaml
//! gateway:
//!   shadow_mode:
//!     enabled: true
//!     sample_rate: 0.01
//!     sink:
//!       type: "redpanda"
//!       redpanda:
//!         brokers: ["redpanda:9092"]
//!         topic: "armageddon.shadow.diffs.v1"
//!       sqlite:
//!         path: "/tmp/armageddon-shadow-diffs.db"
//!         max_rows: 10000
//! ```

use armageddon_common::types::{AdminApiConfig, AuthMode, Cluster, CorsConfig, JwtConfig, KratosConfig, RateLimitConfig, Route};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Runtime selector
// ---------------------------------------------------------------------------

/// Selects the active proxy runtime at startup.
///
/// | Value    | Behaviour |
/// |----------|-----------|
/// | `hyper`  | Legacy hyper 1.x backend (`ForgeServer`). Deprecated since v2.0. |
/// | `pingora`| Pingora-based gateway (`PingoraGateway`). **Default since v2.0.** |
/// | `shadow` | Both backends run concurrently; hyper serves the client, Pingora responses are compared asynchronously via `ShadowSampler`. |
///
/// Recommended migration path: `hyper` → `shadow` (48h validation) → `pingora`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum GatewayRuntime {
    /// Legacy hyper 1.x backend.  Deprecated; will be removed in v3.0.
    Hyper,
    /// Pingora-based gateway. Default since ARMAGEDDON v2.0.
    #[default]
    Pingora,
    /// Shadow mode: hyper is primary, Pingora runs as shadow for parity validation.
    Shadow,
}

impl GatewayRuntime {
    /// Returns `true` when Pingora needs to be booted (pingora or shadow mode).
    pub fn needs_pingora(&self) -> bool {
        matches!(self, Self::Pingora | Self::Shadow)
    }

    /// Returns `true` when the hyper backend needs to be booted.
    pub fn needs_hyper(&self) -> bool {
        matches!(self, Self::Hyper | Self::Shadow)
    }
}

/// Full gateway configuration replacing Envoy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    /// Proxy runtime backend selector.
    ///
    /// - `"pingora"` (default) — Pingora-based proxy.
    /// - `"hyper"` — legacy hyper 1.x path (deprecated, removed in v3.0).
    /// - `"shadow"` — both run in parallel; hyper is primary, Pingora is shadow.
    ///
    /// Omit to use `"pingora"` (default since v2.0).
    #[serde(default)]
    pub runtime: GatewayRuntime,

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

    /// Envoy-style admin API (stats, clusters, config_dump, health, logging).
    /// Loopback-only by default on port 9099. Omit to disable.
    #[serde(default)]
    pub admin_api: Option<AdminApiConfig>,

    /// Enable WebSocket + raw TCP L4 proxying.
    #[serde(default)]
    pub websocket_enabled: bool,

    /// Enable gRPC-Web transcoding filter.
    #[serde(default)]
    pub grpc_web_enabled: bool,

    /// Rate limiting configuration.  Omit (or set `enabled: false`) to
    /// disable rate limiting entirely — zero overhead on the hot path.
    #[serde(default)]
    pub rate_limit: Option<RateLimitConfig>,

    /// Shadow mode sampling + sink configuration.
    ///
    /// Relevant when `runtime = "shadow"`.  Controls which fraction of requests
    /// are mirrored to Pingora and where divergence events are persisted.
    #[serde(default)]
    pub shadow_mode: ShadowModeExtConfig,
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
///
/// This struct is the gateway-config facade for `SpiffeConfig` in
/// `armageddon-common`.  It is intentionally kept thin so that the
/// `armageddon-mesh` crate drives the actual lifecycle.
///
/// Relationship to `SpiffeConfig`:
///   `MeshConfig` → converted to `SpiffeConfig` by the startup code in
///   `armageddon` (the binary) before being passed to `Mesh::new`.
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
    ///
    /// Kept for single-peer backwards compat.  When `authorized_ids` is
    /// non-empty it takes precedence and this field is ignored.
    #[serde(default = "default_mesh_peer_id")]
    pub peer_id: String,

    /// Exhaustive whitelist of SPIFFE IDs allowed to connect to this workload.
    ///
    /// Each entry must be a full URI, e.g.
    /// `spiffe://faso.gov.bf/ns/default/sa/kaya`.  An empty list falls back
    /// to accepting only `peer_id`.
    #[serde(default)]
    pub authorized_ids: Vec<String>,

    /// SPIFFE trust domain (without `spiffe://` prefix).  Used to validate
    /// that every incoming peer cert URI SAN belongs to our trust domain
    /// before matching against `authorized_ids`.
    #[serde(default = "default_mesh_trust_domain")]
    pub trust_domain: String,
}

impl Default for MeshConfig {
    fn default() -> Self {
        Self {
            socket_path: default_mesh_socket(),
            ca_bundle_pem: None,
            peer_id: default_mesh_peer_id(),
            authorized_ids: Vec::new(),
            trust_domain: default_mesh_trust_domain(),
        }
    }
}

fn default_mesh_socket() -> String {
    "/run/spire/sockets/agent.sock".to_string()
}

fn default_mesh_peer_id() -> String {
    "spiffe://faso.gov.bf/ns/default/sa/armageddon".to_string()
}

fn default_mesh_trust_domain() -> String {
    "faso.gov.bf".to_string()
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

// ---------------------------------------------------------------------------
// Shadow mode sink configuration
// ---------------------------------------------------------------------------

/// Selects the diff-event persistence backend for shadow mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ShadowSinkType {
    /// Produce diff events as JSON to a Redpanda topic.
    Redpanda,
    /// Write diff events to a local SQLite file (fallback / dev).
    Sqlite,
    /// Fan-out to both Redpanda and SQLite simultaneously.
    Multi,
    /// Discard all events (unit tests / CI — zero overhead).
    #[default]
    Noop,
}

/// Redpanda-specific sink configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedpandaSinkConfig {
    /// Kafka bootstrap brokers (internal Docker/container address).
    #[serde(default = "default_shadow_brokers")]
    pub brokers: Vec<String>,

    /// Redpanda topic for diff events.
    ///
    /// Partitioned by `tenant_id` (or `request_id` when absent) for
    /// per-tenant ordering.  Retention is controlled by Redpanda topic config
    /// (recommended: 7 days / 10 GiB).
    #[serde(default = "default_shadow_topic")]
    pub topic: String,
}

impl Default for RedpandaSinkConfig {
    fn default() -> Self {
        Self {
            brokers: default_shadow_brokers(),
            topic: default_shadow_topic(),
        }
    }
}

fn default_shadow_brokers() -> Vec<String> {
    vec!["redpanda:9092".to_string()]
}

fn default_shadow_topic() -> String {
    "armageddon.shadow.diffs.v1".to_string()
}

/// SQLite-specific sink configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqliteSinkConfig {
    /// Absolute path to the SQLite database file.
    #[serde(default = "default_sqlite_path")]
    pub path: String,

    /// Maximum number of rows to retain.  Older rows are deleted on insert via
    /// an `id NOT IN (SELECT id … LIMIT max_rows)` delete.
    #[serde(default = "default_sqlite_max_rows")]
    pub max_rows: usize,
}

impl Default for SqliteSinkConfig {
    fn default() -> Self {
        Self {
            path: default_sqlite_path(),
            max_rows: default_sqlite_max_rows(),
        }
    }
}

fn default_sqlite_path() -> String {
    "/tmp/armageddon-shadow-diffs.db".to_string()
}

fn default_sqlite_max_rows() -> usize {
    10_000
}

/// Top-level sink configuration block (`gateway.shadow_mode.sink`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowSinkConfig {
    /// Which backend to use.
    #[serde(rename = "type", default)]
    pub sink_type: ShadowSinkType,

    /// Redpanda backend settings.  Relevant when `type = "redpanda"` or `"multi"`.
    #[serde(default)]
    pub redpanda: RedpandaSinkConfig,

    /// SQLite backend settings.  Relevant when `type = "sqlite"` or `"multi"`.
    #[serde(default)]
    pub sqlite: SqliteSinkConfig,

    /// Bounded channel capacity between the request path and the sink background
    /// task.  Events are dropped (not retried) when the channel is full.
    #[serde(default = "default_sink_channel_capacity")]
    pub channel_capacity: usize,
}

impl Default for ShadowSinkConfig {
    fn default() -> Self {
        Self {
            sink_type: ShadowSinkType::Noop,
            redpanda: RedpandaSinkConfig::default(),
            sqlite: SqliteSinkConfig::default(),
            channel_capacity: default_sink_channel_capacity(),
        }
    }
}

fn default_sink_channel_capacity() -> usize {
    10_000
}

// ---------------------------------------------------------------------------
// Shadow gate sub-configuration
// ---------------------------------------------------------------------------

/// Action taken when the shadow divergence gate trips.
///
/// ```yaml
/// shadow_mode:
///   gate:
///     action: pause   # pause | drop_sample | alert_only
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ShadowGateActionConfig {
    /// Set sample_rate to 0 (fully disable shadow mode).
    #[default]
    Pause,
    /// Halve the current sample_rate on each trip.
    DropSample,
    /// Emit metrics/logs only; do not change the rate.
    AlertOnly,
}

/// Auto-pause gate configuration under `gateway.shadow_mode.gate`.
///
/// The gate evaluates `diverged_total / total` in a sliding window.  When
/// the rate exceeds `max_divergence_rate` and the window has at least
/// `min_samples_before_gate` requests, the configured `action` is triggered.
///
/// Example:
///
/// ```yaml
/// shadow_mode:
///   enabled: true
///   sample_rate: 0.01
///   gate:
///     enabled: true
///     max_divergence_rate: 0.02
///     min_samples_before_gate: 100
///     window_secs: 60
///     action: pause
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowGateExtConfig {
    /// Enable the gate background task.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Maximum fraction of requests that may diverge before the gate trips
    /// (0.0 – 1.0, default 0.02 = 2%).
    #[serde(default = "default_gate_max_divergence_rate")]
    pub max_divergence_rate: f64,

    /// Minimum total requests in the window before the gate is active.
    /// Prevents false positives during cold start.
    #[serde(default = "default_gate_min_samples")]
    pub min_samples_before_gate: u64,

    /// Evaluation window in seconds (default 60).
    #[serde(default = "default_gate_window_secs")]
    pub window_secs: u64,

    /// What to do when the gate trips.
    #[serde(default)]
    pub action: ShadowGateActionConfig,
}

impl Default for ShadowGateExtConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_divergence_rate: default_gate_max_divergence_rate(),
            min_samples_before_gate: default_gate_min_samples(),
            window_secs: default_gate_window_secs(),
            action: ShadowGateActionConfig::default(),
        }
    }
}

fn default_gate_max_divergence_rate() -> f64 {
    0.02
}

fn default_gate_min_samples() -> u64 {
    100
}

fn default_gate_window_secs() -> u64 {
    60
}

// ---------------------------------------------------------------------------
// Shadow mode top-level configuration
// ---------------------------------------------------------------------------

/// Shadow mode configuration block (`gateway.shadow_mode`).
///
/// Used when `gateway.runtime = "shadow"` to control the sampling rate,
/// diff-event persistence, and automatic divergence gate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowModeExtConfig {
    /// Enable shadow mode globally.  When `false`, no requests are mirrored
    /// regardless of `sample_rate`.
    #[serde(default)]
    pub enabled: bool,

    /// Fraction of requests to mirror (0.0 – 1.0, default 0.01 = 1%).
    ///
    /// Stored as a float for config ergonomics; converted to an integer
    /// percentage (0–100) when passed to [`ShadowSampler`].
    #[serde(default = "default_shadow_sample_rate")]
    pub sample_rate: f64,

    /// Diff-event persistence backend.
    #[serde(default)]
    pub sink: ShadowSinkConfig,

    /// Automatic divergence gate.  When enabled, a background task monitors
    /// the divergence rate and auto-pauses (or halves) the sample rate when
    /// it exceeds `gate.max_divergence_rate`.
    #[serde(default)]
    pub gate: ShadowGateExtConfig,
}

impl Default for ShadowModeExtConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            sample_rate: default_shadow_sample_rate(),
            sink: ShadowSinkConfig::default(),
            gate: ShadowGateExtConfig::default(),
        }
    }
}

fn default_shadow_sample_rate() -> f64 {
    0.01
}

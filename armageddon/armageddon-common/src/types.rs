//! Shared types used across all engines.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::IpAddr;

/// Unique per-request identifier.
pub type RequestId = uuid::Uuid;

/// Representation of an HTTP request for engine inspection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpRequest {
    pub method: String,
    pub uri: String,
    pub path: String,
    pub query: Option<String>,
    pub headers: HashMap<String, String>,
    pub body: Option<Vec<u8>>,
    pub version: HttpVersion,
}

/// Representation of an HTTP response for engine inspection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: Option<Vec<u8>>,
}

/// HTTP version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HttpVersion {
    Http10,
    Http11,
    Http2,
    Http3,
}

/// Protocol type for connections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Protocol {
    Http,
    Grpc,
    WebSocket,
    ConnectRpc,
    GraphQL,
}

/// Upstream cluster definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cluster {
    pub name: String,
    pub endpoints: Vec<Endpoint>,
    pub health_check: HealthCheckConfig,
    pub circuit_breaker: CircuitBreakerConfig,
    pub outlier_detection: OutlierDetectionConfig,
}

/// Single upstream endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Endpoint {
    pub address: String,
    pub port: u16,
    pub weight: u32,
    pub healthy: bool,
}

/// Health check configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckConfig {
    pub interval_ms: u64,
    pub timeout_ms: u64,
    pub unhealthy_threshold: u32,
    pub healthy_threshold: u32,
    pub protocol: Protocol,
    pub path: Option<String>,
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            interval_ms: 5000,
            timeout_ms: 2000,
            unhealthy_threshold: 3,
            healthy_threshold: 2,
            protocol: Protocol::Http,
            path: Some("/healthz".to_string()),
        }
    }
}

/// Circuit breaker configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    pub max_connections: u32,
    pub max_pending_requests: u32,
    pub max_requests: u32,
    pub max_retries: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            max_connections: 1024,
            max_pending_requests: 1024,
            max_requests: 1024,
            max_retries: 3,
        }
    }
}

/// Outlier detection configuration (Envoy-compatible passive ejection).
///
/// # Failure modes
///
/// - When too many backends are unhealthy, `max_ejection_percent` caps the
///   ejected set so at least one host remains in the rotation.
/// - After `base_ejection_time_ms * consecutive_ejections` the host re-enters
///   the pool for a probe; on immediate failure it is re-ejected with doubled
///   timeout up to `max_ejection_time_ms`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutlierDetectionConfig {
    /// Consecutive 5xx responses before ejecting a host (default: 5).
    pub consecutive_5xx: u32,
    /// Consecutive gateway failures (TCP reset / refused) before ejecting (default: 5).
    #[serde(default = "OutlierDetectionConfig::default_consec_gw")]
    pub consecutive_gateway_failure: u32,
    /// How often the outlier detector scans all hosts (ms).
    pub interval_ms: u64,
    /// Base ejection duration (ms).  Multiplied by number of consecutive ejections.
    pub base_ejection_time_ms: u64,
    /// Hard cap on ejection duration (ms).
    #[serde(default = "OutlierDetectionConfig::default_max_ejection_time")]
    pub max_ejection_time_ms: u64,
    /// Maximum percentage of hosts that may be ejected simultaneously.
    pub max_ejection_percent: u32,
    /// Enable success-rate based ejection (sliding window).
    #[serde(default)]
    pub success_rate_enabled: bool,
    /// Minimum number of hosts required to compute average success rate.
    #[serde(default = "OutlierDetectionConfig::default_sr_min_hosts")]
    pub success_rate_minimum_hosts: u32,
    /// Minimum request volume per host before success-rate check is valid.
    #[serde(default = "OutlierDetectionConfig::default_sr_request_volume")]
    pub success_rate_request_volume: u32,
    /// Ejection threshold stdev factor (default 1.9, matching Envoy).
    #[serde(default = "OutlierDetectionConfig::default_sr_stdev_factor")]
    pub success_rate_stdev_factor: f64,
}

impl OutlierDetectionConfig {
    fn default_consec_gw() -> u32 { 5 }
    fn default_max_ejection_time() -> u64 { 300_000 }
    fn default_sr_min_hosts() -> u32 { 5 }
    fn default_sr_request_volume() -> u32 { 100 }
    fn default_sr_stdev_factor() -> f64 { 1.9 }
}

impl Default for OutlierDetectionConfig {
    fn default() -> Self {
        Self {
            consecutive_5xx: 5,
            consecutive_gateway_failure: 5,
            interval_ms: 10_000,
            base_ejection_time_ms: 30_000,
            max_ejection_time_ms: 300_000,
            max_ejection_percent: 50,
            success_rate_enabled: false,
            success_rate_minimum_hosts: 5,
            success_rate_request_volume: 100,
            success_rate_stdev_factor: 1.9,
        }
    }
}

/// Config-driven retry policy for serialisation in `armageddon-config`.
///
/// Mirrors `armageddon_retry::RetryPolicy` with primitive types so it can be
/// loaded from YAML/JSON without pulling in the retry crate as a dep.
/// The proxy layer converts this into a live `RetryPolicy` at startup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicyConfig {
    /// Maximum number of retry attempts (not counting the original call).
    pub max_retries: u32,
    /// Per-attempt timeout in milliseconds.
    pub per_try_timeout_ms: u64,
    /// Overall request deadline in milliseconds (original + all retries).
    pub overall_timeout_ms: u64,
    /// HTTP status codes that are retryable (e.g. `[502, 503, 504]`).
    pub retry_on_status: Vec<u16>,
    /// Whether to retry on upstream connection errors.
    pub retry_on_connect_error: bool,
    /// Whether to retry on per-try timeout.
    pub retry_on_timeout: bool,
    /// Initial backoff in milliseconds.
    pub backoff_base_ms: u64,
    /// Backoff cap in milliseconds.
    pub backoff_max_ms: u64,
    /// Jitter mode: `"none"`, `"full"`, or `"equal"`.
    pub jitter: String,
    /// If true, launch a hedged request against a different host after
    /// `per_try_timeout_ms` on the primary attempt.
    pub hedge_on_per_try_timeout: bool,
    /// Retry budget: maximum ratio of retries to active requests (0.0–1.0).
    pub budget_ratio: f32,
    /// Minimum concurrent retries allowed even under low load.
    pub budget_min_retry_concurrency: u32,
}

impl Default for RetryPolicyConfig {
    fn default() -> Self {
        Self {
            max_retries: 2,
            per_try_timeout_ms: 15_000,
            overall_timeout_ms: 45_000,
            retry_on_status: vec![502, 503, 504],
            retry_on_connect_error: true,
            retry_on_timeout: true,
            backoff_base_ms: 25,
            backoff_max_ms: 2_000,
            jitter: "full".to_string(),
            hedge_on_per_try_timeout: false,
            budget_ratio: 0.20,
            budget_min_retry_concurrency: 10,
        }
    }
}

/// Route definition for the proxy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Route {
    pub name: String,
    pub match_rule: RouteMatch,
    pub cluster: String,
    pub timeout_ms: u64,
    pub retry_policy: Option<RetryPolicy>,
    /// Skip authentication for this route (e.g. health checks, public endpoints).
    #[serde(default)]
    pub auth_skip: bool,
}

/// Route matching rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteMatch {
    pub prefix: Option<String>,
    pub path: Option<String>,
    pub regex: Option<String>,
    pub headers: HashMap<String, String>,
    pub methods: Vec<String>,
}

/// Retry policy for a route.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub num_retries: u32,
    pub retry_on: Vec<String>,
    pub per_try_timeout_ms: u64,
}

/// Connection metadata including TLS info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    pub client_ip: IpAddr,
    pub client_port: u16,
    pub server_ip: IpAddr,
    pub server_port: u16,
    pub tls: Option<TlsInfo>,
    pub ja3_fingerprint: Option<String>,
    #[serde(default)]
    pub ja4_fingerprint: Option<String>,
}

/// TLS connection information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsInfo {
    pub version: String,
    pub cipher_suite: String,
    pub sni: Option<String>,
    pub client_cert_subject: Option<String>,
}

/// CORS configuration for a platform/origin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorsConfig {
    pub allowed_origins: Vec<String>,
    pub allowed_methods: Vec<String>,
    pub allowed_headers: Vec<String>,
    pub exposed_headers: Vec<String>,
    pub max_age_secs: u64,
    pub allow_credentials: bool,
}

/// JWT validation configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtConfig {
    pub jwks_uri: String,
    pub issuer: String,
    pub audiences: Vec<String>,
    pub algorithm: String,
    pub cache_ttl_secs: u64,
    pub require_claims: Vec<String>,
}

impl Default for JwtConfig {
    fn default() -> Self {
        Self {
            jwks_uri: "http://auth-ms:8080/.well-known/jwks.json".to_string(),
            issuer: "auth-ms".to_string(),
            audiences: vec!["faso-api".to_string()],
            algorithm: "ES384".to_string(),
            cache_ttl_secs: 300,
            require_claims: vec!["sub".to_string(), "iat".to_string(), "exp".to_string()],
        }
    }
}

/// Kratos session validation configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KratosConfig {
    /// URL of the Kratos /sessions/whoami endpoint.
    pub whoami_url: String,
    /// Name of the session cookie (e.g. "ory_kratos_session").
    pub session_cookie: String,
    /// HTTP request timeout in milliseconds.
    pub timeout_ms: u64,
    /// Cache TTL for validated sessions in seconds.
    pub cache_ttl_secs: u64,
}

impl Default for KratosConfig {
    fn default() -> Self {
        Self {
            whoami_url: "http://kratos:4433/sessions/whoami".to_string(),
            session_cookie: "ory_kratos_session".to_string(),
            timeout_ms: 3000,
            cache_ttl_secs: 60,
        }
    }
}

// ── Rate limiting ─────────────────────────────────────────────────────────────

/// Operating mode for the rate limit filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RateLimitMode {
    /// Per-instance token bucket only — no shared state.
    Local,
    /// Global sliding-window counter via KAYA — shared across instances.
    Global,
    /// Local first, then global (recommended for production).
    Hybrid,
}

impl Default for RateLimitMode {
    fn default() -> Self {
        Self::Local
    }
}

/// What to do when the KAYA backend is unreachable (global / hybrid modes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RateLimitFallback {
    /// Allow the request — prefer availability over enforcement.
    FailOpen,
    /// Deny the request — prefer safety over availability.
    FailClosed,
}

impl Default for RateLimitFallback {
    fn default() -> Self {
        Self::FailOpen
    }
}

/// Rate limit rule for a single descriptor dimension.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitRule {
    /// Descriptor this rule applies to (e.g. `"tenant:acme"`, `"route:/api/v1"`).
    pub descriptor: String,
    /// Maximum requests per window.
    pub requests_per_window: u64,
    /// Window duration in seconds.
    pub window_secs: u64,
    /// Token bucket burst size (local mode).  Defaults to `requests_per_window`.
    #[serde(default)]
    pub burst: Option<u64>,
}

/// Top-level rate limiting configuration.
///
/// Placed in `armageddon-common` so it can be referenced by `armageddon-config`
/// and consumed by `armageddon-ratelimit` without a circular dependency.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Whether rate limiting is enabled.
    #[serde(default = "RateLimitConfig::default_enabled")]
    pub enabled: bool,

    /// Operating mode.
    #[serde(default)]
    pub mode: RateLimitMode,

    /// What to do when KAYA is unavailable (global / hybrid modes).
    #[serde(default)]
    pub fallback: RateLimitFallback,

    /// If `true`, over-limit requests are forwarded anyway (dry-run / canary).
    #[serde(default)]
    pub shadow: bool,

    /// Per-descriptor rules.
    #[serde(default)]
    pub rules: Vec<RateLimitRule>,
}

impl RateLimitConfig {
    fn default_enabled() -> bool {
        false
    }
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: RateLimitMode::Local,
            fallback: RateLimitFallback::FailOpen,
            shadow: false,
            rules: Vec::new(),
        }
    }
}

// ── Authentication ────────────────────────────────────────────────────────────

/// Authentication mode for the gateway.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthMode {
    /// JWT ES384 validation (production).
    #[serde(rename = "jwt")]
    Jwt,
    /// Kratos session cookie validation (development).
    #[serde(rename = "session")]
    Session,
    /// Try JWT first, fallback to session cookie.
    #[serde(rename = "dual")]
    Dual,
}

/// Configuration for the ARMAGEDDON admin API (`armageddon-admin-api`).
///
/// Exposes Envoy-style admin endpoints (`/stats`, `/clusters`,
/// `/config_dump`, `/runtime`, `/server_info`, `/listeners`, `/health`,
/// `/logging`) on a dedicated port.
///
/// Bound to `127.0.0.1:9903` by default. Non-loopback binds REQUIRE a
/// bearer token configured via the env var referenced by `token_env_var`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminApiConfig {
    /// Whether the admin API is enabled.
    #[serde(default = "AdminApiConfig::default_enabled")]
    pub enabled: bool,

    /// Bind address (loopback by default for safety).
    /// Accepts forms like `127.0.0.1:9903` or `[::1]:9099`.
    #[serde(default = "AdminApiConfig::default_bind_addr")]
    pub bind_addr: String,

    /// Name of the environment variable carrying the bearer token.
    #[serde(default = "AdminApiConfig::default_token_env_var")]
    pub token_env_var: String,

    /// CORS origins allowed to access the admin API (defaults to `[]`).
    #[serde(default)]
    pub cors_allowed_origins: Vec<String>,
}

impl AdminApiConfig {
    fn default_enabled() -> bool {
        true
    }

    fn default_bind_addr() -> String {
        "127.0.0.1:9903".to_string()
    }

    fn default_token_env_var() -> String {
        "ARMAGEDDON_ADMIN_TOKEN".to_string()
    }
}

impl Default for AdminApiConfig {
    fn default() -> Self {
        Self {
            enabled: Self::default_enabled(),
            bind_addr: Self::default_bind_addr(),
            token_env_var: Self::default_token_env_var(),
            cors_allowed_origins: Vec::new(),
        }
    }
}

// ── SPIFFE / SPIRE mTLS ───────────────────────────────────────────────────────

/// SPIFFE/SPIRE workload-identity configuration.
///
/// When `enabled = true` the mesh layer will:
///   1. Connect to the SPIRE workload-API at `socket_path`.
///   2. Fetch the initial X.509-SVID and start watching the rotation stream.
///   3. Build hot-swappable `rustls::ServerConfig` / `ClientConfig`.
///   4. Validate every peer certificate URI SAN against `authorized_ids`.
///
/// When `enabled = false` (the default) the mTLS mesh is inactive and the
/// caller falls back to whatever static-TLS or bearer-token path it already
/// uses.  This allows zero-downtime progressive roll-out.
///
/// # Failure modes
///
/// - Socket unreachable at startup → `MeshError::Spiffe`; the process should
///   retry with exponential back-off.
/// - SVID expired while socket is disconnected → existing sessions drain;
///   new handshakes are rejected until SPIRE delivers a fresh SVID.
/// - Peer not in `authorized_ids` → connection dropped before any data is
///   exchanged; logged at WARN level with both peer and expected IDs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpiffeConfig {
    /// Enable the SPIFFE mTLS mesh.  Default: `false` (opt-in).
    #[serde(default = "SpiffeConfig::default_enabled")]
    pub enabled: bool,

    /// Path to the SPIRE workload-API Unix domain socket.
    ///
    /// Consumed by the Rust side (`armageddon-mesh`) and by the Java
    /// `spiffe-grpc` library via the `SPIFFE_ENDPOINT_SOCKET` env var
    /// convention.
    ///
    /// Default: `/run/spire/sockets/agent.sock`
    #[serde(default = "SpiffeConfig::default_socket_path")]
    pub socket_path: String,

    /// SPIFFE trust domain (without the `spiffe://` scheme prefix).
    ///
    /// Example: `"faso.gov.bf"`.  All SVID URI SANs that do not start with
    /// `spiffe://<trust_domain>/` are unconditionally rejected even if their
    /// full URI appears in `authorized_ids`.
    #[serde(default = "SpiffeConfig::default_trust_domain")]
    pub trust_domain: String,

    /// Exhaustive list of SPIFFE IDs (full URI) that this workload is
    /// allowed to accept connections from.
    ///
    /// Example:
    /// ```yaml
    /// authorized_ids:
    ///   - spiffe://faso.gov.bf/ns/default/sa/kaya
    ///   - spiffe://faso.gov.bf/ns/default/sa/armageddon
    /// ```
    ///
    /// An empty list means **no peer is accepted** (fail-closed).  Wildcards
    /// are intentionally not supported — list every authorised peer
    /// explicitly.
    #[serde(default)]
    pub authorized_ids: Vec<String>,
}

impl SpiffeConfig {
    fn default_enabled() -> bool {
        false
    }

    fn default_socket_path() -> String {
        "/run/spire/sockets/agent.sock".to_string()
    }

    fn default_trust_domain() -> String {
        "faso.gov.bf".to_string()
    }
}

impl Default for SpiffeConfig {
    fn default() -> Self {
        Self {
            enabled: Self::default_enabled(),
            socket_path: Self::default_socket_path(),
            trust_domain: Self::default_trust_domain(),
            authorized_ids: Vec::new(),
        }
    }
}

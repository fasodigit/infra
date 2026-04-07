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

/// Outlier detection configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutlierDetectionConfig {
    pub consecutive_5xx: u32,
    pub interval_ms: u64,
    pub base_ejection_time_ms: u64,
    pub max_ejection_percent: u32,
}

impl Default for OutlierDetectionConfig {
    fn default() -> Self {
        Self {
            consecutive_5xx: 5,
            interval_ms: 10_000,
            base_ejection_time_ms: 30_000,
            max_ejection_percent: 50,
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

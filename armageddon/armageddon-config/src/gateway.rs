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

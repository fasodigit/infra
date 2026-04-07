//! Gateway (proxy) configuration: listeners, routes, clusters, TLS.

use armageddon_common::types::{Cluster, CorsConfig, JwtConfig, Route};
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

    /// JWT authentication config.
    pub jwt: JwtConfig,

    /// CORS per-platform config.
    pub cors: Vec<CorsEntry>,

    /// ext_authz (OPA) configuration.
    pub ext_authz: ExtAuthzConfig,

    /// xDS controller endpoint for dynamic config.
    pub xds: XdsEndpoint,
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

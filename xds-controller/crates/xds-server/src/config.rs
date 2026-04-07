// Server configuration for the xDS Controller.

use serde::Deserialize;

/// Configuration for the xDS gRPC server.
#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    /// gRPC listen address (default: "0.0.0.0").
    #[serde(default = "default_listen_addr")]
    pub listen_addr: String,

    /// gRPC listen port (default: 18000).
    #[serde(default = "default_listen_port")]
    pub listen_port: u16,

    /// Control plane identifier sent in DiscoveryResponse.
    #[serde(default = "default_control_plane_id")]
    pub control_plane_id: String,

    /// Maximum number of concurrent ARMAGEDDON connections.
    #[serde(default = "default_max_connections")]
    pub max_connections: usize,

    /// SPIRE agent socket path (for SDS).
    #[serde(default = "default_spire_socket")]
    pub spire_socket_path: String,

    /// Discovery poll interval in seconds.
    #[serde(default = "default_discovery_interval")]
    pub discovery_interval_secs: u64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            listen_addr: default_listen_addr(),
            listen_port: default_listen_port(),
            control_plane_id: default_control_plane_id(),
            max_connections: default_max_connections(),
            spire_socket_path: default_spire_socket(),
            discovery_interval_secs: default_discovery_interval(),
        }
    }
}

fn default_listen_addr() -> String {
    "0.0.0.0".to_string()
}
fn default_listen_port() -> u16 {
    18000
}
fn default_control_plane_id() -> String {
    "faso-xds-controller".to_string()
}
fn default_max_connections() -> usize {
    1024
}
fn default_spire_socket() -> String {
    "/run/spire/sockets/agent.sock".to_string()
}
fn default_discovery_interval() -> u64 {
    30
}

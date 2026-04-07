// Discovery error types.

#[derive(Debug, thiserror::Error)]
pub enum DiscoveryError {
    #[error("DNS resolution failed for {host}: {reason}")]
    DnsResolution { host: String, reason: String },

    #[error("Consul query failed: {0}")]
    ConsulQuery(String),

    #[error("YAML file error: {path}: {reason}")]
    YamlFile { path: String, reason: String },

    #[error("service not found: {0}")]
    ServiceNotFound(String),

    #[error("discovery backend error: {0}")]
    Internal(String),
}

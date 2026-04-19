//! armageddon-config: Configuration management and xDS client for ARMAGEDDON.
//!
//! Loads static config from YAML and receives dynamic updates via gRPC ADS
//! from the xDS Controller.

pub mod gateway;
pub mod loader;
pub mod security;
pub mod xds;

pub use gateway::GatewayConfig;
pub use loader::ConfigLoader;
pub use security::SecurityConfig;

use serde::{Deserialize, Serialize};

/// Top-level ARMAGEDDON configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArmageddonConfig {
    /// Gateway / proxy configuration (listeners, routes, clusters).
    pub gateway: GatewayConfig,

    /// Security engines configuration.
    pub security: SecurityConfig,

    /// KAYA (cache) connection.
    pub kaya: KayaConfig,

    /// Observability settings.
    pub observability: ObservabilityConfig,
}

/// KAYA connection configuration (RESP3+).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KayaConfig {
    pub host: String,
    pub port: u16,
    pub password: Option<String>,
    pub db: u8,
    pub pool_size: u32,
    pub tls: bool,
}

impl Default for KayaConfig {
    fn default() -> Self {
        Self {
            host: "kaya".to_string(),
            port: 6379,
            password: None,
            db: 0,
            pool_size: 16,
            tls: false,
        }
    }
}

/// Observability configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservabilityConfig {
    pub log_level: String,
    pub log_format: LogFormat,
    pub metrics_port: u16,
    pub traces_endpoint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogFormat {
    #[serde(rename = "json")]
    Json,
    #[serde(rename = "text")]
    Text,
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            log_level: "info".to_string(),
            log_format: LogFormat::Json,
            metrics_port: 9090,
            traces_endpoint: None,
        }
    }
}

/// OpenTelemetry tracing configuration.
///
/// Mapped from the `otel` section of `armageddon.yaml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub struct OtelConfig {
    /// OTLP gRPC endpoint. Use `"stdout"` for debug output.
    ///
    /// Default: `"http://otel-collector:4317"`
    #[serde(default = "default_otel_endpoint")]
    pub endpoint: String,

    /// Service name attached to every span as a resource attribute.
    ///
    /// Default: `"armageddon"`
    #[serde(default = "default_otel_service_name")]
    pub service_name: String,

    /// Fraction of traces to sample (0.0–1.0).
    ///
    /// Default: `0.01` (1%)
    #[serde(default = "default_otel_sampling_rate")]
    pub sampling_rate: f64,

    /// Extra resource attributes attached to every span.
    #[serde(default)]
    pub resource_attributes: std::collections::HashMap<String, String>,
}

fn default_otel_endpoint() -> String {
    "http://otel-collector:4317".to_string()
}

fn default_otel_service_name() -> String {
    "armageddon".to_string()
}

fn default_otel_sampling_rate() -> f64 {
    0.01
}

impl Default for OtelConfig {
    fn default() -> Self {
        Self {
            endpoint: default_otel_endpoint(),
            service_name: default_otel_service_name(),
            sampling_rate: default_otel_sampling_rate(),
            resource_attributes: Default::default(),
        }
    }
}

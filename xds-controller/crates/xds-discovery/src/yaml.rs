// YAML file-based service discovery.
//
// Reads static endpoint definitions from YAML files.
// Useful for development and bootstrapping.

use crate::{DiscoveredEndpoint, DiscoveryError, ServiceDiscovery};
use async_trait::async_trait;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::debug;

/// YAML file service discovery backend.
pub struct YamlDiscovery {
    /// Path to the YAML discovery configuration file.
    config_path: PathBuf,
}

impl YamlDiscovery {
    pub fn new(config_path: impl Into<PathBuf>) -> Self {
        Self {
            config_path: config_path.into(),
        }
    }
}

#[async_trait]
impl ServiceDiscovery for YamlDiscovery {
    async fn discover(&self, service_name: &str) -> Result<Vec<DiscoveredEndpoint>, DiscoveryError> {
        debug!(
            service = %service_name,
            backend = "yaml",
            path = %self.config_path.display(),
            "reading yaml discovery file"
        );

        let content = tokio::fs::read_to_string(&self.config_path)
            .await
            .map_err(|e| DiscoveryError::YamlFile {
                path: self.config_path.display().to_string(),
                reason: e.to_string(),
            })?;

        let config: YamlDiscoveryConfig =
            serde_yaml::from_str(&content).map_err(|e| DiscoveryError::YamlFile {
                path: self.config_path.display().to_string(),
                reason: e.to_string(),
            })?;

        let service = config.services.get(service_name).ok_or_else(|| {
            DiscoveryError::ServiceNotFound(service_name.to_string())
        })?;

        let endpoints = service
            .endpoints
            .iter()
            .map(|ep| DiscoveredEndpoint {
                address: ep.address.clone(),
                port: ep.port,
                weight: ep.weight.unwrap_or(1),
                metadata: ep.metadata.clone().unwrap_or_default(),
            })
            .collect();

        Ok(endpoints)
    }

    fn backend_name(&self) -> &str {
        "yaml"
    }
}

/// YAML discovery configuration file format.
#[derive(Debug, Deserialize)]
pub struct YamlDiscoveryConfig {
    pub services: HashMap<String, YamlServiceConfig>,
}

#[derive(Debug, Deserialize)]
pub struct YamlServiceConfig {
    pub endpoints: Vec<YamlEndpointConfig>,
}

#[derive(Debug, Deserialize)]
pub struct YamlEndpointConfig {
    pub address: String,
    pub port: u16,
    pub weight: Option<u32>,
    pub metadata: Option<HashMap<String, String>>,
}

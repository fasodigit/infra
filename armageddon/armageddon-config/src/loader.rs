//! Configuration loader: reads YAML config files and provides hot-reload.

use crate::ArmageddonConfig;
use arc_swap::ArcSwap;
use std::sync::Arc;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("failed to read config file: {0}")]
    ReadFile(#[from] std::io::Error),

    #[error("failed to parse config: {0}")]
    Parse(#[from] serde_yaml::Error),

    #[error("validation failed: {0}")]
    Validation(String),
}

/// Manages configuration lifecycle: load, validate, hot-reload.
pub struct ConfigLoader {
    config: Arc<ArcSwap<ArmageddonConfig>>,
    config_path: String,
}

impl ConfigLoader {
    /// Load configuration from a YAML file.
    pub fn from_file(path: &str) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)?;
        let config: ArmageddonConfig = serde_yaml::from_str(&content)?;

        Ok(Self {
            config: Arc::new(ArcSwap::from_pointee(config)),
            config_path: path.to_string(),
        })
    }

    /// Get a snapshot of the current configuration.
    pub fn get(&self) -> Arc<ArmageddonConfig> {
        self.config.load_full()
    }

    /// Reload configuration from disk.
    pub fn reload(&self) -> Result<(), ConfigError> {
        let content = std::fs::read_to_string(&self.config_path)?;
        let config: ArmageddonConfig = serde_yaml::from_str(&content)?;
        self.config.store(Arc::new(config));
        tracing::info!("configuration reloaded from {}", self.config_path);
        Ok(())
    }

    /// Get the underlying ArcSwap for subscribers that need reactive updates.
    pub fn shared(&self) -> Arc<ArcSwap<ArmageddonConfig>> {
        Arc::clone(&self.config)
    }
}

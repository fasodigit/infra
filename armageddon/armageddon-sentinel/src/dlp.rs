//! Data Loss Prevention: detect sensitive data in requests/responses.

use armageddon_config::security::DlpConfig;
use regex::Regex;
use serde::{Deserialize, Serialize};

/// A DLP pattern to match against.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DlpPattern {
    pub name: String,
    pub description: String,
    #[serde(skip)]
    pub compiled: Option<Regex>,
    pub pattern: String,
}

/// DLP scanning engine.
pub struct DlpEngine {
    config: DlpConfig,
    patterns: Vec<DlpPattern>,
}

impl DlpEngine {
    pub fn new(config: &DlpConfig) -> Self {
        Self {
            config: config.clone(),
            patterns: Vec::new(),
        }
    }

    /// Load DLP patterns from configuration.
    pub fn load_patterns(&mut self) -> std::io::Result<usize> {
        // TODO: load patterns from config.patterns_path
        // Default patterns: credit cards, SSNs, API keys, etc.
        tracing::info!("loading DLP patterns from {}", self.config.patterns_path);
        Ok(self.patterns.len())
    }

    /// Scan a payload for sensitive data.
    pub fn scan(&self, payload: &[u8]) -> Vec<&DlpPattern> {
        let _ = payload;
        // TODO: match against compiled regex patterns
        Vec::new()
    }
}

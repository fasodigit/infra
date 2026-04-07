//! Threat intelligence feed management.

use dashmap::DashMap;
use std::sync::Arc;

/// Manages threat intelligence feeds and provides lookups.
pub struct ThreatIntelManager {
    feeds: Vec<String>,
    refresh_interval_secs: u64,
    /// Known malicious IPs from all feeds.
    known_threats: Arc<DashMap<String, ThreatEntry>>,
}

/// A threat intelligence entry.
#[derive(Debug, Clone)]
pub struct ThreatEntry {
    pub ip: String,
    pub source: String,
    pub category: ThreatCategory,
    pub confidence: f64,
}

/// Threat categories.
#[derive(Debug, Clone)]
pub enum ThreatCategory {
    Botnet,
    Malware,
    Phishing,
    Scanner,
    Spam,
    Tor,
    Vpn,
    Proxy,
    Unknown,
}

impl ThreatIntelManager {
    pub fn new(feeds: &[String], refresh_interval_secs: u64) -> Self {
        Self {
            feeds: feeds.to_vec(),
            refresh_interval_secs,
            known_threats: Arc::new(DashMap::new()),
        }
    }

    /// Check if an IP is a known threat.
    pub fn is_known_threat(&self, ip: &str) -> bool {
        self.known_threats.contains_key(ip)
    }

    /// Get threat details for an IP.
    pub fn get_threat(&self, ip: &str) -> Option<ThreatEntry> {
        self.known_threats.get(ip).map(|entry| entry.value().clone())
    }

    /// Refresh threat intelligence from all feeds.
    pub async fn refresh(&self) {
        for feed in &self.feeds {
            tracing::info!("refreshing threat intel from {}", feed);
            // TODO: HTTP fetch feed, parse entries, update known_threats
        }
    }

    /// Start the periodic refresh loop.
    pub async fn run(&self) {
        tracing::info!(
            "threat intel manager started ({} feeds, refresh every {}s)",
            self.feeds.len(),
            self.refresh_interval_secs,
        );
        // TODO: tokio::time::interval loop calling self.refresh()
    }
}

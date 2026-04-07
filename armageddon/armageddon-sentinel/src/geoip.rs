//! GeoIP engine using MaxMind database.

use armageddon_common::context::GeoInfo;
use std::net::IpAddr;

/// GeoIP lookup engine backed by MaxMind.
pub struct GeoIpEngine {
    db_path: String,
    blocked_countries: Vec<String>,
    // reader: Option<maxminddb::Reader<Vec<u8>>>,
}

impl GeoIpEngine {
    pub fn new(db_path: &str, blocked_countries: &[String]) -> Self {
        Self {
            db_path: db_path.to_string(),
            blocked_countries: blocked_countries.to_vec(),
        }
    }

    /// Initialize the MaxMind database reader.
    pub fn init(&mut self) -> std::io::Result<()> {
        tracing::info!("loading GeoIP database from {}", self.db_path);
        // TODO: self.reader = Some(maxminddb::Reader::open_readfile(&self.db_path)?);
        Ok(())
    }

    /// Look up GeoIP information for an IP address.
    pub fn lookup(&self, ip: &IpAddr) -> Option<GeoInfo> {
        let _ = ip;
        // TODO: query MaxMind reader
        None
    }

    /// Check if an IP's country is blocked.
    pub fn is_blocked(&self, ip: &IpAddr) -> bool {
        self.lookup(ip)
            .map(|geo| self.blocked_countries.contains(&geo.country_code))
            .unwrap_or(false)
    }
}

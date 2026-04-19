//! armageddon-sentinel: IPS engine with Aho-Corasick signatures, DLP, GeoIP, JA3, rate limiting.

pub mod ddos;
pub mod dlp;
pub mod geoip;
pub mod ips;
pub mod ja3;
pub mod ja4;
pub mod rate_limit;

use armageddon_common::context::RequestContext;
use armageddon_common::decision::{Decision, Severity};
use armageddon_common::engine::SecurityEngine;
use armageddon_common::error::Result;
use armageddon_config::security::SentinelConfig;
use async_trait::async_trait;

/// The SENTINEL security engine.
pub struct Sentinel {
    config: SentinelConfig,
    ips: ips::IpsEngine,
    dlp: dlp::DlpEngine,
    geoip: geoip::GeoIpEngine,
    ja3: ja3::Ja3Engine,
    ja4: ja4::Ja4Engine,
    rate_limiter: rate_limit::SlidingWindowLimiter,
    ready: bool,
}

impl Sentinel {
    pub fn new(config: SentinelConfig) -> Self {
        let rate_limiter = rate_limit::SlidingWindowLimiter::new(
            config.rate_limit.window_secs,
            config.rate_limit.max_requests,
        );
        Self {
            ips: ips::IpsEngine::new(&config.signature_path),
            dlp: dlp::DlpEngine::new(&config.dlp),
            geoip: geoip::GeoIpEngine::new(&config.geoip_db_path, &config.blocked_countries),
            ja3: ja3::Ja3Engine::new(config.ja3_blacklist_path.as_deref()),
            ja4: ja4::Ja4Engine::new(config.ja4_blacklist_path.as_deref()),
            rate_limiter,
            config,
            ready: false,
        }
    }
}

#[async_trait]
impl SecurityEngine for Sentinel {
    fn name(&self) -> &'static str {
        "SENTINEL"
    }

    async fn init(&mut self) -> Result<()> {
        tracing::info!("SENTINEL initializing IPS signatures, GeoIP DB, DLP patterns...");
        // Initialize the IPS engine with built-in signatures
        self.ips.init();
        // GeoIP and DLP init are best-effort (files may not exist)
        let _ = self.geoip.init();
        let _ = self.dlp.load_patterns();
        self.ready = true;
        Ok(())
    }

    async fn inspect(&self, ctx: &RequestContext) -> Result<Decision> {
        let start = std::time::Instant::now();

        if !self.config.enabled {
            return Ok(Decision::allow(self.name(), start.elapsed().as_micros() as u64));
        }

        // 1. Rate limit check
        if self.config.rate_limit.enabled {
            let rate_key = ctx.connection.client_ip.to_string();
            if !self.rate_limiter.allow(&rate_key) {
                return Ok(Decision::deny(
                    self.name(),
                    "SENTINEL-RATE-001",
                    &format!(
                        "Rate limit exceeded for {} ({} req/{}s)",
                        rate_key, self.config.rate_limit.max_requests, self.config.rate_limit.window_secs
                    ),
                    Severity::Medium,
                    start.elapsed().as_micros() as u64,
                ));
            }
        }

        // 2. GeoIP check
        if self.geoip.is_blocked(&ctx.connection.client_ip) {
            return Ok(Decision::deny(
                self.name(),
                "SENTINEL-GEO-001",
                &format!("Blocked country for IP {}", ctx.connection.client_ip),
                Severity::High,
                start.elapsed().as_micros() as u64,
            ));
        }

        // 3. JA3 fingerprint check
        if let Some(ja3) = &ctx.connection.ja3_fingerprint {
            if self.ja3.is_blacklisted(ja3) {
                return Ok(Decision::deny(
                    self.name(),
                    "SENTINEL-JA3-001",
                    "Blacklisted JA3 TLS fingerprint",
                    Severity::High,
                    start.elapsed().as_micros() as u64,
                ));
            }
        }

        // 3b. JA4 fingerprint check (modern TLS 1.3-aware fingerprint)
        if let Some(ja4) = &ctx.connection.ja4_fingerprint {
            if self.ja4.observe(ja4) {
                return Ok(Decision::deny(
                    self.name(),
                    "SENTINEL-JA4-001",
                    "Blacklisted JA4 TLS fingerprint",
                    Severity::High,
                    start.elapsed().as_micros() as u64,
                ));
            }
        }

        // 4. IPS signature scan
        let header_pairs: Vec<(String, String)> = ctx
            .request
            .headers
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let ips_matches = self.ips.scan_request(
            &ctx.request.uri,
            &header_pairs,
            ctx.request.body.as_deref(),
        );

        if !ips_matches.is_empty() {
            // Pick the highest-severity match
            let worst = &ips_matches[0];
            let severity = match worst.severity.as_str() {
                "critical" => Severity::Critical,
                "high" => Severity::High,
                "medium" => Severity::Medium,
                "low" => Severity::Low,
                _ => Severity::Medium,
            };

            let mut decision = Decision::deny(
                self.name(),
                &worst.signature_id,
                &format!(
                    "IPS signature matched: {} ({} total matches)",
                    worst.signature_name,
                    ips_matches.len()
                ),
                severity,
                start.elapsed().as_micros() as u64,
            );
            decision.tags = ips_matches
                .iter()
                .map(|m| format!("{:?}", m.category).to_lowercase())
                .collect();
            return Ok(decision);
        }

        // 5. DLP scan (if enabled)
        if self.config.dlp.enabled {
            if let Some(body) = &ctx.request.body {
                let dlp_matches = self.dlp.scan(body);
                if !dlp_matches.is_empty() {
                    return Ok(Decision::flag(
                        self.name(),
                        "SENTINEL-DLP-001",
                        &format!("DLP: sensitive data pattern detected ({})", dlp_matches[0].name),
                        Severity::High,
                        0.9,
                        start.elapsed().as_micros() as u64,
                    ));
                }
            }
        }

        Ok(Decision::allow(
            self.name(),
            start.elapsed().as_micros() as u64,
        ))
    }

    async fn shutdown(&self) -> Result<()> {
        tracing::info!("SENTINEL shutting down");
        Ok(())
    }

    fn is_ready(&self) -> bool {
        self.ready
    }
}

//! armageddon-arbiter: WAF engine with OWASP CRS v4, Aho-Corasick pattern matching.
//!
//! Scans requests for SQL injection, XSS, command injection, and path traversal.
//! Uses anomaly scoring: each rule match adds to a cumulative score. If the score
//! exceeds the threshold, the request is blocked.

pub mod crs;
pub mod matcher;
pub mod rule;

use armageddon_common::context::RequestContext;
use armageddon_common::decision::{Decision, Severity};
use armageddon_common::engine::SecurityEngine;
use armageddon_common::error::Result;
use armageddon_config::security::ArbiterConfig;
use async_trait::async_trait;

/// The ARBITER WAF engine.
pub struct Arbiter {
    config: ArbiterConfig,
    matcher: matcher::MultiPatternMatcher,
    rules: Vec<rule::WafRule>,
    ready: bool,
}

impl Arbiter {
    pub fn new(config: ArbiterConfig) -> Self {
        Self {
            matcher: matcher::MultiPatternMatcher::new(),
            rules: Vec::new(),
            config,
            ready: false,
        }
    }

    /// Compute anomaly score from pattern matches.
    fn compute_anomaly_score(
        &self,
        matches: &[matcher::PatternMatch],
    ) -> (u32, Vec<&rule::WafRule>) {
        let mut score: u32 = 0;
        let mut triggered_rules = Vec::new();
        let mut seen_rule_ids = std::collections::HashSet::new();

        // Map pattern IDs back to rules (patterns are in the same order as rules)
        let applicable_rules: Vec<&rule::WafRule> = self
            .rules
            .iter()
            .filter(|r| r.paranoia_level <= self.config.paranoia_level && r.enabled)
            .collect();

        for m in matches {
            if let Some(rule) = applicable_rules.get(m.pattern_id) {
                if seen_rule_ids.insert(rule.id) {
                    score += rule.score;
                    triggered_rules.push(*rule);
                }
            }
        }

        (score, triggered_rules)
    }
}

#[async_trait]
impl SecurityEngine for Arbiter {
    fn name(&self) -> &'static str {
        "ARBITER"
    }

    async fn init(&mut self) -> Result<()> {
        tracing::info!(
            "ARBITER initializing WAF (paranoia level {}, CRS from {})",
            self.config.paranoia_level,
            self.config.crs_path,
        );

        // Load CRS rules
        let loader = crs::CrsLoader::new(&self.config.crs_path);
        self.rules = loader.load_at_paranoia(self.config.paranoia_level);

        // Extract patterns and compile Aho-Corasick automaton
        let patterns: Vec<String> = self
            .rules
            .iter()
            .filter(|r| r.enabled)
            .map(|r| r.pattern.clone())
            .collect();
        self.matcher.compile(patterns);

        tracing::info!(
            "ARBITER loaded {} rules ({} at paranoia level {})",
            self.rules.len(),
            self.matcher.pattern_count(),
            self.config.paranoia_level,
        );

        self.ready = true;
        Ok(())
    }

    async fn inspect(&self, ctx: &RequestContext) -> Result<Decision> {
        let start = std::time::Instant::now();

        if !self.config.enabled {
            return Ok(Decision::allow(self.name(), start.elapsed().as_micros() as u64));
        }

        let mut all_matches = Vec::new();

        // Scan URI
        all_matches.extend(self.matcher.scan(ctx.request.uri.as_bytes()));

        // Scan query string
        if let Some(query) = &ctx.request.query {
            all_matches.extend(self.matcher.scan(query.as_bytes()));
        }

        // Scan header values
        for (_name, value) in &ctx.request.headers {
            all_matches.extend(self.matcher.scan(value.as_bytes()));
        }

        // Scan body
        if let Some(body) = &ctx.request.body {
            all_matches.extend(self.matcher.scan(body));
        }

        let latency = start.elapsed().as_micros() as u64;

        if all_matches.is_empty() {
            return Ok(Decision::allow(self.name(), latency));
        }

        // Compute anomaly score
        let (anomaly_score, triggered_rules) = self.compute_anomaly_score(&all_matches);

        // Build description from triggered rules
        let rule_names: Vec<&str> = triggered_rules.iter().map(|r| r.name.as_str()).collect();
        let tags: Vec<String> = triggered_rules
            .iter()
            .flat_map(|r| r.tags.clone())
            .collect();

        if anomaly_score >= self.config.anomaly_threshold {
            // Block
            let rule_id = triggered_rules
                .first()
                .map_or("ARBITER-MULTI", |_| {
                    // We can't return a reference to a formatted string, so use a static
                    // fallback. The rule ID is stored in the description instead.
                    "ARBITER-WAF"
                });

            let severity = if anomaly_score >= self.config.anomaly_threshold * 2 {
                Severity::Critical
            } else {
                Severity::High
            };

            let mut decision = Decision::deny(
                self.name(),
                rule_id,
                &format!(
                    "WAF anomaly score {} >= threshold {} (rules: {})",
                    anomaly_score,
                    self.config.anomaly_threshold,
                    rule_names.join(", ")
                ),
                severity,
                latency,
            );
            decision.tags = tags;
            Ok(decision)
        } else if self.config.learning_mode {
            // Learning mode: flag but don't block
            let mut decision = Decision::flag(
                self.name(),
                "ARBITER-LEARN",
                &format!(
                    "WAF learning mode: anomaly score {} (rules: {})",
                    anomaly_score,
                    rule_names.join(", ")
                ),
                Severity::Medium,
                anomaly_score as f64 / self.config.anomaly_threshold as f64,
                latency,
            );
            decision.tags = tags;
            Ok(decision)
        } else {
            // Below threshold: flag for awareness
            let mut decision = Decision::flag(
                self.name(),
                "ARBITER-FLAG",
                &format!(
                    "WAF patterns detected (score {}, threshold {}): {}",
                    anomaly_score,
                    self.config.anomaly_threshold,
                    rule_names.join(", ")
                ),
                Severity::Low,
                anomaly_score as f64 / self.config.anomaly_threshold as f64,
                latency,
            );
            decision.tags = tags;
            Ok(decision)
        }
    }

    async fn shutdown(&self) -> Result<()> {
        tracing::info!("ARBITER shutting down");
        Ok(())
    }

    fn is_ready(&self) -> bool {
        self.ready
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use armageddon_common::context::RequestContext;
    use armageddon_common::decision::Verdict;
    use armageddon_common::types::*;
    use std::collections::HashMap;
    use std::net::{IpAddr, Ipv4Addr};

    fn make_config() -> ArbiterConfig {
        ArbiterConfig {
            enabled: true,
            paranoia_level: 1,
            crs_path: "/dev/null".to_string(),
            custom_rules_path: None,
            anomaly_threshold: 5,
            learning_mode: false,
        }
    }

    fn make_context(uri: &str, body: Option<&str>) -> RequestContext {
        RequestContext::new(
            HttpRequest {
                method: "POST".to_string(),
                uri: uri.to_string(),
                path: uri.to_string(),
                query: None,
                headers: HashMap::new(),
                body: body.map(|b| b.as_bytes().to_vec()),
                version: HttpVersion::Http11,
            },
            ConnectionInfo {
                client_ip: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
                client_port: 12345,
                server_ip: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 100)),
                server_port: 443,
                tls: None,
                ja3_fingerprint: None,
                ja4_fingerprint: None,
            },
            Protocol::Http,
        )
    }

    #[tokio::test]
    async fn test_arbiter_detects_sqli() {
        let mut arbiter = Arbiter::new(make_config());
        arbiter.init().await.unwrap();

        let ctx = make_context("/api/users", Some("id=1 union select * from users"));
        let decision = arbiter.inspect(&ctx).await.unwrap();
        assert!(
            decision.verdict == Verdict::Deny || decision.verdict == Verdict::Flag,
            "Expected Deny or Flag for SQL injection, got {:?}",
            decision.verdict
        );
    }

    #[tokio::test]
    async fn test_arbiter_detects_xss() {
        let mut arbiter = Arbiter::new(make_config());
        arbiter.init().await.unwrap();

        let ctx = make_context("/api/comments", Some("<script>alert(1)</script>"));
        let decision = arbiter.inspect(&ctx).await.unwrap();
        assert!(
            decision.verdict == Verdict::Deny || decision.verdict == Verdict::Flag,
            "Expected Deny or Flag for XSS, got {:?}",
            decision.verdict
        );
    }

    #[tokio::test]
    async fn test_arbiter_allows_clean_request() {
        let mut arbiter = Arbiter::new(make_config());
        arbiter.init().await.unwrap();

        let ctx = make_context("/api/users/123", Some("{\"name\": \"John Doe\"}"));
        let decision = arbiter.inspect(&ctx).await.unwrap();
        assert_eq!(decision.verdict, Verdict::Allow);
    }
}

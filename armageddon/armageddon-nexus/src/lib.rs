//! armageddon-nexus: The brain - aggregation, correlation, scoring.
//!
//! Collects decisions from all Pentagon engines, correlates them, computes
//! a final composite score, and makes the ultimate allow/block decision.
//! Connects to KAYA for caching and state persistence via RESP3+ on port 6380.

pub mod aggregator;
pub mod kaya;
pub mod scorer;

use armageddon_common::context::RequestContext;
use armageddon_common::decision::{Action, Decision, Verdict};
use armageddon_config::security::NexusConfig;

/// The NEXUS brain.
pub struct Nexus {
    config: NexusConfig,
    scorer: scorer::CompositeScorer,
    correlator: aggregator::Correlator,
    kaya_client: kaya::KayaClient,
}

impl Nexus {
    pub fn new(config: NexusConfig, kaya_host: &str, kaya_port: u16) -> Self {
        let correlator = aggregator::Correlator::new(config.correlation_window_ms);
        Self {
            scorer: scorer::CompositeScorer::new(
                config.block_threshold,
                config.challenge_threshold,
            ),
            correlator,
            kaya_client: kaya::KayaClient::new(kaya_host, kaya_port),
            config,
        }
    }

    /// Get the KAYA client for external use.
    pub fn kaya(&self) -> &kaya::KayaClient {
        &self.kaya_client
    }

    /// Connect to KAYA (call during init).
    pub async fn connect_kaya(&self) {
        match self.kaya_client.connect().await {
            Ok(()) => tracing::info!("NEXUS connected to KAYA"),
            Err(e) => tracing::warn!("NEXUS failed to connect to KAYA (will operate without cache): {}", e),
        }
    }

    /// Aggregate decisions from all engines and produce a final verdict.
    ///
    /// Algorithm:
    /// 1. If any engine returned a hard Deny with high confidence, block immediately.
    /// 2. Run cross-engine correlation to detect multi-vector attacks.
    /// 3. Compute weighted composite score.
    /// 4. If multi-vector attack detected, boost the score by 20%.
    /// 5. Compare against block/challenge thresholds.
    pub fn aggregate(&self, ctx: &RequestContext, decisions: &[Decision]) -> FinalVerdict {
        // Step 1: Immediate block on high-confidence deny
        for d in decisions {
            if d.verdict == Verdict::Deny && d.confidence >= 0.95 {
                return FinalVerdict {
                    action: Action::Block,
                    score: 1.0,
                    reason: format!(
                        "Blocked by {} (rule: {}, {})",
                        d.engine,
                        d.rule_id.as_deref().unwrap_or("N/A"),
                        d.description
                    ),
                    decisions: decisions.to_vec(),
                    request_id: ctx.request_id,
                };
            }
        }

        // Step 2: Correlation analysis
        let correlation = self.correlator.correlate(decisions);

        // Step 3: Compute base composite score
        let mut score = self.scorer.score(decisions);

        // Step 4: Boost score for multi-vector attacks
        if correlation.is_multi_vector {
            let boost = 0.2;
            score = (score + boost).min(1.0);
            tracing::warn!(
                request_id = %ctx.request_id,
                "multi-vector attack detected ({} engines flagged, correlated tags: {:?}), score boosted to {:.4}",
                correlation.engines_flagged,
                correlation.correlated_tags,
                score,
            );
        }

        // Step 5: Determine action based on thresholds
        let action = if score >= self.config.block_threshold {
            Action::Block
        } else if score >= self.config.challenge_threshold {
            Action::Challenge
        } else if score > 0.0 {
            Action::LogOnly
        } else {
            Action::Forward
        };

        let reason = match action {
            Action::Forward => "All engines cleared the request".to_string(),
            Action::LogOnly => format!(
                "Minor anomalies detected (score {:.4}), logging only",
                score,
            ),
            _ => format!(
                "Composite score {:.4} (thresholds: block={}, challenge={}; engines flagged: {}/{})",
                score,
                self.config.block_threshold,
                self.config.challenge_threshold,
                correlation.engines_flagged,
                correlation.total_engines,
            ),
        };

        FinalVerdict {
            action,
            score,
            reason,
            decisions: decisions.to_vec(),
            request_id: ctx.request_id,
        }
    }
}

/// The final verdict after NEXUS aggregation.
#[derive(Debug, Clone)]
pub struct FinalVerdict {
    pub action: Action,
    pub score: f64,
    pub reason: String,
    pub decisions: Vec<Decision>,
    pub request_id: uuid::Uuid,
}

#[cfg(test)]
mod tests {
    use super::*;
    use armageddon_common::context::RequestContext;
    use armageddon_common::decision::{Decision, Severity};
    use armageddon_common::types::*;
    use std::collections::HashMap;
    use std::net::{IpAddr, Ipv4Addr};

    fn make_config() -> NexusConfig {
        NexusConfig {
            block_threshold: 0.8,
            challenge_threshold: 0.5,
            correlation_window_ms: 1000,
        }
    }

    fn make_context() -> RequestContext {
        RequestContext::new(
            HttpRequest {
                method: "GET".to_string(),
                uri: "/api/test".to_string(),
                path: "/api/test".to_string(),
                query: None,
                headers: HashMap::new(),
                body: None,
                version: HttpVersion::Http11,
            },
            ConnectionInfo {
                client_ip: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
                client_port: 12345,
                server_ip: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 100)),
                server_port: 443,
                tls: None,
                ja3_fingerprint: None,
            },
            Protocol::Http,
        )
    }

    #[test]
    fn test_all_allow_forwards() {
        let nexus = Nexus::new(make_config(), "localhost", 6380);
        let ctx = make_context();
        let decisions = vec![
            Decision::allow("SENTINEL", 100),
            Decision::allow("ARBITER", 200),
            Decision::allow("ORACLE", 150),
            Decision::allow("AEGIS", 180),
            Decision::allow("AI", 50),
        ];

        let verdict = nexus.aggregate(&ctx, &decisions);
        assert_eq!(verdict.action, Action::Forward);
        assert_eq!(verdict.score, 0.0);
    }

    #[test]
    fn test_high_confidence_deny_blocks() {
        let nexus = Nexus::new(make_config(), "localhost", 6380);
        let ctx = make_context();
        let decisions = vec![
            Decision::deny("SENTINEL", "SIG-SQLI-001", "SQL injection", Severity::Critical, 100),
            Decision::allow("ARBITER", 200),
        ];

        let verdict = nexus.aggregate(&ctx, &decisions);
        assert_eq!(verdict.action, Action::Block);
        assert_eq!(verdict.score, 1.0);
    }

    #[test]
    fn test_multi_vector_boost() {
        let nexus = Nexus::new(make_config(), "localhost", 6380);
        let ctx = make_context();
        let decisions = vec![
            Decision::flag("SENTINEL", "SIG-001", "Suspicious", Severity::Medium, 0.6, 100),
            Decision::flag("ARBITER", "WAF-001", "Suspicious", Severity::Medium, 0.6, 200),
            Decision::allow("ORACLE", 150),
            Decision::allow("AEGIS", 180),
            Decision::allow("AI", 50),
        ];

        let verdict = nexus.aggregate(&ctx, &decisions);
        // Multi-vector flag should boost the score
        assert!(verdict.score > 0.0);
    }
}

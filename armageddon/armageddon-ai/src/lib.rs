//! armageddon-ai: Threat intelligence and prompt injection detection.

pub mod prompt_injection;
pub mod threat_intel;

use armageddon_common::context::RequestContext;
use armageddon_common::decision::{Decision, Severity};
use armageddon_common::engine::SecurityEngine;
use armageddon_common::error::Result;
use armageddon_config::security::AiConfig;
use async_trait::async_trait;

/// The AI security engine.
pub struct AiEngine {
    config: AiConfig,
    threat_intel: threat_intel::ThreatIntelManager,
    prompt_detector: prompt_injection::PromptInjectionDetector,
    ready: bool,
}

impl AiEngine {
    pub fn new(config: AiConfig) -> Self {
        let threat_intel =
            threat_intel::ThreatIntelManager::new(&config.threat_intel_feeds, config.refresh_interval_secs);
        let prompt_detector =
            prompt_injection::PromptInjectionDetector::new(config.prompt_injection_model_path.as_deref());
        Self {
            config,
            threat_intel,
            prompt_detector,
            ready: false,
        }
    }
}

#[async_trait]
impl SecurityEngine for AiEngine {
    fn name(&self) -> &'static str {
        "AI"
    }

    async fn init(&mut self) -> Result<()> {
        tracing::info!(
            "AI engine initializing ({} threat feeds, prompt injection: {})",
            self.config.threat_intel_feeds.len(),
            self.config.prompt_injection_model_path.is_some(),
        );
        self.ready = true;
        Ok(())
    }

    async fn inspect(&self, ctx: &RequestContext) -> Result<Decision> {
        let start = std::time::Instant::now();

        // 1. Check threat intelligence feeds for known-bad IPs
        let ip_str = ctx.connection.client_ip.to_string();
        if self.threat_intel.is_known_threat(&ip_str) {
            return Ok(Decision::deny(
                self.name(),
                "AI-THREAT-001",
                "IP found in threat intelligence feed",
                Severity::High,
                start.elapsed().as_micros() as u64,
            ));
        }

        // 2. Prompt injection detection (for LLM-facing endpoints)
        if let Some(body) = &ctx.request.body {
            if let Ok(text) = std::str::from_utf8(body) {
                let score = self.prompt_detector.detect(text);
                if score > 0.8 {
                    return Ok(Decision::flag(
                        self.name(),
                        "AI-PROMPT-001",
                        "Possible prompt injection detected",
                        Severity::High,
                        score,
                        start.elapsed().as_micros() as u64,
                    ));
                }
            }
        }

        Ok(Decision::allow(self.name(), start.elapsed().as_micros() as u64))
    }

    async fn shutdown(&self) -> Result<()> {
        tracing::info!("AI engine shutting down");
        Ok(())
    }

    fn is_ready(&self) -> bool {
        self.ready
    }
}

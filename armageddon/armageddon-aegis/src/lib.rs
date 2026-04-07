//! armageddon-aegis: Policy engine using Rego (via Microsoft Regorus).
//!
//! Implements deny-by-default policy evaluation for authorization decisions.

pub mod policy;

use armageddon_common::context::RequestContext;
use armageddon_common::decision::{Decision, Severity};
use armageddon_common::engine::SecurityEngine;
use armageddon_common::error::Result;
use armageddon_config::security::AegisConfig;
use async_trait::async_trait;

/// The AEGIS policy engine.
pub struct Aegis {
    config: AegisConfig,
    engine: policy::PolicyEngine,
    ready: bool,
}

impl Aegis {
    pub fn new(config: AegisConfig) -> Self {
        let engine = policy::PolicyEngine::new(&config.policy_dir);
        Self {
            config,
            engine,
            ready: false,
        }
    }
}

#[async_trait]
impl SecurityEngine for Aegis {
    fn name(&self) -> &'static str {
        "AEGIS"
    }

    async fn init(&mut self) -> Result<()> {
        tracing::info!(
            "AEGIS initializing Rego policy engine (policies from {}, default: {:?})",
            self.config.policy_dir,
            self.config.default_decision,
        );
        // TODO: load all .rego files from policy_dir
        self.ready = true;
        Ok(())
    }

    async fn inspect(&self, ctx: &RequestContext) -> Result<Decision> {
        let start = std::time::Instant::now();

        // Build input document for Rego evaluation
        let input = self.engine.build_input(ctx);

        // Evaluate policy
        match self.engine.evaluate(&input) {
            Ok(allowed) => {
                let latency = start.elapsed().as_micros() as u64;
                if allowed {
                    Ok(Decision::allow(self.name(), latency))
                } else {
                    Ok(Decision::deny(
                        self.name(),
                        "AEGIS-POLICY-001",
                        "Request denied by policy",
                        Severity::High,
                        latency,
                    ))
                }
            }
            Err(e) => {
                let latency = start.elapsed().as_micros() as u64;
                // Fail-closed: deny on policy evaluation error
                tracing::error!("AEGIS policy evaluation error: {}", e);
                Ok(Decision::deny(
                    self.name(),
                    "AEGIS-ERROR-001",
                    &format!("Policy evaluation failed (fail-closed): {}", e),
                    Severity::Critical,
                    latency,
                ))
            }
        }
    }

    async fn shutdown(&self) -> Result<()> {
        tracing::info!("AEGIS shutting down");
        Ok(())
    }

    fn is_ready(&self) -> bool {
        self.ready
    }
}

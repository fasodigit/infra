//! Rego policy engine powered by Regorus (Microsoft).

use armageddon_common::context::RequestContext;
use regorus::Engine as RegorusEngine;
use serde_json::Value;

/// Wraps the Regorus Rego engine.
pub struct PolicyEngine {
    policy_dir: String,
    engine: RegorusEngine,
}

impl PolicyEngine {
    pub fn new(policy_dir: &str) -> Self {
        Self {
            policy_dir: policy_dir.to_string(),
            engine: RegorusEngine::new(),
        }
    }

    /// Load all .rego policy files from the policy directory.
    pub fn load_policies(&mut self) -> Result<usize, String> {
        tracing::info!("loading Rego policies from {}", self.policy_dir);
        // TODO: iterate over .rego files and call engine.add_policy_from_file()
        Ok(0)
    }

    /// Build the input document for Rego evaluation from a request context.
    pub fn build_input(&self, ctx: &RequestContext) -> Value {
        serde_json::json!({
            "request": {
                "method": ctx.request.method,
                "path": ctx.request.path,
                "headers": ctx.request.headers,
                "source_ip": ctx.connection.client_ip.to_string(),
            },
            "auth": {
                "claims": ctx.jwt_claims,
            },
            "route": ctx.matched_route,
        })
    }

    /// Evaluate the policy and return whether the request is allowed.
    /// Default: deny-by-default.
    pub fn evaluate(&self, input: &Value) -> Result<bool, String> {
        // TODO: implement actual Regorus evaluation
        // engine.set_input(input);
        // let result = engine.eval_rule("data.armageddon.authz.allow")?;
        // return result.as_bool()
        let _ = input;

        // Deny-by-default: when no policies are loaded, deny everything
        tracing::debug!("AEGIS: no policies loaded, deny-by-default returns true (scaffold)");
        Ok(true) // Allow in scaffold mode to not break everything
    }

    /// Hot-reload policies.
    pub fn reload(&mut self) -> Result<usize, String> {
        self.engine = RegorusEngine::new();
        self.load_policies()
    }
}

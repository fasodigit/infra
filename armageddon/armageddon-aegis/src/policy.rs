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

        let dir = match std::fs::read_dir(&self.policy_dir) {
            Ok(d) => d,
            Err(e) => {
                // If directory doesn't exist, log and return 0 (no policies loaded)
                tracing::warn!("policy directory '{}' not readable: {}", self.policy_dir, e);
                return Ok(0);
            }
        };

        let mut count = 0usize;
        for entry in dir {
            let entry = entry.map_err(|e| format!("failed to read dir entry: {}", e))?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("rego") {
                let path_str = path.display().to_string();
                self.engine
                    .add_policy_from_file(path)
                    .map_err(|e| format!("failed to load policy '{}': {}", path_str, e))?;
                tracing::info!("AEGIS loaded policy: {}", path_str);
                count += 1;
            }
        }

        Ok(count)
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
        let input_str = serde_json::to_string(input)
            .map_err(|e| format!("failed to serialize input: {}", e))?;

        // Clone the engine for this evaluation (Regorus engine is not Send-safe
        // for concurrent eval, so we clone per-evaluation)
        let mut eval_engine = self.engine.clone();

        eval_engine
            .set_input(
                regorus::Value::from_json_str(&input_str)
                    .map_err(|e| format!("failed to parse input as Regorus value: {}", e))?,
            );

        let result = eval_engine
            .eval_rule("data.armageddon.authz.allow".to_string())
            .map_err(|e| format!("Rego evaluation error: {}", e))?;

        // The result should be a boolean. If not, deny by default.
        match result.as_bool() {
            Ok(allowed) => Ok(*allowed),
            Err(_) => {
                tracing::warn!(
                    "AEGIS: policy result is not boolean ({:?}), denying by default",
                    result
                );
                Ok(false)
            }
        }
    }

    /// Hot-reload policies.
    pub fn reload(&mut self) -> Result<usize, String> {
        self.engine = RegorusEngine::new();
        self.load_policies()
    }
}

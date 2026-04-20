// SPDX-License-Identifier: AGPL-3.0-or-later
//! AEGIS (Rego policy engine) adapter for the Pingora pipeline.
//!
//! Wraps [`armageddon_aegis::Aegis`] — a `SecurityEngine` over Microsoft
//! Regorus — so the pipeline can treat it the same way as every other
//! engine.  AEGIS is the first real adapter wired in M3 because it is
//! stateless (no ML model, no async I/O) and therefore the lowest-risk
//! port to validate the pipeline contract.
//!
//! # Context conversion
//!
//! The pingora [`RequestCtx`] does not (yet) carry the full HTTP request
//! body needed by [`armageddon_common::RequestContext`]; the filter
//! chain (M1 #95 / M2 #103) only populates identity fields
//! (`user_id`, `tenant_id`, `roles`, `cluster`, `request_id`).  We
//! therefore synthesise a minimal `RequestContext` whose
//! `request`/`connection` fields are placeholders — a Rego policy that
//! inspects real HTTP data must wait for M3-follow-up #TODO.
//!
//! TODO(#104): once [`RequestCtx`] carries the full HTTP request (see
//! M1 router #95 landing), replace [`request_context_from_ctx`] with a
//! zero-copy view into the Pingora `Session`.

use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use armageddon_aegis::Aegis;
use armageddon_common::context::RequestContext;
use armageddon_common::decision::Verdict;
use armageddon_common::engine::SecurityEngine;
use armageddon_common::types::{
    ConnectionInfo, HttpRequest, HttpVersion, Protocol,
};

use super::pipeline::{EngineAdapter, EngineVerdict};
use crate::pingora::ctx::RequestCtx;

/// Pipeline adapter wrapping a ready (`init().await` already called)
/// [`Aegis`] engine.
pub struct AegisAdapter {
    aegis: Arc<Aegis>,
}

impl AegisAdapter {
    /// Wrap an already-initialised [`Aegis`] instance.  The caller is
    /// responsible for calling `Aegis::init().await` once at startup
    /// (policy load); this adapter never re-initialises.
    pub fn new(aegis: Arc<Aegis>) -> Self {
        Self { aegis }
    }
}

#[async_trait]
impl EngineAdapter for AegisAdapter {
    fn name(&self) -> &'static str {
        "aegis"
    }

    async fn analyze(&self, ctx: &mut RequestCtx) -> EngineVerdict {
        // If AEGIS never finished init, skip rather than evaluate with a
        // cold Regorus engine (which would fail-closed on every call).
        if !self.aegis.is_ready() {
            tracing::debug!("aegis adapter: engine not ready; skipping");
            return EngineVerdict::Skipped;
        }

        let req_ctx = request_context_from_ctx(ctx);
        match self.aegis.inspect(&req_ctx).await {
            Ok(decision) => match decision.verdict {
                Verdict::Allow => EngineVerdict::Allow {
                    score: clamp01(1.0 - decision.confidence as f32),
                },
                Verdict::Deny => EngineVerdict::Deny {
                    score: clamp01(decision.confidence as f32),
                    reason: decision.description,
                },
                // `Flag` and `Abstain` defer to NEXUS / aggregate scoring;
                // treat them as an allow with partial score.
                Verdict::Flag | Verdict::Abstain => EngineVerdict::Allow {
                    score: clamp01(decision.confidence as f32),
                },
            },
            Err(e) => {
                tracing::warn!(error = %e, "aegis inspect failed; treating as Skipped");
                EngineVerdict::Skipped
            }
        }
    }

    fn timeout(&self) -> Duration {
        // Rego evaluation over Regorus can be markedly slower than the
        // 5 ms default — 10 ms matches the SLO budget in
        // INFRA/observability/slo/armageddon-aegis.slo.yaml.
        Duration::from_millis(10)
    }
}

/// Build a best-effort [`RequestContext`] from the pingora per-request
/// state.  Fields not yet populated by the filter chain (HTTP method,
/// path, headers) are filled with empty placeholders so Rego policies
/// that reference them fail-closed rather than panic.
fn request_context_from_ctx(ctx: &RequestCtx) -> RequestContext {
    let request = HttpRequest {
        method: String::new(),
        uri: String::new(),
        path: String::new(),
        query: None,
        headers: Default::default(),
        body: None,
        version: HttpVersion::Http11,
    };
    let connection = ConnectionInfo {
        client_ip: IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
        client_port: 0,
        server_ip: IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
        server_port: 0,
        tls: None,
        ja3_fingerprint: None,
        ja4_fingerprint: None,
    };
    let mut rc = RequestContext::new(request, connection, Protocol::Http);
    // Identity fields already populated by M1 filters — copy them over.
    rc.user_id = ctx.user_id.clone();
    rc.tenant_id = ctx.tenant_id.clone();
    rc.user_roles = ctx.roles.clone();
    if !ctx.cluster.is_empty() {
        rc.target_cluster = Some(ctx.cluster.clone());
    }
    if let Ok(uuid) = uuid::Uuid::parse_str(&ctx.request_id) {
        rc.request_id = uuid;
    }
    rc
}

fn clamp01(v: f32) -> f32 {
    v.clamp(0.0, 1.0)
}

// ── tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use armageddon_config::security::{AegisConfig, AegisDefault};
    use std::path::PathBuf;

    /// Create a temporary directory containing a single `.rego` policy.
    /// Returns the directory handle (kept alive for the caller) and the
    /// path to the dir (as a string, as expected by AegisConfig).
    fn policy_dir_with(policy: &str) -> (PathBuf, String) {
        use std::time::{SystemTime, UNIX_EPOCH};
        let ns = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("armageddon-aegis-test-{ns}"));
        std::fs::create_dir_all(&dir).expect("mkdir tempdir");
        std::fs::write(dir.join("policy.rego"), policy).expect("write policy");
        let path = dir.to_string_lossy().into_owned();
        (dir, path)
    }

    async fn make_aegis(policy_dir: &str, enabled: bool) -> Arc<Aegis> {
        let cfg = AegisConfig {
            enabled,
            policy_dir: policy_dir.to_string(),
            default_decision: AegisDefault::Deny,
        };
        let mut a = Aegis::new(cfg);
        a.init().await.expect("aegis init");
        Arc::new(a)
    }

    #[tokio::test]
    async fn aegis_adapter_always_allow_policy_returns_allow() {
        let policy = r#"
package armageddon.authz

default allow := false

allow := true
"#;
        let (dir, path) = policy_dir_with(policy);
        let aegis = make_aegis(&path, true).await;
        let adapter = AegisAdapter::new(aegis);
        let mut ctx = RequestCtx::new();
        let v = adapter.analyze(&mut ctx).await;
        match v {
            EngineVerdict::Allow { .. } => {}
            other => panic!("expected Allow, got {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn aegis_adapter_always_deny_policy_returns_deny() {
        let policy = r#"
package armageddon.authz

default allow := false
"#;
        let (dir, path) = policy_dir_with(policy);
        let aegis = make_aegis(&path, true).await;
        let adapter = AegisAdapter::new(aegis);
        let mut ctx = RequestCtx::new();
        let v = adapter.analyze(&mut ctx).await;
        match v {
            EngineVerdict::Deny { reason, .. } => {
                assert!(!reason.is_empty(), "deny reason must not be empty");
            }
            other => panic!("expected Deny, got {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn aegis_adapter_respects_timeout() {
        // Engine is not ready → adapter must return Skipped fast,
        // regardless of the 10 ms timeout budget.
        let cfg = AegisConfig {
            enabled: true,
            policy_dir: "/nonexistent/path/for/timeout-test".to_string(),
            default_decision: AegisDefault::Deny,
        };
        let aegis = Aegis::new(cfg); // note: init() NOT called → not ready
        let adapter = AegisAdapter::new(Arc::new(aegis));
        assert_eq!(adapter.timeout(), Duration::from_millis(10));
        let mut ctx = RequestCtx::new();
        let v = adapter.analyze(&mut ctx).await;
        assert!(matches!(v, EngineVerdict::Skipped), "got {v:?}");
    }

    #[tokio::test]
    async fn aegis_adapter_disabled_returns_allow() {
        let policy = r#"
package armageddon.authz

default allow := false
"#;
        let (dir, path) = policy_dir_with(policy);
        // `enabled = false` — AEGIS short-circuits Inspect to Allow.
        let aegis = make_aegis(&path, false).await;
        let adapter = AegisAdapter::new(aegis);
        let mut ctx = RequestCtx::new();
        let v = adapter.analyze(&mut ctx).await;
        assert!(matches!(v, EngineVerdict::Allow { .. }), "got {v:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

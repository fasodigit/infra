// SPDX-License-Identifier: AGPL-3.0-or-later
//! SENTINEL (IPS / DLP / GeoIP / JA3 / JA4 / rate-limit) adapter.
//!
//! # Design
//!
//! Wraps [`armageddon_sentinel::Sentinel`] behind the uniform
//! [`EngineAdapter`] interface so the pipeline can treat it identically
//! to AEGIS, ARBITER, and ORACLE.
//!
//! # Short-circuit
//!
//! If [`RequestCtx::waf_score`] is already `>= 0.9` when `analyze` is
//! called (set by a previous cycle, or by ARBITER running in parallel
//! on the same clone), SENTINEL skips its own evaluation and returns
//! the in-progress score directly.  This avoids redundant scanning
//! when a hard block is already imminent.
//!
//! # Context mapping
//!
//! SENTINEL inspects [`armageddon_common::context::RequestContext`].
//! We build that from the Pingora [`RequestCtx`] via
//! [`super::aegis_adapter::request_context_from_ctx`] (the same
//! helper used by AEGIS, updated in M3-1 to carry real identity
//! fields).  Connection-level fields (client IP, JA3/JA4) are
//! zero-values until M4 adds them to `RequestCtx`.
//!
//! # Failure modes
//!
//! * **Engine not ready** (`is_ready() == false`): returns `Skipped`.
//!   The pipeline continues with other engines; NEXUS makes the final
//!   call without SENTINEL's input.
//! * **Inspect error**: logged at `warn`; treated as `Skipped` (fail-
//!   open for availability, but the pipeline deny-threshold still fires
//!   if aggregate score from other engines is high enough).
//! * **Timeout** (pipeline-level, 15 ms): handled by the pipeline
//!   orchestrator via `FuturesUnordered` drop.
//!
//! # Metrics
//!
//! Emits `tracing` spans; Prometheus counters are wired in M5.

use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use armageddon_common::context::RequestContext;
use armageddon_common::decision::Verdict;
use armageddon_common::engine::SecurityEngine;
use armageddon_common::types::ConnectionInfo;
use armageddon_sentinel::Sentinel;

use super::aegis_adapter::request_context_from_ctx;
use super::pipeline::{EngineAdapter, EngineVerdict};
use crate::pingora::ctx::RequestCtx;

/// WAF-score threshold above which SENTINEL skips its own evaluation.
/// A request that is already almost certain to be blocked does not need
/// another scan — skip to avoid wasting the 15 ms budget.
const WAF_SKIP_THRESHOLD: f32 = 0.9;

/// Pipeline adapter wrapping an initialised [`Sentinel`] engine.
pub struct SentinelAdapter {
    sentinel: Arc<Sentinel>,
}

impl SentinelAdapter {
    /// Wrap an already-initialised [`Sentinel`] instance.
    ///
    /// The caller must have called `Sentinel::init().await` before
    /// constructing this adapter; the adapter never re-initialises.
    pub fn new(sentinel: Arc<Sentinel>) -> Self {
        Self { sentinel }
    }
}

#[async_trait]
impl EngineAdapter for SentinelAdapter {
    fn name(&self) -> &'static str {
        "sentinel"
    }

    async fn analyze(&self, ctx: &mut RequestCtx) -> EngineVerdict {
        // Short-circuit: another engine already flagged a near-certain block.
        if ctx.waf_score >= WAF_SKIP_THRESHOLD {
            tracing::debug!(
                waf_score = ctx.waf_score,
                "sentinel adapter: waf_score >= {WAF_SKIP_THRESHOLD}; skipping (already flagged)"
            );
            return EngineVerdict::Allow {
                score: ctx.waf_score,
            };
        }

        if !self.sentinel.is_ready() {
            tracing::debug!("sentinel adapter: engine not ready; skipping");
            return EngineVerdict::Skipped;
        }

        let req_ctx = build_sentinel_ctx(ctx);
        match self.sentinel.inspect(&req_ctx).await {
            Ok(decision) => decision_to_verdict(decision),
            Err(e) => {
                tracing::warn!(error = %e, "sentinel inspect failed; treating as Skipped");
                EngineVerdict::Skipped
            }
        }
    }

    /// SENTINEL runs IPS (Aho-Corasick), GeoIP, JA3/JA4 and DLP scans —
    /// budget is 15 ms (more than AEGIS which is pure Rego).
    fn timeout(&self) -> Duration {
        Duration::from_millis(15)
    }
}

/// Build a [`RequestContext`] for SENTINEL from Pingora state.
///
/// Reuses the AEGIS helper for identity fields, then layers the
/// connection-level data that SENTINEL specifically needs (JA3/JA4,
/// client IP).  Both are zero-valued until M4 adds them to
/// `RequestCtx`; SENTINEL's GeoIP and JA3/JA4 checks will be no-ops
/// until then (the GeoIP and JA3 engines check `None` → skip).
fn build_sentinel_ctx(ctx: &RequestCtx) -> RequestContext {
    // Start from the AEGIS helper (identity + headers already populated).
    let mut rc = request_context_from_ctx(ctx);

    // Override connection with Sentinel-specific fields.
    // TODO(M4): replace with real client_ip / ja3 / ja4 from RequestCtx
    // once the upstream selector threads them through.
    rc.connection = ConnectionInfo {
        client_ip: IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
        client_port: 0,
        server_ip: IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
        server_port: 0,
        tls: None,
        ja3_fingerprint: None,
        ja4_fingerprint: None,
    };

    rc
}

/// Map a [`armageddon_common::decision::Decision`] to an [`EngineVerdict`].
fn decision_to_verdict(d: armageddon_common::decision::Decision) -> EngineVerdict {
    match d.verdict {
        Verdict::Allow => EngineVerdict::Allow {
            score: clamp01(1.0 - d.confidence as f32),
        },
        Verdict::Deny => EngineVerdict::Deny {
            score: clamp01(d.confidence as f32),
            reason: d.description,
        },
        // Flag / Abstain: defer to NEXUS; pass partial score.
        Verdict::Flag | Verdict::Abstain => EngineVerdict::Allow {
            score: clamp01(d.confidence as f32),
        },
    }
}

fn clamp01(v: f32) -> f32 {
    v.clamp(0.0, 1.0)
}

// ── tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use armageddon_config::security::{
        DlpConfig, RateLimitConfig, SentinelConfig,
    };
    #[allow(unused_imports)]
    use armageddon_config::security::RateLimitKeyType;

    fn make_sentinel_config(enabled: bool) -> SentinelConfig {
        use armageddon_config::security::RateLimitKeyType;
        SentinelConfig {
            enabled,
            // Non-existent paths → IPS / GeoIP / JA3 / JA4 engines skip
            // gracefully (they handle missing files as best-effort).
            signature_path: "/dev/null".to_string(),
            geoip_db_path: "/dev/null".to_string(),
            blocked_countries: vec![],
            ja3_blacklist_path: None,
            ja4_blacklist_path: None,
            rate_limit: RateLimitConfig {
                enabled: false,
                window_secs: 60,
                max_requests: 1000,
                key_type: RateLimitKeyType::Ip,
            },
            dlp: DlpConfig {
                enabled: false,
                patterns_path: "/dev/null".to_string(),
                scan_response: false,
            },
        }
    }

    async fn make_adapter(enabled: bool) -> SentinelAdapter {
        let cfg = make_sentinel_config(enabled);
        let mut s = Sentinel::new(cfg);
        s.init().await.expect("sentinel init");
        SentinelAdapter::new(Arc::new(s))
    }

    // ── Test 1: enabled + clean request → Allow ──────────────────────
    #[tokio::test]
    async fn sentinel_clean_request_returns_allow() {
        let adapter = make_adapter(true).await;
        let mut ctx = RequestCtx::new();
        let v = adapter.analyze(&mut ctx).await;
        // Clean request with no signatures / no blocked countries → Allow.
        assert!(
            matches!(v, EngineVerdict::Allow { .. }),
            "expected Allow for clean request, got {v:?}"
        );
    }

    // ── Test 2: disabled engine → Allow (engine returns allow internally) ──
    #[tokio::test]
    async fn sentinel_disabled_engine_returns_allow() {
        let adapter = make_adapter(false).await;
        let mut ctx = RequestCtx::new();
        let v = adapter.analyze(&mut ctx).await;
        assert!(
            matches!(v, EngineVerdict::Allow { .. }),
            "expected Allow when engine disabled, got {v:?}"
        );
    }

    // ── Test 3: waf_score >= 0.9 → short-circuit, no sentinel scan ───
    #[tokio::test]
    async fn sentinel_short_circuits_on_high_waf_score() {
        let adapter = make_adapter(true).await;
        let mut ctx = RequestCtx::new();
        ctx.waf_score = 0.95; // pre-flagged by arbiter (or previous cycle)
        let v = adapter.analyze(&mut ctx).await;
        // Short-circuit returns Allow with the existing high score; no scan.
        match v {
            EngineVerdict::Allow { score } => {
                assert!(
                    (score - 0.95).abs() < f32::EPSILON,
                    "score must be the pre-existing waf_score, got {score}"
                );
            }
            other => panic!("expected Allow (short-circuit), got {other:?}"),
        }
    }

    // ── Test 4: engine not ready → Skipped ───────────────────────────
    #[tokio::test]
    async fn sentinel_not_ready_returns_skipped() {
        use armageddon_config::security::RateLimitKeyType;
        let cfg = SentinelConfig {
            enabled: true,
            signature_path: "/dev/null".to_string(),
            geoip_db_path: "/dev/null".to_string(),
            blocked_countries: vec![],
            ja3_blacklist_path: None,
            ja4_blacklist_path: None,
            rate_limit: RateLimitConfig {
                enabled: false,
                window_secs: 60,
                max_requests: 1000,
                key_type: RateLimitKeyType::Ip,
            },
            dlp: DlpConfig {
                enabled: false,
                patterns_path: "/dev/null".to_string(),
                scan_response: false,
            },
        };
        // Note: init() NOT called → is_ready() == false
        let s = Sentinel::new(cfg);
        let adapter = SentinelAdapter::new(Arc::new(s));
        let mut ctx = RequestCtx::new();
        let v = adapter.analyze(&mut ctx).await;
        assert!(
            matches!(v, EngineVerdict::Skipped),
            "expected Skipped when not ready, got {v:?}"
        );
    }

    // ── Test 5: decision_to_verdict mapping ──────────────────────────
    #[test]
    fn decision_to_verdict_deny_maps_correctly() {
        use armageddon_common::decision::{Decision, Severity};
        let d = Decision::deny("SENTINEL", "SIG-001", "SQL injection", Severity::High, 100);
        let v = decision_to_verdict(d);
        match v {
            EngineVerdict::Deny { score, reason } => {
                assert!((score - 1.0).abs() < f32::EPSILON, "deny confidence=1.0");
                assert!(reason.contains("SQL injection"));
            }
            other => panic!("expected Deny, got {other:?}"),
        }
    }

    #[test]
    fn decision_to_verdict_flag_maps_to_allow_with_score() {
        use armageddon_common::decision::{Decision, Severity};
        let d = Decision::flag("SENTINEL", "DLP-001", "DLP match", Severity::Medium, 0.7, 50);
        let v = decision_to_verdict(d);
        match v {
            EngineVerdict::Allow { score } => {
                assert!((score - 0.7).abs() < 0.001, "flag confidence→score");
            }
            other => panic!("expected Allow (flag→defer), got {other:?}"),
        }
    }
}

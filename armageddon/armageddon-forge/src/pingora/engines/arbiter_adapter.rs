// SPDX-License-Identifier: AGPL-3.0-or-later
//! ARBITER (OWASP CRS v4 / Aho-Corasick WAF) adapter.
//!
//! # Design
//!
//! Wraps [`armageddon_arbiter::Arbiter`] — an anomaly-scoring WAF that
//! runs an Aho-Corasick multi-pattern automaton against the URI, query
//! string, headers, and request body.
//!
//! # Anomaly-score semantics
//!
//! ARBITER accumulates a rule-match score.  Its `inspect()` returns:
//!
//! * `Verdict::Allow`  — score below threshold, no patterns matched.
//! * `Verdict::Deny`   — score ≥ `anomaly_threshold`.
//! * `Verdict::Flag`   — score < threshold but patterns present, or
//!   learning mode active.  `confidence` carries the ratio
//!   `score / threshold` (0.0–1.0).
//!
//! The adapter maps `Flag` to `Allow { score: confidence }` so NEXUS
//! can weight it alongside other engine signals.
//!
//! # Failure modes
//!
//! * **Engine not ready**: `Skipped` — ARBITER rules not loaded yet.
//! * **Inspect error**: `Skipped`, logged at `warn`.
//! * **Pipeline timeout** (20 ms): handled by `FuturesUnordered` drop.
//!
//! Aho-Corasick scan is O(n) in input length but can spike on very
//! large bodies; the 20 ms timeout is deliberately generous.
//!
//! # Metrics
//!
//! Prometheus counters (`arbiter_deny_total`, `arbiter_flag_total`) are
//! planned for M5 — `tracing` spans cover observability until then.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use armageddon_arbiter::Arbiter;
use armageddon_common::decision::Verdict;
use armageddon_common::engine::SecurityEngine;

use super::aegis_adapter::request_context_from_ctx;
use super::pipeline::{EngineAdapter, EngineVerdict};
use crate::pingora::ctx::RequestCtx;

/// Pipeline adapter wrapping an initialised [`Arbiter`] WAF engine.
pub struct ArbiterAdapter {
    arbiter: Arc<Arbiter>,
}

impl ArbiterAdapter {
    /// Wrap an already-initialised [`Arbiter`] instance.
    ///
    /// The caller must have called `Arbiter::init().await` (which loads
    /// the CRS ruleset and compiles the Aho-Corasick automaton) before
    /// constructing this adapter.
    pub fn new(arbiter: Arc<Arbiter>) -> Self {
        Self { arbiter }
    }
}

#[async_trait]
impl EngineAdapter for ArbiterAdapter {
    fn name(&self) -> &'static str {
        "arbiter"
    }

    async fn analyze(&self, ctx: &mut RequestCtx) -> EngineVerdict {
        if !self.arbiter.is_ready() {
            tracing::debug!("arbiter adapter: engine not ready; skipping");
            return EngineVerdict::Skipped;
        }

        let req_ctx = request_context_from_ctx(ctx);
        match self.arbiter.inspect(&req_ctx).await {
            Ok(decision) => {
                tracing::trace!(
                    engine = "arbiter",
                    verdict = ?decision.verdict,
                    confidence = decision.confidence,
                    "arbiter decision"
                );
                decision_to_verdict(decision)
            }
            Err(e) => {
                tracing::warn!(error = %e, "arbiter inspect failed; treating as Skipped");
                EngineVerdict::Skipped
            }
        }
    }

    /// Aho-Corasick scan on URI + headers + body is bounded by O(n)
    /// but bodies can be large.  20 ms matches the SLO budget in
    /// `armageddon-arbiter.slo.yaml`.
    fn timeout(&self) -> Duration {
        Duration::from_millis(20)
    }
}

/// Map [`armageddon_common::decision::Decision`] → [`EngineVerdict`].
fn decision_to_verdict(d: armageddon_common::decision::Decision) -> EngineVerdict {
    match d.verdict {
        Verdict::Allow => EngineVerdict::Allow {
            score: clamp01(1.0 - d.confidence as f32),
        },
        Verdict::Deny => EngineVerdict::Deny {
            score: clamp01(d.confidence as f32),
            reason: d.description,
        },
        // Flag carries the confidence ratio (score/threshold).
        // Abstain defers to NEXUS.
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
    use armageddon_config::security::ArbiterConfig;

    fn make_config(enabled: bool, threshold: u32, learning: bool) -> ArbiterConfig {
        ArbiterConfig {
            enabled,
            paranoia_level: 1,
            // /dev/null → CrsLoader returns 0 rules; any real payload is Allow.
            crs_path: "/dev/null".to_string(),
            custom_rules_path: None,
            anomaly_threshold: threshold,
            learning_mode: learning,
        }
    }

    async fn make_adapter(enabled: bool) -> ArbiterAdapter {
        let cfg = make_config(enabled, 5, false);
        let mut a = Arbiter::new(cfg);
        a.init().await.expect("arbiter init");
        ArbiterAdapter::new(Arc::new(a))
    }

    // ── Test 1: clean request → Allow ───────────────────────────────
    #[tokio::test]
    async fn arbiter_clean_request_returns_allow() {
        let adapter = make_adapter(true).await;
        let mut ctx = RequestCtx::new();
        // No headers / body → Aho-Corasick finds no matches → Allow.
        let v = adapter.analyze(&mut ctx).await;
        assert!(
            matches!(v, EngineVerdict::Allow { .. }),
            "expected Allow for clean empty request, got {v:?}"
        );
    }

    // ── Test 2: disabled engine → Allow (engine short-circuits) ─────
    #[tokio::test]
    async fn arbiter_disabled_engine_returns_allow() {
        let adapter = make_adapter(false).await;
        let mut ctx = RequestCtx::new();
        let v = adapter.analyze(&mut ctx).await;
        assert!(
            matches!(v, EngineVerdict::Allow { .. }),
            "expected Allow when engine disabled, got {v:?}"
        );
    }

    // ── Test 3: engine not ready → Skipped ──────────────────────────
    #[tokio::test]
    async fn arbiter_not_ready_returns_skipped() {
        let cfg = make_config(true, 5, false);
        // init() NOT called → is_ready() == false
        let a = Arbiter::new(cfg);
        let adapter = ArbiterAdapter::new(Arc::new(a));
        let mut ctx = RequestCtx::new();
        let v = adapter.analyze(&mut ctx).await;
        assert!(
            matches!(v, EngineVerdict::Skipped),
            "expected Skipped when not ready, got {v:?}"
        );
    }

    // ── Test 4: decision mapping — Deny ─────────────────────────────
    #[test]
    fn decision_deny_maps_to_engine_deny() {
        use armageddon_common::decision::{Decision, Severity};
        let d = Decision::deny("ARBITER", "ARBITER-WAF", "WAF anomaly score 10", Severity::High, 200);
        let v = decision_to_verdict(d);
        match v {
            EngineVerdict::Deny { score, reason } => {
                assert!((score - 1.0).abs() < f32::EPSILON, "deny confidence=1.0 → score 1.0");
                assert!(reason.contains("WAF anomaly"));
            }
            other => panic!("expected Deny, got {other:?}"),
        }
    }

    // ── Test 5: decision mapping — Flag (learning mode / below threshold) ──
    #[test]
    fn decision_flag_maps_to_allow_with_partial_score() {
        use armageddon_common::decision::{Decision, Severity};
        // confidence = 0.4 (anomaly_score / threshold ratio)
        let d = Decision::flag("ARBITER", "ARBITER-FLAG", "WAF patterns detected", Severity::Low, 0.4, 50);
        let v = decision_to_verdict(d);
        match v {
            EngineVerdict::Allow { score } => {
                assert!((score - 0.4).abs() < 0.001, "flag score should be 0.4, got {score}");
            }
            other => panic!("expected Allow (flag→defer), got {other:?}"),
        }
    }

    // ── Test 6: baseline vs burst — clean baseline stays below threshold ─
    #[tokio::test]
    async fn arbiter_baseline_stays_below_threshold() {
        // With no CRS rules loaded (crs_path=/dev/null), even a burst of
        // requests with complex URIs cannot trigger a deny.
        let adapter = make_adapter(true).await;
        for _ in 0..5 {
            let mut ctx = RequestCtx::new();
            let v = adapter.analyze(&mut ctx).await;
            assert!(
                matches!(v, EngineVerdict::Allow { .. }),
                "baseline burst should not trigger deny with no rules loaded"
            );
        }
    }
}

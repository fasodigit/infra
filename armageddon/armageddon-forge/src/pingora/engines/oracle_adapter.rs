// SPDX-License-Identifier: AGPL-3.0-or-later
//! ORACLE (ONNX anomaly detection, 22-feature model) adapter.
//!
//! # Design
//!
//! Wraps [`armageddon_oracle::Oracle`] — an ML-based anomaly scorer that
//! extracts 22 features from each [`RequestContext`] and runs them
//! through an ONNX model.  The current ONNX loader is a stub (the
//! `onnxruntime` wiring is TODO in `armageddon-oracle` itself); the
//! feature extractor is fully implemented and produces a deterministic
//! 22-element vector.
//!
//! # OTEL propagation
//!
//! The adapter reads `ctx.trace_id` and `ctx.span_id` (populated by
//! the M1 `OtelFilter`) and records them as `tracing` span attributes
//! so the Oracle prediction appears in the distributed trace.
//!
//! For full OTLP export (Tempo/Jaeger), wire
//! `tracing-opentelemetry` at server startup — that is a M6 cutover
//! task.  Until then, the `tracing::info_span!` with explicit IDs
//! gives structured-log coverage.
//!
//! # Failure modes
//!
//! * **Engine not ready**: `Skipped`.
//! * **Inspect error**: `Skipped`, logged at `warn`.
//! * **Pipeline timeout** (25 ms): handled by `FuturesUnordered` drop.
//!   25 ms is generous because the current ONNX model is a mock; tighten
//!   once a real model is loaded.
//!
//! # Determinism guarantee
//!
//! The feature extractor is deterministic for the same `RequestContext`
//! inputs — no randomness, no clock reads.  The ONNX model (once real)
//! is a pure function of its inputs.  Tests can therefore use fixed
//! inputs and assert exact scores.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use armageddon_common::decision::Verdict;
use armageddon_common::engine::SecurityEngine;
use armageddon_oracle::Oracle;

use super::aegis_adapter::request_context_from_ctx;
use super::pipeline::{EngineAdapter, EngineVerdict};
use crate::pingora::ctx::RequestCtx;

/// Pipeline adapter wrapping an initialised [`Oracle`] engine.
pub struct OracleAdapter {
    oracle: Arc<Oracle>,
}

impl OracleAdapter {
    /// Wrap an already-initialised [`Oracle`] instance.
    ///
    /// The caller must have called `Oracle::init().await` before
    /// constructing this adapter.
    pub fn new(oracle: Arc<Oracle>) -> Self {
        Self { oracle }
    }
}

#[async_trait]
impl EngineAdapter for OracleAdapter {
    fn name(&self) -> &'static str {
        "oracle"
    }

    async fn analyze(&self, ctx: &mut RequestCtx) -> EngineVerdict {
        if !self.oracle.is_ready() {
            tracing::debug!("oracle adapter: engine not ready; skipping");
            return EngineVerdict::Skipped;
        }

        // Record trace context as structured log fields so the Oracle
        // prediction is correlated with the upstream distributed trace.
        // We avoid `tracing::Span::enter()` across `.await` points
        // because `EnteredSpan` is not `Send`; use `in_scope` for the
        // synchronous parts only, and plain field annotations here.
        tracing::debug!(
            trace_id   = %ctx.trace_id,
            span_id    = %ctx.span_id,
            request_id = %ctx.request_id,
            "oracle.analyze: building request context"
        );

        let req_ctx = request_context_from_ctx(ctx);

        match self.oracle.inspect(&req_ctx).await {
            Ok(decision) => {
                tracing::debug!(
                    verdict  = ?decision.verdict,
                    confidence = decision.confidence,
                    "oracle decision"
                );
                decision_to_verdict(decision)
            }
            Err(e) => {
                tracing::warn!(error = %e, "oracle inspect failed; treating as Skipped");
                EngineVerdict::Skipped
            }
        }
    }

    /// 25 ms: generous for the current mock ONNX loader; tighten once a
    /// real model is wired (expected p99 < 5 ms on CPU).
    fn timeout(&self) -> Duration {
        Duration::from_millis(25)
    }
}

/// Map a [`armageddon_common::decision::Decision`] → [`EngineVerdict`].
///
/// ORACLE returns `Flag` when the anomaly score exceeds the threshold
/// (not `Deny` — it defers to NEXUS).  We map `Flag` to
/// `Allow { score: confidence }` so the pipeline aggregate can still
/// trip the deny-threshold if multiple engines flag the same request.
fn decision_to_verdict(d: armageddon_common::decision::Decision) -> EngineVerdict {
    match d.verdict {
        Verdict::Allow => EngineVerdict::Allow {
            score: clamp01(1.0 - d.confidence as f32),
        },
        Verdict::Deny => EngineVerdict::Deny {
            score: clamp01(d.confidence as f32),
            reason: d.description,
        },
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
    use armageddon_config::security::OracleConfig;

    fn make_oracle_config(enabled: bool, threshold: f64) -> OracleConfig {
        OracleConfig {
            enabled,
            // /dev/null → OnnxModel::predict returns a deterministic stub score.
            model_path: "/dev/null".to_string(),
            feature_count: 22,
            anomaly_threshold: threshold,
            prompt_injection_threshold: 0.8,
        }
    }

    async fn make_adapter(enabled: bool, threshold: f64) -> OracleAdapter {
        let cfg = make_oracle_config(enabled, threshold);
        let mut o = Oracle::new(cfg);
        o.init().await.expect("oracle init");
        OracleAdapter::new(Arc::new(o))
    }

    // ── Test 1: engine not ready → Skipped ──────────────────────────
    #[tokio::test]
    async fn oracle_not_ready_returns_skipped() {
        let cfg = make_oracle_config(true, 0.5);
        // init() NOT called
        let o = Oracle::new(cfg);
        let adapter = OracleAdapter::new(Arc::new(o));
        let mut ctx = RequestCtx::new();
        let v = adapter.analyze(&mut ctx).await;
        assert!(
            matches!(v, EngineVerdict::Skipped),
            "expected Skipped when not ready, got {v:?}"
        );
    }

    // ── Test 2: disabled engine → Allow (Oracle short-circuits) ─────
    #[tokio::test]
    async fn oracle_disabled_returns_allow() {
        let adapter = make_adapter(false, 0.5).await;
        let mut ctx = RequestCtx::new();
        let v = adapter.analyze(&mut ctx).await;
        assert!(
            matches!(v, EngineVerdict::Allow { .. }),
            "expected Allow when disabled, got {v:?}"
        );
    }

    // ── Test 3: enabled + clean request → deterministic result ──────
    //
    // The stub ONNX model returns a fixed score (0.0 for zero-feature
    // input).  With a high threshold this should be Allow.
    #[tokio::test]
    async fn oracle_clean_request_with_high_threshold_returns_allow() {
        // threshold=1.0 → no score from the stub model can exceed it.
        let adapter = make_adapter(true, 1.0).await;
        let mut ctx = RequestCtx::new();
        let v = adapter.analyze(&mut ctx).await;
        assert!(
            matches!(v, EngineVerdict::Allow { .. }),
            "clean request with threshold=1.0 should be Allow, got {v:?}"
        );
    }

    // ── Test 4: OTEL trace_id / span_id are preserved ───────────────
    //
    // The adapter must not corrupt ctx.trace_id / ctx.span_id during analysis.
    #[tokio::test]
    async fn oracle_preserves_otel_context() {
        let adapter = make_adapter(true, 1.0).await;
        let mut ctx = RequestCtx::new();
        ctx.trace_id = "4bf92f3577b34da6a3ce929d0e0e4736".to_string();
        ctx.span_id  = "00f067aa0ba902b7".to_string();
        let _v = adapter.analyze(&mut ctx).await;
        assert_eq!(ctx.trace_id, "4bf92f3577b34da6a3ce929d0e0e4736");
        assert_eq!(ctx.span_id, "00f067aa0ba902b7");
    }

    // ── Test 5: feature extraction determinism ───────────────────────
    //
    // Running the same ctx twice must produce the same EngineVerdict
    // (no internal state mutation between calls).
    #[tokio::test]
    async fn oracle_feature_extraction_is_deterministic() {
        let adapter = make_adapter(true, 1.0).await;
        let mut ctx1 = RequestCtx::new();
        ctx1.user_id = Some("user-42".to_string());
        ctx1.tenant_id = Some("tenant-bf".to_string());
        // Clone ctx1 to get an identical second ctx.
        let mut ctx2 = ctx1.clone();
        // Give ctx2 a different request_id (as new_ctx would) to ensure
        // we're not accidentally caching on request_id.
        ctx2.request_id = uuid::Uuid::new_v4().to_string();

        let v1 = adapter.analyze(&mut ctx1).await;
        let v2 = adapter.analyze(&mut ctx2).await;

        // Both verdicts must be the same variant and same score.
        match (v1, v2) {
            (EngineVerdict::Allow { score: s1 }, EngineVerdict::Allow { score: s2 }) => {
                assert!((s1 - s2).abs() < f32::EPSILON, "scores must match: {s1} vs {s2}");
            }
            (EngineVerdict::Skipped, EngineVerdict::Skipped) => {}
            (a, b) => panic!("verdicts differ: {a:?} vs {b:?}"),
        }
    }

    // ── Test 6: decision_to_verdict — Flag maps to Allow with score ──
    #[test]
    fn decision_flag_maps_to_allow() {
        use armageddon_common::decision::{Decision, Severity};
        let d = Decision::flag(
            "ORACLE",
            "ORACLE-ANOMALY-001",
            "Anomaly score 0.7500 exceeds threshold",
            Severity::High,
            0.75,
            100,
        );
        let v = decision_to_verdict(d);
        match v {
            EngineVerdict::Allow { score } => {
                assert!((score - 0.75).abs() < 0.001, "expected score≈0.75, got {score}");
            }
            other => panic!("expected Allow (flag→defer), got {other:?}"),
        }
    }
}

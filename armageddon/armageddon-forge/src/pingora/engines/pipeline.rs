// SPDX-License-Identifier: AGPL-3.0-or-later
//! Security-engine pipeline orchestrator (M3 · issue #104).
//!
//! # Design
//!
//! The pipeline fans out the incoming request to every registered
//! [`EngineAdapter`] in parallel, each running under its own timeout.
//! Verdicts are collected as they arrive:
//!
//! * `Allow { score }` → contributes to the aggregate score (`max`).
//! * `Deny { .. }`     → short-circuits the pipeline.  Remaining futures
//!                       are dropped (Pingora requests finish fast).
//! * `Skipped`         → engine unavailable, timed-out, or opted out;
//!                       contributes `0.0`.
//!
//! # Score semantics
//!
//! Aggregation is **max of individual scores**: the most alarmed engine
//! wins.  If any individual engine's adapter classifies its score into
//! the WAF or AI bucket (by name — `SENTINEL` / `ARBITER` → `waf_score`,
//! `ORACLE` / `AI` → `ai_score`), [`RequestCtx`] is updated so the rest
//! of the filter chain can read the per-bucket numbers without
//! re-introspecting the pipeline.
//!
//! # Concurrency primitive
//!
//! [`futures_util::stream::FuturesUnordered`] is used rather than
//! [`futures_util::future::join_all`] because it supports **early exit**
//! (drop the stream → all pending futures are cancelled), which is the
//! whole point of the `Deny` short-circuit.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures_util::stream::{FuturesUnordered, StreamExt};

use crate::pingora::ctx::RequestCtx;

/// Verdict produced by a single [`EngineAdapter::analyze`] call.
#[derive(Debug, Clone)]
pub enum EngineVerdict {
    /// Engine ran and considered the request safe (score ∈ [0.0, 1.0]).
    Allow { score: f32 },
    /// Engine demands a hard block.
    Deny { score: f32, reason: String },
    /// Engine unavailable, disabled, or timed-out.  Does not count.
    Skipped,
}

impl EngineVerdict {
    /// Contribution of this verdict to the aggregate score.  `Skipped`
    /// contributes `0.0` rather than being excluded from the max so that
    /// an all-skipped pipeline yields `0.0` (allow).
    fn score_for_aggregate(&self) -> f32 {
        match self {
            EngineVerdict::Allow { score } | EngineVerdict::Deny { score, .. } => *score,
            EngineVerdict::Skipped => 0.0,
        }
    }
}

/// Adapter that wraps one security engine (SENTINEL, ARBITER, …) behind
/// the pipeline's uniform interface.
#[async_trait]
pub trait EngineAdapter: Send + Sync + 'static {
    /// Short stable identifier used for score bucketing and metrics
    /// (`"sentinel"`, `"arbiter"`, `"oracle"`, `"aegis"`, `"nexus"`,
    /// `"ai"`, `"wasm"`).
    fn name(&self) -> &'static str;

    /// Inspect the request and return a verdict.
    async fn analyze(&self, ctx: &mut RequestCtx) -> EngineVerdict;

    /// Hard timeout for [`Self::analyze`].  When exceeded, the pipeline
    /// treats the engine as [`EngineVerdict::Skipped`].  Default 5 ms.
    fn timeout(&self) -> Duration {
        Duration::from_millis(5)
    }
}

/// Top-level verdict returned by [`Pipeline::evaluate`].
#[derive(Debug, Clone)]
pub enum PipelineVerdict {
    /// All engines either allowed or skipped; the aggregate score stayed
    /// below the deny threshold.
    Allow { aggregate_score: f32 },
    /// One engine returned `Deny`, or the aggregate crossed the
    /// configured deny threshold.
    Deny {
        reason: String,
        engine: &'static str,
        score: f32,
    },
}

/// Fan-out orchestrator for the registered security engines.
pub struct Pipeline {
    engines: Vec<Arc<dyn EngineAdapter>>,
    deny_threshold: f32,
}

impl Pipeline {
    /// Build an empty pipeline with the configured aggregate-score
    /// threshold.  When the max engine score is `>= deny_threshold`, the
    /// pipeline returns `Deny` even if no individual engine explicitly
    /// denied.
    pub fn new(deny_threshold: f32) -> Self {
        Self {
            engines: Vec::new(),
            deny_threshold,
        }
    }

    /// Register an engine.  Engines are evaluated in parallel, so order
    /// is irrelevant for correctness but affects tie-breaking when two
    /// engines report identical scores.
    pub fn add(&mut self, engine: Arc<dyn EngineAdapter>) {
        self.engines.push(engine);
    }

    /// Number of engines currently registered (useful for metrics).
    pub fn len(&self) -> usize {
        self.engines.len()
    }

    /// `true` when no engines have been registered.
    pub fn is_empty(&self) -> bool {
        self.engines.is_empty()
    }

    /// Evaluate all engines concurrently.
    ///
    /// Returns as soon as any engine returns [`EngineVerdict::Deny`], or
    /// when all engines have settled.  Updates
    /// [`RequestCtx::waf_score`] / [`RequestCtx::ai_score`] based on the
    /// name of each reporting engine.
    pub async fn evaluate(&self, ctx: &mut RequestCtx) -> PipelineVerdict {
        if self.engines.is_empty() {
            return PipelineVerdict::Allow {
                aggregate_score: 0.0,
            };
        }

        // Clone the ctx once per engine so the futures are independent
        // and the adapter may populate it freely.  After the fan-out
        // finishes (or short-circuits) the winning / aggregate scores
        // are stamped back onto the caller's `ctx`.
        //
        // This also sidesteps the `&mut` aliasing problem: we cannot
        // hand out N concurrent `&mut RequestCtx` references.
        let mut futs = FuturesUnordered::new();
        for engine in &self.engines {
            let e = Arc::clone(engine);
            let mut local_ctx = ctx.clone();
            let timeout = e.timeout();
            futs.push(async move {
                let name = e.name();
                let verdict = match tokio::time::timeout(timeout, e.analyze(&mut local_ctx)).await {
                    Ok(v) => v,
                    Err(_elapsed) => {
                        tracing::warn!(engine = name, ?timeout, "engine timed out; treating as Skipped");
                        EngineVerdict::Skipped
                    }
                };
                (name, verdict, local_ctx)
            });
        }

        let mut aggregate: f32 = 0.0;
        let mut top_engine: &'static str = "";
        // Per-bucket bookkeeping so we stamp the caller's ctx once at
        // the end and don't fight over `waf_score` / `ai_score`.
        let mut waf_score: f32 = 0.0;
        let mut ai_score: f32 = 0.0;

        while let Some((name, verdict, _local_ctx)) = futs.next().await {
            // Track the per-bucket maxima by engine name.
            let score = verdict.score_for_aggregate();
            match name {
                "sentinel" | "arbiter" => {
                    if score > waf_score {
                        waf_score = score;
                    }
                }
                "oracle" | "ai" => {
                    if score > ai_score {
                        ai_score = score;
                    }
                }
                _ => {}
            }

            // Short-circuit on explicit deny.  We drop `futs` so the
            // remaining engines are cancelled — see `FuturesUnordered`
            // docs: dropping the stream drops the pending tasks.
            if let EngineVerdict::Deny { score, reason } = verdict {
                ctx.waf_score = ctx.waf_score.max(waf_score);
                ctx.ai_score = ctx.ai_score.max(ai_score);
                return PipelineVerdict::Deny {
                    reason,
                    engine: name,
                    score,
                };
            }

            if score > aggregate {
                aggregate = score;
                top_engine = name;
            }
        }

        ctx.waf_score = ctx.waf_score.max(waf_score);
        ctx.ai_score = ctx.ai_score.max(ai_score);

        if aggregate >= self.deny_threshold && self.deny_threshold > 0.0 {
            PipelineVerdict::Deny {
                reason: format!("aggregate score {aggregate:.3} >= threshold {:.3}", self.deny_threshold),
                engine: top_engine,
                score: aggregate,
            }
        } else {
            PipelineVerdict::Allow {
                aggregate_score: aggregate,
            }
        }
    }
}

// ── tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Fixed-verdict adapter for deterministic tests.
    struct FixedAdapter {
        name: &'static str,
        verdict: EngineVerdict,
        timeout: Duration,
    }

    impl FixedAdapter {
        fn allow(name: &'static str, score: f32) -> Arc<Self> {
            Arc::new(Self {
                name,
                verdict: EngineVerdict::Allow { score },
                timeout: Duration::from_millis(50),
            })
        }
        fn deny(name: &'static str, score: f32, reason: &str) -> Arc<Self> {
            Arc::new(Self {
                name,
                verdict: EngineVerdict::Deny {
                    score,
                    reason: reason.to_string(),
                },
                timeout: Duration::from_millis(50),
            })
        }
    }

    #[async_trait]
    impl EngineAdapter for FixedAdapter {
        fn name(&self) -> &'static str {
            self.name
        }
        async fn analyze(&self, _ctx: &mut RequestCtx) -> EngineVerdict {
            self.verdict.clone()
        }
        fn timeout(&self) -> Duration {
            self.timeout
        }
    }

    /// Adapter that blocks past its own timeout → pipeline must treat it
    /// as Skipped.
    struct SlowAdapter {
        name: &'static str,
        sleep: Duration,
        timeout: Duration,
        hit: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl EngineAdapter for SlowAdapter {
        fn name(&self) -> &'static str {
            self.name
        }
        async fn analyze(&self, _ctx: &mut RequestCtx) -> EngineVerdict {
            tokio::time::sleep(self.sleep).await;
            self.hit.fetch_add(1, Ordering::SeqCst);
            EngineVerdict::Allow { score: 0.9 } // should never be observed
        }
        fn timeout(&self) -> Duration {
            self.timeout
        }
    }

    #[tokio::test]
    async fn empty_pipeline_allows_with_zero_score() {
        let p = Pipeline::new(0.8);
        let mut ctx = RequestCtx::new();
        let v = p.evaluate(&mut ctx).await;
        match v {
            PipelineVerdict::Allow { aggregate_score } => assert_eq!(aggregate_score, 0.0),
            other => panic!("expected Allow, got {other:?}"),
        }
        assert_eq!(ctx.waf_score, 0.0);
        assert_eq!(ctx.ai_score, 0.0);
    }

    #[tokio::test]
    async fn all_engines_allow_returns_max_score() {
        let mut p = Pipeline::new(0.9);
        p.add(FixedAdapter::allow("sentinel", 0.1));
        p.add(FixedAdapter::allow("arbiter", 0.5));
        p.add(FixedAdapter::allow("aegis", 0.2));
        let mut ctx = RequestCtx::new();
        let v = p.evaluate(&mut ctx).await;
        match v {
            PipelineVerdict::Allow { aggregate_score } => {
                assert!((aggregate_score - 0.5).abs() < f32::EPSILON, "got {aggregate_score}");
            }
            other => panic!("expected Allow, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn any_deny_short_circuits_pipeline() {
        let mut p = Pipeline::new(0.9);
        p.add(FixedAdapter::allow("sentinel", 0.2));
        p.add(FixedAdapter::deny("arbiter", 0.99, "SQLi detected"));
        p.add(FixedAdapter::allow("aegis", 0.1));
        let mut ctx = RequestCtx::new();
        let v = p.evaluate(&mut ctx).await;
        match v {
            PipelineVerdict::Deny { reason, engine, score } => {
                assert_eq!(engine, "arbiter");
                assert!(reason.contains("SQLi"));
                assert!((score - 0.99).abs() < 0.001);
            }
            other => panic!("expected Deny, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn timeout_counts_as_skipped_not_allow() {
        let hit = Arc::new(AtomicUsize::new(0));
        let mut p = Pipeline::new(0.9);
        p.add(Arc::new(SlowAdapter {
            name: "oracle",
            sleep: Duration::from_millis(200),
            timeout: Duration::from_millis(10),
            hit: Arc::clone(&hit),
        }));
        p.add(FixedAdapter::allow("aegis", 0.3));
        let mut ctx = RequestCtx::new();
        let v = p.evaluate(&mut ctx).await;
        match v {
            PipelineVerdict::Allow { aggregate_score } => {
                // Only aegis contributed 0.3; oracle timed-out → 0.0.
                assert!((aggregate_score - 0.3).abs() < f32::EPSILON, "got {aggregate_score}");
            }
            other => panic!("expected Allow, got {other:?}"),
        }
        // oracle's `ai_score` bucket must not have been stamped with 0.9.
        assert_eq!(ctx.ai_score, 0.0);
    }

    #[tokio::test]
    async fn pipeline_updates_ctx_scores() {
        let mut p = Pipeline::new(0.99);
        p.add(FixedAdapter::allow("sentinel", 0.42));
        p.add(FixedAdapter::allow("arbiter", 0.33));
        p.add(FixedAdapter::allow("oracle", 0.77));
        p.add(FixedAdapter::allow("ai", 0.55));
        p.add(FixedAdapter::allow("aegis", 0.10));
        let mut ctx = RequestCtx::new();
        let _ = p.evaluate(&mut ctx).await;
        // WAF bucket = max(sentinel, arbiter) = 0.42
        assert!((ctx.waf_score - 0.42).abs() < f32::EPSILON, "waf={}", ctx.waf_score);
        // AI  bucket = max(oracle, ai)      = 0.77
        assert!((ctx.ai_score - 0.77).abs() < f32::EPSILON, "ai={}", ctx.ai_score);
    }

    #[tokio::test]
    async fn aggregate_over_threshold_converts_allow_to_deny() {
        let mut p = Pipeline::new(0.5);
        p.add(FixedAdapter::allow("sentinel", 0.6));
        let mut ctx = RequestCtx::new();
        let v = p.evaluate(&mut ctx).await;
        match v {
            PipelineVerdict::Deny { engine, score, .. } => {
                assert_eq!(engine, "sentinel");
                assert!((score - 0.6).abs() < f32::EPSILON);
            }
            other => panic!("expected Deny by aggregate threshold, got {other:?}"),
        }
    }
}

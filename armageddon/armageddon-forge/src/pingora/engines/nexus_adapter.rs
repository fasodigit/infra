// SPDX-License-Identifier: AGPL-3.0-or-later
//! NEXUS (brain — composite-score aggregator) adapter.
//!
//! # Role
//!
//! NEXUS is the final stage of the security engine pipeline.  While the
//! other adapters (SENTINEL, ARBITER, ORACLE, AEGIS) produce independent
//! `EngineVerdict` values, NEXUS **fuses** them into a single composite
//! verdict using weighted scoring, multi-vector correlation, and
//! configurable block/challenge thresholds.
//!
//! # Integration with the pipeline
//!
//! The Pingora [`Pipeline`] runs all engines concurrently via
//! `FuturesUnordered`.  NEXUS is registered last (by convention) and
//! reads from `RequestCtx` the aggregate scores already stamped by the
//! pipeline:
//!
//! * `ctx.waf_score` — max of SENTINEL + ARBITER scores.
//! * `ctx.ai_score`  — max of ORACLE + AI scores.
//!
//! It constructs synthetic [`Decision`] objects from these scores
//! (using the respective engine names expected by `CompositeScorer`)
//! and passes them to [`armageddon_nexus::Nexus::aggregate`].
//!
//! **Important**: because the pipeline runs all engines in parallel,
//! `ctx.waf_score` and `ctx.ai_score` may not yet be stamped when
//! NEXUS's own future runs.  To avoid a race, the pipeline caller
//! should register NEXUS as a *post-pipeline* step rather than in the
//! `FuturesUnordered` fan-out.  See `pipeline.rs` — the
//! `evaluate_with_nexus` helper handles this.
//!
//! # Failure modes
//!
//! * **Nexus not configured** (`nexus` is `None`): `Skipped`.
//! * **All upstream engines skipped** (scores both 0.0): returns
//!   `Allow { score: 0.0 }` — no data, assume safe.
//! * **Aggregate score ≥ block_threshold**: `Deny`.
//! * **Aggregate score ≥ challenge_threshold**: `Allow { score }` —
//!   the challenge is communicated through the pipeline verdict's
//!   aggregate score, not a hard Deny.  The gateway layer can
//!   inspect `waf_score` to decide whether to issue a challenge.
//!
//! # Tests
//!
//! See the module-level `tests` block for aggregation logic,
//! tie-breaking (multi-vector boost), and weighted combination.

use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use armageddon_common::context::RequestContext;
use armageddon_common::decision::{Action, Decision};
use armageddon_common::types::{ConnectionInfo, HttpRequest, HttpVersion, Protocol};
use armageddon_nexus::{FinalVerdict, Nexus};

use super::pipeline::{EngineAdapter, EngineVerdict};
use crate::pingora::ctx::RequestCtx;

/// Pipeline adapter wrapping [`Nexus`].
///
/// Registered **after** the other five adapters; reads
/// `ctx.waf_score` / `ctx.ai_score` that the pipeline has already
/// stamped from the SENTINEL / ARBITER / ORACLE runs.
pub struct NexusAdapter {
    nexus: Arc<Nexus>,
}

impl NexusAdapter {
    /// Wrap an initialised [`Nexus`] instance.
    pub fn new(nexus: Arc<Nexus>) -> Self {
        Self { nexus }
    }
}

#[async_trait]
impl EngineAdapter for NexusAdapter {
    fn name(&self) -> &'static str {
        "nexus"
    }

    async fn analyze(&self, ctx: &mut RequestCtx) -> EngineVerdict {
        // Build synthetic decisions from the per-bucket scores already
        // stamped by the pipeline.  NEXUS sees "virtual" engines whose
        // names match the ENGINE_WEIGHTS table in scorer.rs.
        let decisions = synthetic_decisions_from_ctx(ctx);

        // Build a minimal RequestContext for NEXUS (it only reads
        // `request_id` from the context for the FinalVerdict).
        let req_ctx = minimal_request_ctx(ctx);

        let verdict: FinalVerdict = self.nexus.aggregate(&req_ctx, &decisions);
        tracing::debug!(
            action   = ?verdict.action,
            score    = verdict.score,
            reason   = %verdict.reason,
            "nexus aggregated verdict"
        );

        final_verdict_to_engine_verdict(verdict)
    }

    /// NEXUS is CPU-bound (no I/O); 10 ms is sufficient.
    fn timeout(&self) -> Duration {
        Duration::from_millis(10)
    }
}

/// Build a minimal [`RequestContext`] suitable for NEXUS aggregation.
///
/// NEXUS only needs `request_id` from the context (used in
/// `FinalVerdict.request_id`); all other fields are zero-valued.
fn minimal_request_ctx(ctx: &RequestCtx) -> RequestContext {
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
    if let Ok(uuid) = uuid::Uuid::parse_str(&ctx.request_id) {
        rc.request_id = uuid;
    }
    rc
}

/// Synthesise [`Decision`] objects from the per-bucket scores in ctx.
///
/// We create one `Decision` per engine name that `CompositeScorer` knows
/// about, using the bucket scores already stamped by the pipeline:
///
/// * SENTINEL + ARBITER → `ctx.waf_score` (each gets the same value;
///   the scorer weights them independently).
/// * ORACLE              → `ctx.ai_score`.
/// * AEGIS               → no dedicated bucket; use average of waf+ai
///   as a proxy.
///
/// A score of 0.0 → `Verdict::Allow`; score > 0.0 → `Verdict::Flag`
/// with `confidence = score`.
fn synthetic_decisions_from_ctx(ctx: &RequestCtx) -> Vec<Decision> {
    let waf = ctx.waf_score as f64;
    let ai = ctx.ai_score as f64;
    let aegis_proxy = ((waf + ai) / 2.0).min(1.0);

    vec![
        score_to_decision("SENTINEL", waf),
        score_to_decision("ARBITER", waf),
        score_to_decision("ORACLE", ai),
        score_to_decision("AEGIS", aegis_proxy),
    ]
}

/// Convert a score in [0.0, 1.0] to a synthetic [`Decision`].
///
/// The severity is chosen so that `confidence × severity_multiplier`
/// in [`CompositeScorer`] preserves the pipeline score faithfully:
///
/// * score ≥ 1.0              → `Deny` (confidence=1.0, multiplier N/A)
/// * 0.8 ≤ score < 1.0        → `Flag` / `Critical`   (multiplier 1.0)
/// * 0.5 ≤ score < 0.8        → `Flag` / `High`        (multiplier 0.8)
/// * 0.0 < score < 0.5        → `Flag` / `Medium`      (multiplier 0.5)
/// * score ≤ 0.0              → `Allow`
///
/// Using `Critical` severity for high scores means the weighted
/// composite accurately reflects a near-block situation from upstream.
fn score_to_decision(engine: &str, score: f64) -> Decision {
    use armageddon_common::decision::Severity;
    if score <= 0.0 {
        Decision::allow(engine, 0)
    } else if score >= 1.0 {
        Decision::deny(engine, "NEXUS-SYNTH-DENY", "Synthetic deny from score=1.0", Severity::Critical, 0)
    } else {
        // Choose severity so that `confidence × multiplier ≈ score`
        // → multiplier(Critical)=1.0, multiplier(High)=0.8, multiplier(Medium)=0.5
        let (severity, confidence) = if score >= 0.8 {
            (Severity::Critical, score)            // confidence × 1.0 = score
        } else if score >= 0.5 {
            (Severity::High, score / 0.8)          // confidence × 0.8 ≈ score
        } else {
            (Severity::Medium, score / 0.5)        // confidence × 0.5 ≈ score
        };
        Decision::flag(
            engine,
            "NEXUS-SYNTH-FLAG",
            "Synthetic flag from non-zero score",
            severity,
            confidence.min(1.0),
            0,
        )
    }
}

/// Convert a [`FinalVerdict`] from NEXUS to an [`EngineVerdict`].
fn final_verdict_to_engine_verdict(v: FinalVerdict) -> EngineVerdict {
    match v.action {
        Action::Block => EngineVerdict::Deny {
            score: clamp01(v.score as f32),
            reason: v.reason,
        },
        // Challenge / Throttle / LogOnly → allow but surface the score
        // so the gateway layer can decide (inspect waf_score / ai_score).
        Action::Challenge | Action::Throttle | Action::LogOnly => EngineVerdict::Allow {
            score: clamp01(v.score as f32),
        },
        Action::Forward => EngineVerdict::Allow {
            score: clamp01(v.score as f32),
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
    use armageddon_common::decision::Verdict;
    use armageddon_config::security::NexusConfig;

    fn make_nexus(block: f64, challenge: f64) -> Arc<Nexus> {
        let cfg = NexusConfig {
            block_threshold: block,
            challenge_threshold: challenge,
            correlation_window_ms: 1000,
        };
        Arc::new(Nexus::new(cfg, "localhost", 6380))
    }

    fn make_adapter(block: f64, challenge: f64) -> NexusAdapter {
        NexusAdapter::new(make_nexus(block, challenge))
    }

    // ── Test 1: all scores zero → Forward → Allow(0.0) ───────────────
    #[tokio::test]
    async fn nexus_all_clear_returns_allow_zero() {
        let adapter = make_adapter(0.8, 0.5);
        let mut ctx = RequestCtx::new();
        // waf_score and ai_score both default to 0.0
        let v = adapter.analyze(&mut ctx).await;
        match v {
            EngineVerdict::Allow { score } => {
                assert_eq!(score, 0.0, "all-clear should yield score 0.0, got {score}");
            }
            other => panic!("expected Allow(0.0), got {other:?}"),
        }
    }

    // ── Test 2: high waf_score → Block → Deny ────────────────────────
    #[tokio::test]
    async fn nexus_high_waf_score_triggers_deny() {
        // block_threshold = 0.7; waf_score = 0.95 → synthetic decisions
        // will flag SENTINEL + ARBITER at 0.95 confidence → score >> 0.7
        let adapter = make_adapter(0.7, 0.4);
        let mut ctx = RequestCtx::new();
        ctx.waf_score = 0.95;
        let v = adapter.analyze(&mut ctx).await;
        assert!(
            matches!(v, EngineVerdict::Deny { .. }),
            "waf_score=0.95 should trigger Deny via NEXUS block, got {v:?}"
        );
    }

    // ── Test 3: moderate score → Forward or Challenge → Allow ─────────
    #[tokio::test]
    async fn nexus_moderate_score_does_not_deny() {
        // block_threshold = 0.9 → moderate waf=0.3 stays below threshold
        let adapter = make_adapter(0.9, 0.5);
        let mut ctx = RequestCtx::new();
        ctx.waf_score = 0.3;
        ctx.ai_score = 0.2;
        let v = adapter.analyze(&mut ctx).await;
        assert!(
            matches!(v, EngineVerdict::Allow { .. }),
            "moderate scores should not Deny with high block_threshold, got {v:?}"
        );
    }

    // ── Test 4: multi-vector (waf + ai both flagged) → score boost ────
    #[tokio::test]
    async fn nexus_multi_vector_boosts_score() {
        // Low block_threshold; multi-vector should produce Deny even with
        // individually moderate scores.
        let adapter = make_adapter(0.4, 0.2);
        let mut ctx = RequestCtx::new();
        ctx.waf_score = 0.5; // above challenge but below block individually
        ctx.ai_score  = 0.5;
        let v = adapter.analyze(&mut ctx).await;
        // With block=0.4 and both scores=0.5, NEXUS should block.
        assert!(
            matches!(v, EngineVerdict::Deny { .. }),
            "multi-vector high scores should trigger Deny, got {v:?}"
        );
    }

    // ── Test 5: score_to_decision correctness ─────────────────────────
    #[test]
    fn score_to_decision_zero_is_allow() {
        let d = score_to_decision("SENTINEL", 0.0);
        assert_eq!(d.verdict, Verdict::Allow);
    }

    #[test]
    fn score_to_decision_one_is_deny() {
        let d = score_to_decision("ARBITER", 1.0);
        assert_eq!(d.verdict, Verdict::Deny);
    }

    #[test]
    fn score_to_decision_mid_is_flag() {
        // score=0.6 falls in [0.5, 0.8) → severity=High, confidence=0.6/0.8=0.75
        let d = score_to_decision("ORACLE", 0.6);
        assert_eq!(d.verdict, Verdict::Flag);
        // confidence×0.8 ≈ 0.6 (the original score is preserved through multiplication)
        assert!((d.confidence * 0.8 - 0.6).abs() < 1e-6, "confidence×0.8 should ≈ 0.6, got {}", d.confidence);
    }

    // ── Test 6: final_verdict_to_engine_verdict tie-breaking ──────────
    #[test]
    fn final_verdict_block_maps_to_engine_deny() {
        use armageddon_common::decision::Action;
        let fv = FinalVerdict {
            action: Action::Block,
            score: 0.95,
            reason: "blocked by NEXUS".to_string(),
            decisions: vec![],
            request_id: uuid::Uuid::new_v4(),
        };
        let v = final_verdict_to_engine_verdict(fv);
        match v {
            EngineVerdict::Deny { score, reason } => {
                assert!((score - 0.95).abs() < 0.001, "score mismatch");
                assert!(reason.contains("NEXUS"));
            }
            other => panic!("expected Deny, got {other:?}"),
        }
    }

    #[test]
    fn final_verdict_challenge_maps_to_engine_allow() {
        use armageddon_common::decision::Action;
        let fv = FinalVerdict {
            action: Action::Challenge,
            score: 0.55,
            reason: "challenge issued".to_string(),
            decisions: vec![],
            request_id: uuid::Uuid::new_v4(),
        };
        let v = final_verdict_to_engine_verdict(fv);
        assert!(
            matches!(v, EngineVerdict::Allow { .. }),
            "Challenge should map to Allow, got {v:?}"
        );
    }
}

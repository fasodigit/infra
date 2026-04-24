// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//! Multi-provider AI routing for the ARMAGEDDON pipeline (Backlog BL-3).
//!
//! [`MultiAiProvider`] wraps multiple [`AiProvider`] implementations and
//! dispatches `contextualise` calls according to a configurable
//! [`MultiStrategy`]:
//!
//! - **Fallback**: try providers in declaration order; return the first
//!   `Some(result)`.  Providers that return `None` (fail-open) are
//!   transparently skipped.
//! - **Ensemble**: broadcast to all providers, collect scores, aggregate
//!   with [`EnsembleMode::Average`], [`EnsembleMode::Max`], or
//!   [`EnsembleMode::Majority`] (>50 % agree on deny/allow).
//! - **Routed**: delegate provider selection to a [`RequestRouter`]
//!   implementation; only the selected provider is called.
//!
//! # Failure modes
//!
//! | Scenario | Behaviour |
//! |----------|-----------|
//! | No providers registered | `None` returned (fail-open) |
//! | Fallback: all fail-open | `None` returned |
//! | Ensemble: all fail-open | `None` returned |
//! | Routed: router returns unknown name | `None` returned |
//! | Routed: named provider fails | `None` returned |
//!
//! # Thread safety
//!
//! `MultiAiProvider` is `Send + Sync` because all inner providers implement
//! `AiProvider: Send + Sync + 'static` and the router implements
//! `RequestRouter: Send + Sync + 'static`.
//!
//! # Metrics (future)
//!
//! Metrics per-provider (calls, latency, selection frequency) will be added
//! in BL-4 / Vague 4 once the BL-3 API stabilises.  See
//! `docs/VAGUE-4-RECHERCHE-ROADMAP.md` item 4.

use std::sync::Arc;

use super::ai_adapter::AiProvider;

// ── EnsembleMode ─────────────────────────────────────────────────────────────

/// How to aggregate verdicts from multiple providers in `Ensemble` mode.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EnsembleMode {
    /// Arithmetic mean of all provider scores.
    Average,
    /// Maximum score across all providers (pessimistic / conservative).
    Max,
    /// `"deny"` when more than 50 % of providers return a score > 0.5,
    /// otherwise `"allow"`.  Encoded as a score of `1.0` or `0.0`.
    Majority,
}

// ── RequestRouter ─────────────────────────────────────────────────────────────

/// Selects a provider name for a given request context.
///
/// Implementations may use request attributes (path, risk score, tenant, …)
/// to pick the most appropriate provider.  The returned name must match a
/// `named` entry in [`MultiAiProvider::providers`].
///
/// # Invariant
///
/// Must be `Send + Sync + 'static` (shared across OS threads via `Arc`).
pub trait RequestRouter: Send + Sync + 'static {
    /// Return the name of the provider to call for this request.
    ///
    /// `user_id`, `path`, `score` mirror the `AiProvider::contextualise`
    /// signature so the router has the same information as the provider.
    ///
    /// Returns `None` to fall through to `None` (fail-open) when no
    /// suitable provider can be determined.
    fn route(&self, user_id: Option<&str>, path: &str, score: f32) -> Option<String>;
}

// ── MultiStrategy ─────────────────────────────────────────────────────────────

/// Strategy controlling how [`MultiAiProvider`] dispatches calls.
pub enum MultiStrategy {
    /// Try providers in order; return the first `Some` result.
    Fallback,
    /// Broadcast to all providers and aggregate scores.
    Ensemble {
        /// Aggregation mode for the collected scores.
        aggregator: EnsembleMode,
    },
    /// Route by request attributes; call only the selected provider.
    Routed {
        /// Router implementation that maps request context to a provider name.
        router: Arc<dyn RequestRouter>,
    },
}

impl std::fmt::Debug for MultiStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MultiStrategy::Fallback => write!(f, "Fallback"),
            MultiStrategy::Ensemble { aggregator } => {
                write!(f, "Ensemble({aggregator:?})")
            }
            MultiStrategy::Routed { .. } => write!(f, "Routed(..)"),
        }
    }
}

// ── Named provider entry ──────────────────────────────────────────────────────

/// A named [`AiProvider`] entry in [`MultiAiProvider`].
pub struct NamedProvider {
    /// Logical name (e.g. `"claude-primary"`, `"ollama-fallback"`).
    pub name: String,
    /// The underlying provider implementation.
    pub provider: Arc<dyn AiProvider>,
}

impl std::fmt::Debug for NamedProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NamedProvider")
            .field("name", &self.name)
            .finish()
    }
}

// ── MultiAiProvider ───────────────────────────────────────────────────────────

/// Multi-provider AI dispatcher with configurable strategy.
///
/// # Usage
///
/// ```rust,ignore
/// use armageddon_forge::pingora::engines::multi_ai_provider::{
///     MultiAiProvider, MultiStrategy, NamedProvider,
/// };
///
/// let multi = MultiAiProvider::new(
///     vec![
///         NamedProvider { name: "claude".into(), provider: Arc::new(claude_provider) },
///         NamedProvider { name: "ollama".into(), provider: Arc::new(ollama_provider) },
///     ],
///     MultiStrategy::Fallback,
/// );
/// ```
pub struct MultiAiProvider {
    /// Ordered list of named providers.  For `Fallback`, order determines
    /// priority (first provider tried first).
    pub providers: Vec<NamedProvider>,
    /// Dispatch strategy.
    pub strategy: MultiStrategy,
}

impl std::fmt::Debug for MultiAiProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MultiAiProvider")
            .field("providers", &self.providers)
            .field("strategy", &self.strategy)
            .finish()
    }
}

impl MultiAiProvider {
    /// Create a new multi-provider with the given strategy.
    pub fn new(providers: Vec<NamedProvider>, strategy: MultiStrategy) -> Self {
        Self { providers, strategy }
    }

    // ── Internal dispatch helpers ─────────────────────────────────────────

    fn dispatch_fallback(
        &self,
        user_id: Option<&str>,
        path: &str,
        score: f32,
    ) -> Option<String> {
        for named in &self.providers {
            if let Some(result) = named.provider.contextualise(user_id, path, score) {
                return Some(result);
            }
        }
        None
    }

    fn dispatch_ensemble(
        &self,
        user_id: Option<&str>,
        path: &str,
        score: f32,
        aggregator: EnsembleMode,
    ) -> Option<String> {
        let scores: Vec<f32> = self
            .providers
            .iter()
            .filter_map(|named| {
                // Try to parse the score from the contextualisation string.
                // Providers encode score as `score=X.XX ...`; fall back to
                // the raw heuristic score if parsing fails.
                named
                    .provider
                    .contextualise(user_id, path, score)
                    .and_then(|s| extract_score_from_context(&s))
            })
            .collect();

        if scores.is_empty() {
            return None;
        }

        let aggregated = match aggregator {
            EnsembleMode::Average => {
                scores.iter().sum::<f32>() / scores.len() as f32
            }
            EnsembleMode::Max => {
                scores.iter().cloned().fold(f32::NEG_INFINITY, f32::max)
            }
            EnsembleMode::Majority => {
                let deny_votes = scores.iter().filter(|&&s| s > 0.5).count();
                if deny_votes * 2 > scores.len() { 1.0 } else { 0.0 }
            }
        };

        Some(format!("score={aggregated:.4} ensemble_mode={aggregator:?} providers={}", scores.len()))
    }

    fn dispatch_routed(
        &self,
        user_id: Option<&str>,
        path: &str,
        score: f32,
        router: &Arc<dyn RequestRouter>,
    ) -> Option<String> {
        let provider_name = router.route(user_id, path, score)?;
        let named = self
            .providers
            .iter()
            .find(|p| p.name == provider_name)?;
        named.provider.contextualise(user_id, path, score)
    }
}

impl AiProvider for MultiAiProvider {
    fn contextualise(&self, user_id: Option<&str>, path: &str, score: f32) -> Option<String> {
        if self.providers.is_empty() {
            return None;
        }
        match &self.strategy {
            MultiStrategy::Fallback => self.dispatch_fallback(user_id, path, score),
            MultiStrategy::Ensemble { aggregator } => {
                self.dispatch_ensemble(user_id, path, score, *aggregator)
            }
            MultiStrategy::Routed { router } => {
                self.dispatch_routed(user_id, path, score, router)
            }
        }
    }
}

// ── Score extraction helper ───────────────────────────────────────────────────

/// Extract a `score=X.XX` float from a contextualisation string.
///
/// Providers emit strings like `"score=0.85 labels=[...] evidence=..."`.
/// This helper finds the first `score=` token and parses the float that
/// follows.  Returns `None` on parse failure (fail-open).
fn extract_score_from_context(context: &str) -> Option<f32> {
    for token in context.split_whitespace() {
        if let Some(rest) = token.strip_prefix("score=") {
            return rest.parse::<f32>().ok();
        }
    }
    None
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Test helpers ─────────────────────────────────────────────────────────

    struct FixedProvider {
        result: Option<f32>,
    }

    impl AiProvider for FixedProvider {
        fn contextualise(&self, _user_id: Option<&str>, _path: &str, _score: f32) -> Option<String> {
            self.result
                .map(|s| format!("score={s:.2} labels=[] evidence=fixed"))
        }
    }

    struct FailingProvider;

    impl AiProvider for FailingProvider {
        fn contextualise(&self, _user_id: Option<&str>, _path: &str, _score: f32) -> Option<String> {
            None // always fail-open
        }
    }

    fn named(name: &str, score: Option<f32>) -> NamedProvider {
        NamedProvider {
            name: name.to_string(),
            provider: Arc::new(FixedProvider { result: score }),
        }
    }

    fn failing(name: &str) -> NamedProvider {
        NamedProvider {
            name: name.to_string(),
            provider: Arc::new(FailingProvider),
        }
    }

    // ── Fallback strategy ─────────────────────────────────────────────────────

    /// Fallback: first provider fails → second provider succeeds → verdict returned.
    #[test]
    fn fallback_first_fails_second_succeeds() {
        let multi = MultiAiProvider::new(
            vec![failing("p1"), named("p2", Some(0.7))],
            MultiStrategy::Fallback,
        );
        let result = multi.contextualise(None, "/api", 0.5);
        assert!(result.is_some(), "expected p2 to succeed");
        assert!(result.unwrap().contains("score=0.70"), "expected p2 score");
    }

    /// Fallback: all providers fail → None.
    #[test]
    fn fallback_all_fail_returns_none() {
        let multi = MultiAiProvider::new(
            vec![failing("p1"), failing("p2")],
            MultiStrategy::Fallback,
        );
        assert!(multi.contextualise(None, "/api", 0.5).is_none());
    }

    /// Fallback: first provider succeeds → used immediately, second not called.
    #[test]
    fn fallback_first_success_used_immediately() {
        let multi = MultiAiProvider::new(
            vec![named("p1", Some(0.3)), named("p2", Some(0.9))],
            MultiStrategy::Fallback,
        );
        let result = multi.contextualise(None, "/api", 0.5);
        assert!(result.is_some());
        // p1 should be used — score is 0.30, not 0.90.
        assert!(result.unwrap().contains("score=0.30"), "expected p1 score");
    }

    // ── Ensemble: average ─────────────────────────────────────────────────────

    /// Ensemble average: scores [0.2, 0.8, 0.5] → mean 0.5.
    #[test]
    fn ensemble_average_three_providers() {
        let multi = MultiAiProvider::new(
            vec![named("p1", Some(0.2)), named("p2", Some(0.8)), named("p3", Some(0.5))],
            MultiStrategy::Ensemble { aggregator: EnsembleMode::Average },
        );
        let result = multi.contextualise(None, "/api", 0.5);
        assert!(result.is_some());
        let s = result.unwrap();
        // Extract the aggregated score.
        let agg = s.split_whitespace()
            .find_map(|t| t.strip_prefix("score=").and_then(|v| v.parse::<f32>().ok()))
            .expect("score token must be present");
        assert!(
            (agg - 0.5).abs() < 0.01,
            "expected mean ≈ 0.5, got {agg}"
        );
    }

    /// Ensemble max: scores [0.2, 0.8, 0.5] → max 0.8.
    #[test]
    fn ensemble_max_three_providers() {
        let multi = MultiAiProvider::new(
            vec![named("p1", Some(0.2)), named("p2", Some(0.8)), named("p3", Some(0.5))],
            MultiStrategy::Ensemble { aggregator: EnsembleMode::Max },
        );
        let result = multi.contextualise(None, "/api", 0.5);
        assert!(result.is_some());
        let agg = result.unwrap().split_whitespace()
            .find_map(|t| t.strip_prefix("score=").and_then(|v| v.parse::<f32>().ok()))
            .expect("score token");
        assert!(
            (agg - 0.8).abs() < 0.01,
            "expected max ≈ 0.8, got {agg}"
        );
    }

    /// Ensemble majority: 2/3 providers flag deny (score > 0.5) → score 1.0.
    #[test]
    fn ensemble_majority_deny_wins() {
        let multi = MultiAiProvider::new(
            vec![
                named("p1", Some(0.9)), // deny
                named("p2", Some(0.8)), // deny
                named("p3", Some(0.2)), // allow
            ],
            MultiStrategy::Ensemble { aggregator: EnsembleMode::Majority },
        );
        let result = multi.contextualise(None, "/api", 0.5);
        assert!(result.is_some());
        let agg = result.unwrap().split_whitespace()
            .find_map(|t| t.strip_prefix("score=").and_then(|v| v.parse::<f32>().ok()))
            .expect("score token");
        assert!(
            (agg - 1.0).abs() < 0.01,
            "majority deny: expected 1.0, got {agg}"
        );
    }

    /// Ensemble majority: 1/3 providers deny → allow (score 0.0).
    #[test]
    fn ensemble_majority_allow_wins() {
        let multi = MultiAiProvider::new(
            vec![
                named("p1", Some(0.9)), // deny
                named("p2", Some(0.2)), // allow
                named("p3", Some(0.1)), // allow
            ],
            MultiStrategy::Ensemble { aggregator: EnsembleMode::Majority },
        );
        let result = multi.contextualise(None, "/api", 0.5);
        assert!(result.is_some());
        let agg = result.unwrap().split_whitespace()
            .find_map(|t| t.strip_prefix("score=").and_then(|v| v.parse::<f32>().ok()))
            .expect("score token");
        assert!(
            (agg - 0.0).abs() < 0.01,
            "majority allow: expected 0.0, got {agg}"
        );
    }

    // ── Routed strategy ───────────────────────────────────────────────────────

    /// Router returns "claude" → only Claude provider called.
    #[test]
    fn routed_calls_only_selected_provider() {
        struct StaticRouter(String);
        impl RequestRouter for StaticRouter {
            fn route(&self, _u: Option<&str>, _p: &str, _s: f32) -> Option<String> {
                Some(self.0.clone())
            }
        }

        let multi = MultiAiProvider::new(
            vec![
                named("claude", Some(0.85)),
                named("ollama", Some(0.10)),
            ],
            MultiStrategy::Routed {
                router: Arc::new(StaticRouter("claude".into())),
            },
        );
        let result = multi.contextualise(None, "/api", 0.5);
        assert!(result.is_some());
        assert!(result.unwrap().contains("score=0.85"), "expected claude score");
    }

    /// Router returns unknown name → None (fail-open).
    #[test]
    fn routed_unknown_provider_returns_none() {
        struct StaticRouter(String);
        impl RequestRouter for StaticRouter {
            fn route(&self, _u: Option<&str>, _p: &str, _s: f32) -> Option<String> {
                Some(self.0.clone())
            }
        }

        let multi = MultiAiProvider::new(
            vec![named("claude", Some(0.85))],
            MultiStrategy::Routed {
                router: Arc::new(StaticRouter("non-existent".into())),
            },
        );
        assert!(multi.contextualise(None, "/api", 0.5).is_none());
    }

    // ── Edge cases ────────────────────────────────────────────────────────────

    /// Empty provider list → None for any strategy.
    #[test]
    fn empty_providers_returns_none() {
        let multi = MultiAiProvider::new(vec![], MultiStrategy::Fallback);
        assert!(multi.contextualise(None, "/api", 0.5).is_none());
    }

    /// score extraction helper: parses valid context strings.
    #[test]
    fn extract_score_parses_correctly() {
        let ctx = "score=0.75 labels=[\"x\"] evidence=test";
        let s = extract_score_from_context(ctx);
        assert!(s.is_some());
        assert!((s.unwrap() - 0.75).abs() < 0.001);
    }

    /// score extraction helper: returns None on missing score token.
    #[test]
    fn extract_score_missing_returns_none() {
        assert!(extract_score_from_context("no score here").is_none());
    }
}

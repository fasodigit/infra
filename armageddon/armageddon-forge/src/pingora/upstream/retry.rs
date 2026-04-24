// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Retry adapter — integrates `armageddon-retry` with the Pingora upstream
//! response filter.
//!
//! # Design
//!
//! The `armageddon-retry` crate provides a general-purpose
//! `execute_with_retry` loop with budgets, jitter, `Retry-After` header
//! support, and overall/per-try timeouts.  This module adapts that machinery
//! to the Pingora proxy model where:
//!
//! 1. **On 5xx response** (`upstream_response_filter` hook): increment the
//!    cluster's retry counter, check the budget, and — if retryable —
//!    record the intent to retry so the next call to `upstream_peer` picks
//!    a *different* endpoint via the selector.
//! 2. **On connect failure** (`fail_to_proxy` hook): same budget check, but
//!    triggered by a TCP-level error.
//!
//! ## Interaction with circuit_breaker
//!
//! Retry counters **do not** feed the circuit breaker.  The circuit breaker
//! sees each *attempt* as an independent event (already counted before the
//! retry decision is made).  This prevents double-counting.
//!
//! ## `Retry-After` header
//!
//! When the upstream response carries `Retry-After: <seconds>`, the delay
//! before the next attempt is clamped to `[policy.backoff_base, policy.backoff_cap]`
//! and overrides the normal exponential backoff.
//!
//! # Failure modes
//!
//! | Scenario | Behaviour |
//! |---|---|
//! | Non-retriable status (4xx) | `RetryDecision::NoRetry` immediately |
//! | Budget depleted | `RetryDecision::BudgetExhausted` — 503 to client |
//! | max_retries reached | `RetryDecision::Exhausted` — last upstream response forwarded |
//! | 429 with `Retry-After` | Wait the requested delay (clamped to backoff_cap) |
//! | 503 with `Retry-After` | Same as 429 |

use std::sync::Arc;
use std::time::Duration;

use tracing::{debug, warn};

use armageddon_retry::budget::RetryBudget;
use armageddon_retry::policy::{JitterMode, RetryOn, RetryPolicy};

// ── RetryDecision ─────────────────────────────────────────────────────────────

/// Outcome returned by [`PingoraRetryPolicy::should_retry`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetryDecision {
    /// Retry with the given backoff duration before the next attempt.
    Retry { backoff: Duration, attempt: u32 },
    /// Do not retry — pass the response back to the client.
    NoRetry,
    /// Retry budget depleted.
    BudgetExhausted,
    /// Maximum retries reached.
    Exhausted { attempts: u32 },
}

// ── retriable status set ──────────────────────────────────────────────────────

/// Returns `true` when `status` should be retried.
///
/// - 502 Bad Gateway
/// - 503 Service Unavailable
/// - 504 Gateway Timeout
///
/// 429 is handled separately via `Retry-After`.
pub fn is_retriable_status(status: u16) -> bool {
    matches!(status, 429 | 502 | 503 | 504)
}

// ── PingoraRetryPolicy ────────────────────────────────────────────────────────

/// Per-cluster retry configuration and budget, adapted for the Pingora path.
///
/// Stored in the gateway's upstream registry (one instance per cluster) and
/// shared across requests via `Arc`.
///
/// # Thread safety
///
/// `RetryBudget` uses atomic counters internally — safe for concurrent use.
#[derive(Debug)]
pub struct PingoraRetryPolicy {
    pub policy: RetryPolicy,
    pub budget: Arc<RetryBudget>,
    pub cluster: String,
}

impl PingoraRetryPolicy {
    /// Create a new policy with the given `RetryPolicy` and a shared budget.
    pub fn new(cluster: impl Into<String>, policy: RetryPolicy, budget: Arc<RetryBudget>) -> Self {
        Self {
            cluster: cluster.into(),
            policy,
            budget,
        }
    }

    /// Create a policy with sensible defaults for a given cluster.
    pub fn with_defaults(cluster: impl Into<String>) -> Self {
        Self::new(
            cluster,
            RetryPolicy {
                max_retries: 2,
                per_try_timeout: Duration::from_secs(15),
                overall_timeout: Duration::from_secs(45),
                backoff_base: Duration::from_millis(25),
                backoff_cap: Duration::from_secs(2),
                jitter: JitterMode::Full,
                retry_on: RetryOn {
                    status_codes: vec![502, 503, 504],
                    connect_error: true,
                    timeout: true,
                },
                hedge_on_per_try_timeout: false,
            },
            Arc::new(RetryBudget::default()),
        )
    }

    /// Decide whether to retry based on an HTTP status code and optional
    /// `Retry-After` header.
    ///
    /// `attempt` is the **number of the attempt that just failed** (1-based:
    /// 1 = original attempt, 2 = first retry, …).
    ///
    /// Returns [`RetryDecision::Retry`] with the computed backoff, or one of
    /// the non-retry variants.
    pub fn should_retry(
        &self,
        status: u16,
        retry_after_secs: Option<u64>,
        attempt: u32,
    ) -> RetryDecision {
        if !self.policy.retry_on.matches_status(status) && status != 429 {
            debug!(
                cluster = %self.cluster,
                status,
                "retry: non-retriable status — no retry"
            );
            return RetryDecision::NoRetry;
        }

        if attempt > self.policy.max_retries {
            warn!(
                cluster = %self.cluster,
                attempt,
                max = self.policy.max_retries,
                "retry: max_retries reached"
            );
            return RetryDecision::Exhausted { attempts: attempt };
        }

        if !self.budget.try_reserve() {
            warn!(
                cluster = %self.cluster,
                "retry: budget depleted"
            );
            return RetryDecision::BudgetExhausted;
        }

        let backoff = match retry_after_secs {
            Some(secs) => Duration::from_secs(secs).min(self.policy.backoff_cap),
            None => self.policy.backoff_for(attempt),
        };

        debug!(
            cluster = %self.cluster,
            status,
            attempt,
            backoff_ms = backoff.as_millis(),
            "retry: scheduling retry"
        );

        RetryDecision::Retry { backoff, attempt }
    }

    /// Decide whether to retry after a connect failure.
    ///
    /// Connect failures are always retriable when the budget allows and
    /// `retry_on.connect_error` is set.
    pub fn should_retry_connect_fail(&self, attempt: u32) -> RetryDecision {
        if !self.policy.retry_on.connect_error {
            return RetryDecision::NoRetry;
        }

        if attempt > self.policy.max_retries {
            warn!(
                cluster = %self.cluster,
                attempt,
                "retry: max_retries reached after connect fail"
            );
            return RetryDecision::Exhausted { attempts: attempt };
        }

        if !self.budget.try_reserve() {
            warn!(cluster = %self.cluster, "retry: budget depleted after connect fail");
            return RetryDecision::BudgetExhausted;
        }

        let backoff = self.policy.backoff_for(attempt);

        debug!(
            cluster = %self.cluster,
            attempt,
            backoff_ms = backoff.as_millis(),
            "retry: scheduling retry after connect fail"
        );

        RetryDecision::Retry { backoff, attempt }
    }

    /// Release a budget slot after a retry completes (success or final fail).
    ///
    /// Must be called symmetrically after every `try_reserve()` in
    /// `should_retry` / `should_retry_connect_fail`.
    pub fn release_budget(&self) {
        self.budget.release_retry();
    }

    /// Parse a `Retry-After` header value (seconds integer).
    ///
    /// Returns `None` when the header is absent, malformed, or zero.
    pub fn parse_retry_after(header: Option<&str>) -> Option<u64> {
        header
            .and_then(|v| v.trim().parse::<u64>().ok())
            .filter(|&n| n > 0)
    }
}

// ── RetryStats ────────────────────────────────────────────────────────────────

/// Per-request retry counters propagated through `RequestCtx`.
///
/// These are stored in `RequestCtx::retry_stats` (added in this module) and
/// read by the gateway hooks to track retry state across the pipeline.
#[derive(Debug, Default, Clone)]
pub struct RetryStats {
    /// Number of attempts made so far (original + retries).
    pub attempts: u32,
    /// Total retry delay accumulated so far.
    pub total_backoff: Duration,
}

impl RetryStats {
    /// Record one more attempt.
    pub fn record_attempt(&mut self, backoff: Duration) {
        self.attempts += 1;
        self.total_backoff += backoff;
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn policy_with_max_retries(n: u32) -> PingoraRetryPolicy {
        PingoraRetryPolicy {
            cluster: "test".to_string(),
            policy: RetryPolicy {
                max_retries: n,
                per_try_timeout: Duration::from_secs(5),
                overall_timeout: Duration::from_secs(30),
                backoff_base: Duration::from_millis(1),
                backoff_cap: Duration::from_secs(2),
                jitter: JitterMode::None, // deterministic
                retry_on: RetryOn {
                    status_codes: vec![502, 503, 504],
                    connect_error: true,
                    timeout: true,
                },
                hedge_on_per_try_timeout: false,
            },
            budget: Arc::new(RetryBudget::default()),
        }
    }

    // ── is_retriable_status ───────────────────────────────────────────────────

    #[test]
    fn retriable_status_codes_are_correct() {
        assert!(is_retriable_status(429));
        assert!(is_retriable_status(502));
        assert!(is_retriable_status(503));
        assert!(is_retriable_status(504));
        assert!(!is_retriable_status(200));
        assert!(!is_retriable_status(301));
        assert!(!is_retriable_status(400));
        assert!(!is_retriable_status(401));
        assert!(!is_retriable_status(403));
        assert!(!is_retriable_status(404));
    }

    // ── should_retry on 503 ───────────────────────────────────────────────────

    #[test]
    fn retry_on_503_within_budget() {
        let p = policy_with_max_retries(2);
        let decision = p.should_retry(503, None, 1);
        assert!(
            matches!(decision, RetryDecision::Retry { attempt: 1, .. }),
            "503 on attempt 1 should trigger retry"
        );
    }

    // ── no retry on 4xx (non-retriable) ──────────────────────────────────────

    #[test]
    fn no_retry_on_non_retriable_4xx() {
        let p = policy_with_max_retries(3);
        for status in [400u16, 401, 403, 404, 422] {
            let decision = p.should_retry(status, None, 1);
            assert_eq!(
                decision,
                RetryDecision::NoRetry,
                "status {status} must not trigger retry"
            );
        }
    }

    // ── exhausted after max_retries ───────────────────────────────────────────

    #[test]
    fn exhausted_after_max_retries() {
        let p = policy_with_max_retries(2);
        // attempt=3 → max exceeded
        let decision = p.should_retry(503, None, 3);
        assert_eq!(
            decision,
            RetryDecision::Exhausted { attempts: 3 },
            "attempt > max_retries must return Exhausted"
        );
    }

    // ── budget depleted ───────────────────────────────────────────────────────

    #[test]
    fn budget_depleted_blocks_retry() {
        let p = PingoraRetryPolicy {
            budget: Arc::new(RetryBudget::new(0.0, 0)), // zero capacity
            ..policy_with_max_retries(5)
        };
        let decision = p.should_retry(503, None, 1);
        assert_eq!(
            decision,
            RetryDecision::BudgetExhausted,
            "zero-budget must block retry"
        );
    }

    // ── Retry-After header respected ──────────────────────────────────────────

    #[test]
    fn retry_after_header_overrides_backoff() {
        let p = policy_with_max_retries(3);
        let decision = p.should_retry(429, Some(5), 1);
        match decision {
            RetryDecision::Retry { backoff, .. } => {
                assert_eq!(
                    backoff,
                    Duration::from_secs(5).min(p.policy.backoff_cap),
                    "backoff must equal Retry-After value (clamped to cap)"
                );
            }
            other => panic!("expected Retry, got {other:?}"),
        }
    }

    #[test]
    fn retry_after_capped_at_backoff_cap() {
        let p = PingoraRetryPolicy {
            policy: RetryPolicy {
                backoff_cap: Duration::from_secs(2),
                ..policy_with_max_retries(3).policy
            },
            ..policy_with_max_retries(3)
        };
        // Server says wait 3600 seconds — must be clamped to backoff_cap.
        let decision = p.should_retry(503, Some(3600), 1);
        match decision {
            RetryDecision::Retry { backoff, .. } => {
                assert_eq!(
                    backoff,
                    Duration::from_secs(2),
                    "Retry-After must be clamped to backoff_cap"
                );
            }
            other => panic!("expected Retry, got {other:?}"),
        }
    }

    // ── connect fail retry ────────────────────────────────────────────────────

    #[test]
    fn retry_on_connect_fail() {
        let p = policy_with_max_retries(2);
        let decision = p.should_retry_connect_fail(1);
        assert!(
            matches!(decision, RetryDecision::Retry { .. }),
            "connect fail must trigger retry when connect_error=true"
        );
    }

    #[test]
    fn no_retry_on_connect_fail_when_disabled() {
        let p = PingoraRetryPolicy {
            policy: RetryPolicy {
                retry_on: RetryOn {
                    connect_error: false,
                    ..RetryOn::default()
                },
                ..policy_with_max_retries(2).policy
            },
            ..policy_with_max_retries(2)
        };
        let decision = p.should_retry_connect_fail(1);
        assert_eq!(decision, RetryDecision::NoRetry);
    }

    // ── parse_retry_after ─────────────────────────────────────────────────────

    #[test]
    fn parse_retry_after_parses_integer() {
        assert_eq!(
            PingoraRetryPolicy::parse_retry_after(Some("30")),
            Some(30)
        );
    }

    #[test]
    fn parse_retry_after_ignores_zero() {
        assert_eq!(PingoraRetryPolicy::parse_retry_after(Some("0")), None);
    }

    #[test]
    fn parse_retry_after_ignores_garbage() {
        assert_eq!(PingoraRetryPolicy::parse_retry_after(Some("not-a-number")), None);
    }

    #[test]
    fn parse_retry_after_handles_none() {
        assert_eq!(PingoraRetryPolicy::parse_retry_after(None), None);
    }

    // ── retry_stats ───────────────────────────────────────────────────────────

    #[test]
    fn retry_stats_accumulates_attempts() {
        let mut stats = RetryStats::default();
        stats.record_attempt(Duration::from_millis(25));
        stats.record_attempt(Duration::from_millis(50));
        assert_eq!(stats.attempts, 2);
        assert_eq!(stats.total_backoff, Duration::from_millis(75));
    }

    // ── circuit_breaker interaction (no double-counting) ─────────────────────

    /// Ensure that the retry module itself does not call any circuit-breaker
    /// methods.  The circuit breaker is updated by the gateway hooks *before*
    /// the retry decision is taken — this test documents the invariant.
    #[test]
    fn retry_does_not_open_circuit_breaker() {
        use crate::pingora::upstream::circuit_breaker::{BreakerConfig, BreakerState, CircuitState};

        let cb = BreakerState::new(BreakerConfig {
            consecutive_5xx_threshold: 2,
            ..BreakerConfig::default()
        });
        let p = policy_with_max_retries(3);

        // Simulate: gateway records 1 × 5xx into circuit breaker,
        // THEN retry module is consulted.
        cb.record_5xx();
        assert_eq!(cb.state(), CircuitState::Closed, "1 failure should not open");

        // Retry module says "retry" — does NOT touch the circuit breaker.
        let decision = p.should_retry(503, None, 1);
        assert!(matches!(decision, RetryDecision::Retry { .. }));

        // Circuit breaker still has only 1 failure (not 2).
        assert_eq!(cb.state(), CircuitState::Closed,
            "retry module must not record a second failure in the circuit breaker");
    }
}

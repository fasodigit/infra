// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! `armageddon-retry` — Retry, timeout, and retry-budget policies for ARMAGEDDON
//! upstream calls.
//!
//! Implements:
//! - Envoy-compatible `x-envoy-max-retries` / `x-envoy-upstream-rq-timeout-ms`
//!   header semantics ([`policy`]).
//! - Token-bucket-style retry budget to prevent cascade failures ([`budget`]).
//! - Hedged request helper that races two attempts for tail-latency reduction
//!   ([`hedged`]).
//! - Tower [`Layer`] / [`Service`] composable middleware ([`layer`]).
//! - Unified error type ([`error`]).

pub mod budget;
pub mod error;
pub mod hedged;
pub mod layer;
pub mod metrics;
pub mod policy;

pub use budget::{RequestGuard, RetryBudget};
pub use error::RetryError;
pub use hedged::hedged;
pub use layer::{RetryLayer, RetryService};
pub use metrics::RetryMetrics;
pub use policy::{JitterMode, RetryOn, RetryPolicy};

use std::future::Future;
use std::time::Duration;
// Use tokio's Instant so that `start_paused` in tests controls the clock.
use tokio::time::Instant;
use tracing::{debug, warn};

// -- RetryableRequest trait --

/// Trait that a request type must implement to participate in the retry loop.
///
/// Implementors must be cheaply clonable (e.g. `Arc`-backed or copy types) and
/// must know which errors from their associated `call` are retryable.
pub trait RetryableRequest: Sized {
    /// The successful response type produced by a call.
    type Response;

    /// The error type produced by a call.
    type Error: std::fmt::Debug + std::fmt::Display;

    /// Produce a fresh clone of this request for a retry attempt.
    fn clone_for_retry(&self) -> Self;

    /// Return `true` if `e` represents a condition that should be retried.
    fn is_retryable_error(e: &Self::Error) -> bool;

    /// Optionally extract an HTTP status code from a *response* to check
    /// whether it should be retried (e.g. 503).  Return `None` when the
    /// response is successful.
    fn retryable_status(resp: &Self::Response) -> Option<u16> {
        let _ = resp;
        None
    }

    /// Optionally extract a `Retry-After` delay (in seconds) from a response
    /// header so the retry loop can honour server-driven back-off.
    fn retry_after(resp: &Self::Response) -> Option<Duration> {
        let _ = resp;
        None
    }
}

// -- execute_with_retry --

/// Drive a request through the retry loop governed by `policy` and `budget`.
///
/// # Behaviour
///
/// 1. The original request is attempted first (attempt 0).
/// 2. On a retryable error **or** a retryable status code in the response, the
///    loop increments `attempt` and — if the budget allows — fires another try.
/// 3. Each retry waits for `policy.backoff_for(attempt)` unless the server
///    returned a `Retry-After` header (which takes precedence, clamped to
///    `backoff_cap`).
/// 4. Every individual call is bounded by `policy.per_try_timeout`.
/// 5. The entire function returns [`RetryError::Timeout`] if
///    `policy.overall_timeout` elapses, even when retries are still available.
///
/// # Errors
///
/// - [`RetryError::Exhausted`]  — ran out of retry attempts.
/// - [`RetryError::Timeout`]    — overall deadline elapsed.
/// - [`RetryError::BudgetDepleted`] — budget said no.
/// - [`RetryError::PerTryTimeout`] — final attempt timed out with no retries left.
/// - [`RetryError::NonRetryable`] — error that must not be retried.
pub async fn execute_with_retry<R, F, Fut>(
    policy: &RetryPolicy,
    budget: &RetryBudget,
    req: R,
    call: F,
) -> Result<R::Response, RetryError>
where
    R: RetryableRequest,
    F: Fn(R) -> Fut,
    Fut: Future<Output = Result<R::Response, R::Error>>,
{
    let deadline = Instant::now() + policy.overall_timeout;
    let mut attempt: u32 = 0;
    // Clone the request before moving it into the first call.
    let mut current_req = req;

    loop {
        // -- overall timeout guard --
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            warn!(attempt, "overall timeout reached before attempt");
            return Err(RetryError::Timeout {
                timeout: policy.overall_timeout,
                attempts: attempt,
            });
        }

        // -- per-try timeout --
        let per_try = policy.per_try_timeout.min(remaining);
        let req_clone = current_req.clone_for_retry();

        debug!(attempt, "dispatching upstream call");

        let outcome = tokio::time::timeout(per_try, call(req_clone)).await;

        attempt += 1;

        match outcome {
            // --- success ---
            Ok(Ok(response)) => {
                // Check whether the *response* itself signals a retryable condition.
                if let Some(status) = R::retryable_status(&response) {
                    if policy.retry_on.matches_status(status) && attempt <= policy.max_retries {
                        warn!(attempt, status, "retryable response status");
                        let backoff = R::retry_after(&response)
                            .map(|d| d.min(policy.backoff_cap))
                            .unwrap_or_else(|| policy.backoff_for(attempt));

                        if !budget.try_reserve() {
                            warn!(attempt, "retry budget depleted");
                            return Err(RetryError::BudgetDepleted);
                        }
                        tokio::time::sleep(backoff).await;
                        budget.release_retry();
                        current_req = current_req.clone_for_retry();
                        continue;
                    }
                }
                return Ok(response);
            }

            // --- call-level error ---
            Ok(Err(e)) => {
                if !R::is_retryable_error(&e) {
                    return Err(RetryError::NonRetryable(e.to_string()));
                }
                if attempt > policy.max_retries {
                    warn!(attempt, error = %e, "retries exhausted");
                    return Err(RetryError::Exhausted { attempts: attempt });
                }
                // Check overall deadline before sleeping.
                if Instant::now() >= deadline {
                    warn!(attempt, "overall timeout reached before backoff sleep");
                    return Err(RetryError::Timeout {
                        timeout: policy.overall_timeout,
                        attempts: attempt,
                    });
                }
                if !budget.try_reserve() {
                    warn!(attempt, "retry budget depleted");
                    return Err(RetryError::BudgetDepleted);
                }
                let backoff = policy.backoff_for(attempt);
                debug!(attempt, backoff_ms = backoff.as_millis(), "backing off before retry");
                tokio::time::sleep(backoff).await;
                budget.release_retry();
                // Re-check overall deadline after sleeping.
                if Instant::now() >= deadline {
                    warn!(attempt, "overall timeout reached after backoff sleep");
                    return Err(RetryError::Timeout {
                        timeout: policy.overall_timeout,
                        attempts: attempt,
                    });
                }
                current_req = current_req.clone_for_retry();
            }

            // --- per-try timeout ---
            Err(_elapsed) => {
                if !policy.retry_on.timeout {
                    return Err(RetryError::PerTryTimeout {
                        timeout: per_try,
                    });
                }
                // A per-try timeout already consumed `per_try` time — check overall.
                if Instant::now() >= deadline {
                    warn!(attempt, "overall timeout reached after per-try timeout");
                    return Err(RetryError::Timeout {
                        timeout: policy.overall_timeout,
                        attempts: attempt,
                    });
                }
                if attempt > policy.max_retries {
                    warn!(attempt, "retries exhausted after per-try timeout");
                    return Err(RetryError::PerTryTimeout {
                        timeout: per_try,
                    });
                }
                if !budget.try_reserve() {
                    warn!(attempt, "retry budget depleted after per-try timeout");
                    return Err(RetryError::BudgetDepleted);
                }
                let backoff = policy.backoff_for(attempt);
                debug!(attempt, backoff_ms = backoff.as_millis(), "backing off after timeout");
                tokio::time::sleep(backoff).await;
                budget.release_retry();
                // Re-check overall deadline after sleeping.
                if Instant::now() >= deadline {
                    warn!(attempt, "overall timeout reached after per-try backoff sleep");
                    return Err(RetryError::Timeout {
                        timeout: policy.overall_timeout,
                        attempts: attempt,
                    });
                }
                current_req = current_req.clone_for_retry();
            }
        }
    }
}

// -- tests --

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    // -- helpers --

    /// A minimal retryable request wrapping a counter of attempts.
    #[derive(Clone)]
    struct Req {
        id: u32,
    }

    #[derive(Debug)]
    enum CallError {
        Retryable(String),
        Fatal(String),
    }

    impl std::fmt::Display for CallError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::Retryable(s) | Self::Fatal(s) => write!(f, "{s}"),
            }
        }
    }

    impl RetryableRequest for Req {
        type Response = String;
        type Error = CallError;

        fn clone_for_retry(&self) -> Self {
            Req { id: self.id }
        }

        fn is_retryable_error(e: &Self::Error) -> bool {
            matches!(e, CallError::Retryable(_))
        }
    }

    fn fast_policy(max_retries: u32) -> RetryPolicy {
        RetryPolicy {
            max_retries,
            per_try_timeout: Duration::from_secs(5),
            overall_timeout: Duration::from_secs(30),
            backoff_base: Duration::from_millis(1),
            backoff_cap: Duration::from_millis(5),
            ..Default::default()
        }
    }

    // -- TEST 1: success on first attempt → no retry --
    #[tokio::test]
    async fn success_first_attempt_no_retry() {
        let calls = Arc::new(AtomicU32::new(0));
        let calls2 = Arc::clone(&calls);

        let result = execute_with_retry(
            &fast_policy(3),
            &RetryBudget::default(),
            Req { id: 1 },
            move |_req| {
                calls2.fetch_add(1, Ordering::SeqCst);
                async { Ok::<String, CallError>("ok".into()) }
            },
        )
        .await;

        assert_eq!(result.unwrap(), "ok");
        assert_eq!(calls.load(Ordering::SeqCst), 1, "should call exactly once");
    }

    // -- TEST 2: 5xx then success on 2nd → 1 retry, returns success --
    #[tokio::test]
    async fn retryable_status_then_success() {
        #[derive(Clone)]
        struct StatusReq;

        impl RetryableRequest for StatusReq {
            type Response = u16; // we return status codes directly
            type Error = CallError;

            fn clone_for_retry(&self) -> Self { StatusReq }
            fn is_retryable_error(e: &Self::Error) -> bool {
                matches!(e, CallError::Retryable(_))
            }
            fn retryable_status(resp: &u16) -> Option<u16> {
                if *resp >= 500 { Some(*resp) } else { None }
            }
        }

        let counter = Arc::new(AtomicU32::new(0));
        let counter2 = Arc::clone(&counter);

        let result = execute_with_retry(
            &fast_policy(2),
            &RetryBudget::default(),
            StatusReq,
            move |_req| {
                let n = counter2.fetch_add(1, Ordering::SeqCst);
                async move {
                    if n == 0 { Ok::<u16, CallError>(503) } else { Ok(200) }
                }
            },
        )
        .await;

        assert_eq!(result.unwrap(), 200);
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    // -- TEST 3: 5xx three times → RetryError::Exhausted (max_retries = 2) --
    #[tokio::test]
    async fn exhausted_after_max_retries() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter2 = Arc::clone(&counter);

        let result = execute_with_retry(
            &fast_policy(2),
            &RetryBudget::default(),
            Req { id: 2 },
            move |_req| {
                counter2.fetch_add(1, Ordering::SeqCst);
                async { Err::<String, CallError>(CallError::Retryable("503".into())) }
            },
        )
        .await;

        assert!(matches!(result, Err(RetryError::Exhausted { attempts: 3 })));
        assert_eq!(counter.load(Ordering::SeqCst), 3); // original + 2 retries
    }

    // -- TEST 4: budget depleted → no retry --
    #[tokio::test]
    async fn budget_depleted_blocks_retry() {
        // Budget with 0 capacity.
        let budget = RetryBudget::new(0.0, 0);
        let calls = Arc::new(AtomicU32::new(0));
        let calls2 = Arc::clone(&calls);

        let result = execute_with_retry(
            &fast_policy(3),
            &budget,
            Req { id: 3 },
            move |_req| {
                calls2.fetch_add(1, Ordering::SeqCst);
                async { Err::<String, CallError>(CallError::Retryable("fail".into())) }
            },
        )
        .await;

        assert!(matches!(result, Err(RetryError::BudgetDepleted)));
        assert_eq!(calls.load(Ordering::SeqCst), 1); // only 1 attempt
    }

    // -- TEST 5: per_try_timeout fires → retry --
    #[tokio::test(start_paused = true)]
    async fn per_try_timeout_triggers_retry() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter2 = Arc::clone(&counter);

        let policy = RetryPolicy {
            max_retries: 1,
            per_try_timeout: Duration::from_millis(50),
            overall_timeout: Duration::from_secs(10),
            backoff_base: Duration::from_millis(1),
            backoff_cap: Duration::from_millis(5),
            retry_on: RetryOn { timeout: true, ..Default::default() },
            ..Default::default()
        };

        let result = execute_with_retry(
            &policy,
            &RetryBudget::default(),
            Req { id: 5 },
            move |_req| {
                let n = counter2.fetch_add(1, Ordering::SeqCst);
                async move {
                    if n == 0 {
                        // Hang forever → per-try timeout fires.
                        tokio::time::sleep(Duration::from_secs(999)).await;
                        Err::<String, _>(CallError::Retryable("hang".into()))
                    } else {
                        Ok("recovered".into())
                    }
                }
            },
        )
        .await;

        assert_eq!(result.unwrap(), "recovered");
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    // -- TEST 6: overall_timeout → RetryError::Timeout --
    #[tokio::test(start_paused = true)]
    async fn overall_timeout_stops_retries() {
        let policy = RetryPolicy {
            max_retries: 10,
            per_try_timeout: Duration::from_millis(200),
            overall_timeout: Duration::from_millis(300),
            backoff_base: Duration::from_millis(1),
            backoff_cap: Duration::from_millis(5),
            retry_on: RetryOn { timeout: true, ..Default::default() },
            ..Default::default()
        };

        let result = execute_with_retry(
            &policy,
            &RetryBudget::default(),
            Req { id: 6 },
            |_req| async {
                // Each attempt hangs until per-try timeout.
                tokio::time::sleep(Duration::from_millis(200)).await;
                Err::<String, _>(CallError::Retryable("hang".into()))
            },
        )
        .await;

        assert!(
            matches!(result, Err(RetryError::Timeout { .. })),
            "expected Timeout, got {result:?}"
        );
    }

    // -- TEST 7: Retry-After header adjusts backoff --
    #[tokio::test(start_paused = true)]
    async fn retry_after_header_respected() {
        #[derive(Clone)]
        struct RaReq;

        #[derive(Debug)]
        struct RaResp { status: u16, retry_after_secs: Option<u64> }

        impl RetryableRequest for RaReq {
            type Response = RaResp;
            type Error = CallError;

            fn clone_for_retry(&self) -> Self { RaReq }
            fn is_retryable_error(_: &CallError) -> bool { true }
            fn retryable_status(r: &RaResp) -> Option<u16> {
                if r.status >= 500 { Some(r.status) } else { None }
            }
            fn retry_after(r: &RaResp) -> Option<Duration> {
                r.retry_after_secs.map(Duration::from_secs)
            }
        }

        let counter = Arc::new(AtomicU32::new(0));
        let counter2 = Arc::clone(&counter);
        let start = tokio::time::Instant::now();

        let policy = RetryPolicy {
            max_retries: 1,
            per_try_timeout: Duration::from_secs(5),
            overall_timeout: Duration::from_secs(60),
            backoff_base: Duration::from_millis(1),
            backoff_cap: Duration::from_secs(10),
            ..Default::default()
        };

        let _result = execute_with_retry(
            &policy,
            &RetryBudget::default(),
            RaReq,
            move |_req| {
                let n = counter2.fetch_add(1, Ordering::SeqCst);
                async move {
                    if n == 0 {
                        Ok::<RaResp, CallError>(RaResp { status: 503, retry_after_secs: Some(2) })
                    } else {
                        Ok(RaResp { status: 200, retry_after_secs: None })
                    }
                }
            },
        )
        .await;

        // With start_paused, tokio::time::sleep advances the mock clock.
        // The retry should have waited at least 2 s (Retry-After).
        assert!(start.elapsed() >= Duration::from_secs(2));
    }

    // -- TEST 8: non-retryable error is propagated immediately --
    #[tokio::test]
    async fn non_retryable_error_not_retried() {
        let calls = Arc::new(AtomicU32::new(0));
        let calls2 = Arc::clone(&calls);

        let result = execute_with_retry(
            &fast_policy(5),
            &RetryBudget::default(),
            Req { id: 8 },
            move |_req| {
                calls2.fetch_add(1, Ordering::SeqCst);
                async { Err::<String, _>(CallError::Fatal("fatal".into())) }
            },
        )
        .await;

        assert!(matches!(result, Err(RetryError::NonRetryable(_))));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}

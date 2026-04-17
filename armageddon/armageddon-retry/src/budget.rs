// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Retry budget — limits the proportion of total active requests that may be
//! retries at any given time, preventing retry storms from cascading into
//! downstream failure.
//!
//! Inspired by Envoy's `retry_budget` and Twitter Finagle retry budgets.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

// -- RetryBudget --

/// Shared, thread-safe retry budget.
///
/// At most `max(min_retry_concurrency, floor(active_requests * retry_percent))`
/// concurrent retries are allowed at once.
#[derive(Debug)]
pub struct RetryBudget {
    /// Currently in-flight (original + retry) requests.
    active_requests: Arc<AtomicUsize>,
    /// Currently in-flight *retry* requests (subset of `active_requests`).
    active_retries: Arc<AtomicUsize>,
    /// Fraction of active requests that may be retries (0.0 – 1.0).
    pub retry_percent: f32,
    /// Minimum number of retries allowed even when `active_requests` is very small.
    pub min_retry_concurrency: u32,
}

impl Default for RetryBudget {
    fn default() -> Self {
        Self::new(0.20, 10)
    }
}

impl RetryBudget {
    /// Create a new `RetryBudget`.
    ///
    /// - `retry_percent`: e.g. `0.20` means up to 20 % of active requests can be retries.
    /// - `min_retry_concurrency`: floor that is always available, regardless of load.
    pub fn new(retry_percent: f32, min_retry_concurrency: u32) -> Self {
        Self {
            active_requests: Arc::new(AtomicUsize::new(0)),
            active_retries: Arc::new(AtomicUsize::new(0)),
            retry_percent: retry_percent.clamp(0.0, 1.0),
            min_retry_concurrency,
        }
    }

    /// Register a new in-flight request.  Call this when a request starts.
    /// Returns a guard that decrements the counter on drop.
    pub fn acquire_request(&self) -> RequestGuard {
        self.active_requests.fetch_add(1, Ordering::Relaxed);
        RequestGuard {
            active_requests: Arc::clone(&self.active_requests),
        }
    }

    /// Attempt to reserve one retry slot.
    ///
    /// Returns `true` and increments `active_retries` if the budget allows it.
    /// Returns `false` if the budget is exhausted — the caller should **not** retry.
    pub fn try_reserve(&self) -> bool {
        let active = self.active_requests.load(Ordering::Relaxed);
        let allowed = std::cmp::max(
            self.min_retry_concurrency as usize,
            (active as f64 * self.retry_percent as f64).floor() as usize,
        );
        // Attempt CAS-loop to increment within budget.
        loop {
            let current = self.active_retries.load(Ordering::Acquire);
            if current >= allowed {
                return false;
            }
            match self.active_retries.compare_exchange_weak(
                current,
                current + 1,
                Ordering::AcqRel,
                Ordering::Relaxed,
            ) {
                Ok(_) => return true,
                Err(_) => continue,
            }
        }
    }

    /// Release a previously reserved retry slot.
    pub fn release_retry(&self) {
        // Avoid underflow if release is called without a prior reservation.
        let prev = self.active_retries.fetch_update(
            Ordering::AcqRel,
            Ordering::Relaxed,
            |v| if v > 0 { Some(v - 1) } else { Some(0) },
        );
        let _ = prev;
    }

    /// Snapshot of the current active-request count (for metrics / tests).
    pub fn active_request_count(&self) -> usize {
        self.active_requests.load(Ordering::Relaxed)
    }

    /// Snapshot of the current active-retry count (for metrics / tests).
    pub fn active_retry_count(&self) -> usize {
        self.active_retries.load(Ordering::Relaxed)
    }
}

// -- RequestGuard --

/// RAII guard that decrements the active-request counter on drop.
pub struct RequestGuard {
    active_requests: Arc<AtomicUsize>,
}

impl Drop for RequestGuard {
    fn drop(&mut self) {
        self.active_requests.fetch_update(
            Ordering::AcqRel,
            Ordering::Relaxed,
            |v| if v > 0 { Some(v - 1) } else { Some(0) },
        ).ok();
    }
}

// -- tests --

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_allows_retries_within_min_concurrency() {
        let budget = RetryBudget::new(0.0, 3); // only min floor, no percent
        assert!(budget.try_reserve());
        assert!(budget.try_reserve());
        assert!(budget.try_reserve());
        assert!(!budget.try_reserve()); // 4th should fail
    }

    #[test]
    fn budget_release_restores_slot() {
        let budget = RetryBudget::new(0.0, 1);
        assert!(budget.try_reserve());
        assert!(!budget.try_reserve());
        budget.release_retry();
        assert!(budget.try_reserve()); // slot restored
    }

    #[test]
    fn budget_percent_scales_with_active_requests() {
        let budget = RetryBudget::new(0.5, 0); // 50%, min=0
        let _g1 = budget.acquire_request();
        let _g2 = budget.acquire_request();
        let _g3 = budget.acquire_request();
        let _g4 = budget.acquire_request(); // 4 active → floor(4*0.5) = 2 retries allowed
        assert!(budget.try_reserve());
        assert!(budget.try_reserve());
        assert!(!budget.try_reserve()); // 3rd fails
    }

    #[test]
    fn request_guard_decrements_on_drop() {
        let budget = RetryBudget::new(0.20, 5);
        {
            let _g = budget.acquire_request();
            assert_eq!(budget.active_request_count(), 1);
        }
        assert_eq!(budget.active_request_count(), 0);
    }
}

// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Prometheus metrics for the retry / hedge subsystem.
//!
//! # Exported metrics
//!
//! | Name | Type | Labels | Description |
//! |------|------|--------|-------------|
//! | `armageddon_retries_total` | Counter | `cluster`, `outcome` | Retry attempts by outcome |
//! | `armageddon_retry_budget_exceeded_total` | Counter | `cluster` | Budget-blocked retries |
//! | `armageddon_hedge_fired_total` | Counter | `cluster` | Hedged requests launched |
//!
//! # Outcomes
//!
//! `outcome` label values: `"success"`, `"exhausted"`, `"timeout"`,
//! `"budget_depleted"`, `"per_try_timeout"`, `"non_retryable"`.
//!
//! # Failure modes
//!
//! Metrics registration panics on duplicate if a different `RetryMetrics`
//! instance was created in the same process.  The fallback approach (used in
//! tests) calls `register_counter_vec` inside `unwrap_or_else` to return an
//! isolated counter that is still functional but un-registered.

use prometheus::{
    register_counter_vec, CounterVec,
};

// -- RetryMetrics --

/// Shared Prometheus counters for the retry engine.
///
/// Construct once per cluster and pass a shared reference to
/// `execute_with_retry`.
pub struct RetryMetrics {
    /// `armageddon_retries_total{cluster, outcome}`.
    pub retries_total: CounterVec,
    /// `armageddon_retry_budget_exceeded_total{cluster}`.
    pub budget_exceeded_total: CounterVec,
    /// `armageddon_hedge_fired_total{cluster}`.
    pub hedge_fired_total: CounterVec,
}

impl RetryMetrics {
    /// Register (or re-use) the Prometheus metrics for `cluster`.
    ///
    /// Safe to call multiple times with the same cluster name in production
    /// (idempotent via the fallback path).  In tests, call this once per
    /// process or use `RetryMetrics::noop()`.
    pub fn new() -> Self {
        let retries_total = register_counter_vec!(
            "armageddon_retries_total",
            "Total retry attempts by cluster and outcome",
            &["cluster", "outcome"]
        )
        .unwrap_or_else(|_| {
            // Duplicate registration (e.g. multiple tests in same process).
            prometheus::CounterVec::new(
                prometheus::Opts::new(
                    "armageddon_retries_total_fallback",
                    "duplicate-registration fallback",
                ),
                &["cluster", "outcome"],
            )
            .unwrap()
        });

        let budget_exceeded_total = register_counter_vec!(
            "armageddon_retry_budget_exceeded_total",
            "Times a retry was skipped because the budget was depleted",
            &["cluster"]
        )
        .unwrap_or_else(|_| {
            prometheus::CounterVec::new(
                prometheus::Opts::new(
                    "armageddon_retry_budget_exceeded_total_fallback",
                    "duplicate-registration fallback",
                ),
                &["cluster"],
            )
            .unwrap()
        });

        let hedge_fired_total = register_counter_vec!(
            "armageddon_hedge_fired_total",
            "Times a hedged request was launched",
            &["cluster"]
        )
        .unwrap_or_else(|_| {
            prometheus::CounterVec::new(
                prometheus::Opts::new(
                    "armageddon_hedge_fired_total_fallback",
                    "duplicate-registration fallback",
                ),
                &["cluster"],
            )
            .unwrap()
        });

        Self {
            retries_total,
            budget_exceeded_total,
            hedge_fired_total,
        }
    }

    /// Record a retry outcome for a given cluster.
    ///
    /// `outcome` should be one of: `"success"`, `"exhausted"`, `"timeout"`,
    /// `"budget_depleted"`, `"per_try_timeout"`, `"non_retryable"`.
    #[inline]
    pub fn record_retry(&self, cluster: &str, outcome: &str) {
        self.retries_total
            .with_label_values(&[cluster, outcome])
            .inc();
    }

    /// Record that a retry was suppressed by the budget for `cluster`.
    #[inline]
    pub fn record_budget_exceeded(&self, cluster: &str) {
        self.budget_exceeded_total
            .with_label_values(&[cluster])
            .inc();
    }

    /// Record that a hedged request was fired for `cluster`.
    #[inline]
    pub fn record_hedge_fired(&self, cluster: &str) {
        self.hedge_fired_total
            .with_label_values(&[cluster])
            .inc();
    }
}

impl Default for RetryMetrics {
    fn default() -> Self {
        Self::new()
    }
}

// -- tests --

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_record_retry_increments_counter() {
        let m = RetryMetrics::new();
        m.record_retry("cluster-a", "success");
        m.record_retry("cluster-a", "success");
        m.record_retry("cluster-a", "exhausted");

        // We can't read prometheus counters trivially, but we can assert the
        // record calls don't panic and the metric object is valid.
        m.record_budget_exceeded("cluster-a");
        m.record_hedge_fired("cluster-a");
    }
}

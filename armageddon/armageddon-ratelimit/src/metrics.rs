// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Prometheus metrics for the rate limiting subsystem.

use prometheus::{CounterVec, HistogramOpts, HistogramVec, Opts, Registry};

/// Prometheus metrics bundle for `armageddon-ratelimit`.
#[derive(Clone)]
pub struct RateLimitMetrics {
    /// `armageddon_ratelimit_decisions_total{mode, decision, descriptor}`
    pub decisions: CounterVec,
    /// `armageddon_ratelimit_kaya_latency_seconds{descriptor}`
    pub kaya_latency: HistogramVec,
}

impl RateLimitMetrics {
    /// Register all metrics with the provided `Registry`.
    pub fn new(registry: &Registry) -> Result<Self, prometheus::Error> {
        let decisions = CounterVec::new(
            Opts::new(
                "armageddon_ratelimit_decisions_total",
                "Total rate limit decisions by mode, decision and descriptor",
            ),
            &["mode", "decision", "descriptor"],
        )?;

        let kaya_latency = HistogramVec::new(
            HistogramOpts::new(
                "armageddon_ratelimit_kaya_latency_seconds",
                "Latency of KAYA rate-limit INCR operations",
            )
            .buckets(vec![
                0.0001, 0.0005, 0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0,
            ]),
            &["descriptor"],
        )?;

        registry.register(Box::new(decisions.clone()))?;
        registry.register(Box::new(kaya_latency.clone()))?;

        Ok(Self { decisions, kaya_latency })
    }

    /// Record a rate limit decision.
    pub fn record_decision(&self, mode: &str, decision: &str, descriptor: &str) {
        self.decisions
            .with_label_values(&[mode, decision, descriptor])
            .inc();
    }

    /// Record KAYA latency for a descriptor.
    pub fn observe_kaya_latency(&self, descriptor: &str, elapsed_secs: f64) {
        self.kaya_latency
            .with_label_values(&[descriptor])
            .observe(elapsed_secs);
    }
}

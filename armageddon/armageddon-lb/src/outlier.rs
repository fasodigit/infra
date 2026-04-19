// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Passive outlier detection — ejects upstream hosts that produce excessive
//! errors, matching Envoy's `outlier_detection` semantics.
//!
//! # Design
//!
//! Each [`OutlierDetector`] tracks per-endpoint state in a
//! `parking_lot::RwLock<HashMap<…>>`.  Writers hold the lock only for a short
//! CAS-style update; readers acquire a shared lock to scan for ejected hosts
//! before every load-balancer selection.
//!
//! # Ejection algorithm
//!
//! 1. On each upstream response the caller reports success or failure via
//!    [`OutlierDetector::record_success`] / [`OutlierDetector::record_failure`].
//! 2. Consecutive 5xx ≥ `consecutive_5xx` **or** consecutive gateway failures
//!    ≥ `consecutive_gateway_failure` triggers ejection.
//! 3. Ejection duration = `base_ejection_time * consecutive_ejections`,
//!    capped at `max_ejection_time`.
//! 4. After expiry the host's state flips to `EjectionState::Probing`.
//!    The next successful call re-admits it; the next failure re-ejects it with
//!    doubled timeout.
//! 5. At most `max_ejection_percent` of the cluster may be ejected at once.
//!
//! # Success-rate ejection (optional)
//!
//! When `success_rate_enabled` is true the detector also computes a rolling
//! success rate over the last `success_rate_request_volume` calls per host.
//! Hosts whose rate falls below `average - stdev * stdev_factor` are ejected.
//!
//! # Failure modes
//!
//! - **Leader / replica loss**: not relevant; detector is per-process.
//! - **Quorum loss**: `max_ejection_percent` ensures at least 50 % of hosts
//!   remain in rotation even under a cascade failure.
//! - **Network partition**: hosts on the wrong side of the split will accumulate
//!   errors and be ejected; they re-enter after `max_ejection_time` with probing.
//!
//! # Metrics
//!
//! | Name | Type | Labels | Description |
//! |------|------|--------|-------------|
//! | `armageddon_outlier_ejections_total` | Counter | `cluster`, `reason` | Total ejection events |
//! | `armageddon_outlier_hosts_ejected` | Gauge | `cluster` | Currently ejected host count |

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use armageddon_common::types::OutlierDetectionConfig;
use parking_lot::RwLock;
use prometheus::{register_counter_vec, register_gauge_vec, CounterVec, GaugeVec};
use tracing::{debug, info, warn};

// -- ejection state --

/// Reason for an ejection event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EjectionReason {
    /// Too many consecutive 5xx responses.
    Consecutive5xx,
    /// Too many consecutive gateway (TCP-level) failures.
    ConsecutiveGatewayFailure,
    /// Host's rolling success rate fell below the cluster average threshold.
    SuccessRate,
}

impl EjectionReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::Consecutive5xx => "consecutive_5xx",
            Self::ConsecutiveGatewayFailure => "consecutive_gateway_failure",
            Self::SuccessRate => "success_rate",
        }
    }
}

/// Per-host ejection state machine.
#[derive(Debug, Clone, PartialEq, Eq)]
enum EjectionState {
    /// Host is healthy and in active rotation.
    Healthy,
    /// Host has been ejected; excluded from selection until `until`.
    Ejected { until: Instant },
    /// Ejection timer expired; host is allowed one probe request.
    /// If that probe succeeds → back to Healthy.  If it fails → re-ejected.
    Probing,
}

/// Sliding-window success-rate tracker (ring buffer).
#[derive(Debug)]
struct SuccessWindow {
    /// Ring buffer: `true` = success, `false` = failure.
    window: VecDeque<bool>,
    capacity: usize,
}

impl SuccessWindow {
    fn new(capacity: usize) -> Self {
        Self {
            window: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    fn push(&mut self, success: bool) {
        if self.window.len() == self.capacity {
            self.window.pop_front();
        }
        self.window.push_back(success);
    }

    /// Returns `Some(rate)` once the window is full, `None` otherwise.
    fn success_rate(&self) -> Option<f64> {
        if self.window.len() < self.capacity {
            return None;
        }
        let successes = self.window.iter().filter(|&&v| v).count();
        Some(successes as f64 / self.capacity as f64)
    }
}

/// Per-host tracking state.
#[derive(Debug)]
struct HostState {
    address: String,
    /// Consecutive 5xx counter; reset on success.
    consecutive_5xx: u32,
    /// Consecutive gateway-failure counter; reset on success.
    consecutive_gateway_failure: u32,
    /// Number of times this host has been ejected (resets to 0 on healthy > 1 interval).
    consecutive_ejections: u32,
    ejection_state: EjectionState,
    success_window: Option<SuccessWindow>,
}

impl HostState {
    fn new(address: String, window_size: Option<usize>) -> Self {
        Self {
            address,
            consecutive_5xx: 0,
            consecutive_gateway_failure: 0,
            consecutive_ejections: 0,
            ejection_state: EjectionState::Healthy,
            success_window: window_size.map(SuccessWindow::new),
        }
    }

    /// Returns `true` when the host should be excluded from selection.
    fn is_excluded(&self) -> bool {
        match &self.ejection_state {
            EjectionState::Ejected { until } => Instant::now() < *until,
            // A probing host is allowed exactly one request; caller marks it
            // either back to Healthy or re-ejects it.
            EjectionState::Probing => false,
            EjectionState::Healthy => false,
        }
    }

    /// Transition expired Ejected → Probing.
    fn maybe_transition_to_probing(&mut self) {
        if let EjectionState::Ejected { until } = self.ejection_state {
            if Instant::now() >= until {
                debug!(host = %self.address, "outlier: ejection expired → Probing");
                self.ejection_state = EjectionState::Probing;
            }
        }
    }
}

// -- metrics --

struct OutlierMetrics {
    ejections_total: CounterVec,
    hosts_ejected: GaugeVec,
}

impl OutlierMetrics {
    fn new() -> Self {
        let ejections_total = register_counter_vec!(
            "armageddon_outlier_ejections_total",
            "Total outlier ejection events by cluster and reason",
            &["cluster", "reason"]
        )
        .unwrap_or_else(|_| {
            prometheus::CounterVec::new(
                prometheus::Opts::new(
                    "armageddon_outlier_ejections_total_fallback",
                    "duplicate-registration fallback",
                ),
                &["cluster", "reason"],
            )
            .unwrap()
        });

        let hosts_ejected = register_gauge_vec!(
            "armageddon_outlier_hosts_ejected",
            "Number of hosts currently ejected by cluster",
            &["cluster"]
        )
        .unwrap_or_else(|_| {
            prometheus::GaugeVec::new(
                prometheus::Opts::new(
                    "armageddon_outlier_hosts_ejected_fallback",
                    "duplicate-registration fallback",
                ),
                &["cluster"],
            )
            .unwrap()
        });

        Self {
            ejections_total,
            hosts_ejected,
        }
    }
}

// -- OutlierDetector --

/// Per-cluster outlier detector.
///
/// Tracks per-host error counters and ejection state.  Designed to be held
/// behind an `Arc` and shared between the LB selection path and the response
/// recording path.
///
/// # Example
///
/// ```rust,ignore
/// use armageddon_lb::outlier::{OutlierDetector, FailureKind};
/// use armageddon_common::types::OutlierDetectionConfig;
/// use std::sync::Arc;
///
/// let detector = Arc::new(OutlierDetector::new("my-cluster", OutlierDetectionConfig::default()));
///
/// // On each upstream response:
/// detector.record_success("10.0.0.1:8080");
/// detector.record_failure("10.0.0.2:8080", FailureKind::Http5xx);
///
/// // Before host selection:
/// let allowed: Vec<_> = endpoints.iter().filter(|e| !detector.is_ejected(&e.address)).collect();
/// ```
pub struct OutlierDetector {
    cluster: String,
    config: OutlierDetectionConfig,
    state: RwLock<HashMap<String, HostState>>,
    metrics: Arc<OutlierMetrics>,
}

impl std::fmt::Debug for OutlierDetector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OutlierDetector")
            .field("cluster", &self.cluster)
            .field("hosts", &self.state.read().len())
            .finish()
    }
}

impl OutlierDetector {
    /// Create a new detector for `cluster` using the given config.
    pub fn new(cluster: impl Into<String>, config: OutlierDetectionConfig) -> Self {
        Self {
            cluster: cluster.into(),
            config,
            state: RwLock::new(HashMap::new()),
            metrics: Arc::new(OutlierMetrics::new()),
        }
    }

    /// Record a successful response from `host`.
    ///
    /// Resets consecutive-error counters.  If the host was in `Probing` state
    /// it transitions back to `Healthy`.
    pub fn record_success(&self, host: &str) {
        let mut was_probing = false;
        {
            let mut state = self.state.write();
            let hs = state
                .entry(host.to_string())
                .or_insert_with(|| HostState::new(host.to_string(), self.window_size()));

            hs.consecutive_5xx = 0;
            hs.consecutive_gateway_failure = 0;

            if let Some(w) = hs.success_window.as_mut() {
                w.push(true);
            }

            if hs.ejection_state == EjectionState::Probing {
                info!(host, cluster = %self.cluster, "outlier: probe succeeded → Healthy");
                hs.ejection_state = EjectionState::Healthy;
                hs.consecutive_ejections = 0;
                was_probing = true;
            }
        }
        if was_probing {
            // Re-acquire read lock to refresh the gauge after the write is released.
            let state = self.state.read();
            self.refresh_ejected_gauge_from_read(&state);
        }
    }

    /// Record a failure from `host`.
    ///
    /// Increments the appropriate counter and triggers ejection if the
    /// threshold is crossed.
    ///
    /// `kind` controls which counter is incremented:
    /// - [`FailureKind::Http5xx`] → `consecutive_5xx`
    /// - [`FailureKind::GatewayFailure`] → `consecutive_gateway_failure`
    pub fn record_failure(&self, host: &str, kind: FailureKind) {
        // Compute cap-check values before taking a mutable borrow so the
        // borrow checker is satisfied (no &mut + & aliasing on the same map).
        let (total, current_ejected) = {
            let state = self.state.read();
            let total = state.len().max(1);
            let ejected = state
                .values()
                .filter(|h| {
                    matches!(h.ejection_state, EjectionState::Ejected { until }
                             if Instant::now() < until)
                })
                .count();
            (total, ejected)
        };

        let mut state = self.state.write();
        let hs = state
            .entry(host.to_string())
            .or_insert_with(|| HostState::new(host.to_string(), self.window_size()));

        if let Some(w) = hs.success_window.as_mut() {
            w.push(false);
        }

        // Only count new failures when host is not already ejected.
        if hs.is_excluded() {
            return;
        }

        let was_probing = hs.ejection_state == EjectionState::Probing;

        let reason = match kind {
            FailureKind::Http5xx => {
                hs.consecutive_5xx += 1;
                hs.consecutive_gateway_failure = 0;
                if hs.consecutive_5xx >= self.config.consecutive_5xx || was_probing {
                    Some(EjectionReason::Consecutive5xx)
                } else {
                    None
                }
            }
            FailureKind::GatewayFailure => {
                hs.consecutive_gateway_failure += 1;
                hs.consecutive_5xx = 0;
                if hs.consecutive_gateway_failure >= self.config.consecutive_gateway_failure
                    || was_probing
                {
                    Some(EjectionReason::ConsecutiveGatewayFailure)
                } else {
                    None
                }
            }
        };

        if was_probing {
            warn!(host, cluster = %self.cluster, "outlier: probe failed → re-ejecting");
        }

        if let Some(reason) = reason {
            self.eject_host_inner(hs, reason, total, current_ejected);
        }
    }

    /// Returns `true` when `host` is currently ejected (excluded from selection).
    ///
    /// Also transitions `Ejected → Probing` if the timer has expired.
    pub fn is_ejected(&self, host: &str) -> bool {
        // Try a cheap read first.
        {
            let state = self.state.read();
            if let Some(hs) = state.get(host) {
                if let EjectionState::Ejected { until } = hs.ejection_state {
                    if Instant::now() < until {
                        return true;
                    }
                    // Fall through to write path to transition state.
                } else {
                    return false;
                }
            } else {
                return false;
            }
        }

        // Write path: transition Ejected → Probing.
        {
            let mut state = self.state.write();
            if let Some(hs) = state.get_mut(host) {
                hs.maybe_transition_to_probing();
                // hs borrow ends here; refresh gauge after hs is dropped.
            }
        }
        // Re-acquire read lock to update the gauge outside the write scope.
        {
            let state = self.state.read();
            self.refresh_ejected_gauge_from_read(&state);
        }
        false
    }

    /// Run the success-rate check across all hosts and eject outliers.
    ///
    /// Should be called periodically (e.g. every `interval_ms`).  The LB
    /// core can spawn a background task that calls this method.
    pub fn run_success_rate_check(&self) {
        if !self.config.success_rate_enabled {
            return;
        }

        // Phase 1: read-only snapshot — collect rates and cap-check values.
        let (rates, total, current_ejected) = {
            let state = self.state.read();
            let total = state.len();
            if total < self.config.success_rate_minimum_hosts as usize {
                return;
            }

            let rates: Vec<(String, f64)> = state
                .values()
                .filter_map(|hs| {
                    let rate = hs.success_window.as_ref()?.success_rate()?;
                    Some((hs.address.clone(), rate))
                })
                .collect();

            let ejected = state
                .values()
                .filter(|h| {
                    matches!(h.ejection_state, EjectionState::Ejected { until }
                             if Instant::now() < until)
                })
                .count();

            (rates, total, ejected)
        };

        if rates.len() < self.config.success_rate_minimum_hosts as usize {
            return;
        }

        let n = rates.len() as f64;
        let mean = rates.iter().map(|(_, r)| r).sum::<f64>() / n;
        let variance = rates
            .iter()
            .map(|(_, r)| (r - mean).powi(2))
            .sum::<f64>()
            / n;
        let stdev = variance.sqrt();
        let threshold = mean - stdev * self.config.success_rate_stdev_factor;

        // Phase 2: write path — eject outliers.
        let mut ejected_count = 0u32;
        let mut state = self.state.write();
        // Re-compute current_ejected after acquiring write lock (may have changed).
        let current_ejected_now = state
            .values()
            .filter(|h| {
                matches!(h.ejection_state, EjectionState::Ejected { until }
                         if Instant::now() < until)
            })
            .count();
        let _ = current_ejected; // original read-only value no longer needed

        for (host, rate) in &rates {
            if *rate < threshold {
                if let Some(hs) = state.get_mut(host) {
                    if !hs.is_excluded() {
                        // Compute effective ejected count including already-ejected hosts.
                        let effective_ejected = current_ejected_now + ejected_count as usize;
                        self.eject_host_inner(
                            hs,
                            EjectionReason::SuccessRate,
                            total,
                            effective_ejected,
                        );
                        ejected_count += 1;
                    }
                }
            }
        }

        if ejected_count > 0 {
            warn!(
                cluster = %self.cluster,
                ejected = ejected_count,
                mean_rate = mean,
                threshold,
                "outlier: success-rate check ejected hosts"
            );
        }
    }

    /// Current count of ejected hosts for this cluster.
    pub fn ejected_count(&self) -> usize {
        let state = self.state.read();
        state
            .values()
            .filter(|hs| matches!(hs.ejection_state, EjectionState::Ejected { until } if Instant::now() < until))
            .count()
    }

    // -- internals --

    /// Compute the ejection timeout for a host.
    ///
    /// `consecutive_ejections` is already post-increment (1 on first ejection).
    /// Duration = `base_ejection_time_ms * consecutive_ejections`, matching
    /// Envoy's behaviour: first ejection = 1× base, second = 2× base, etc.
    fn ejection_duration(&self, consecutive_ejections: u32) -> Duration {
        let factor = consecutive_ejections.max(1) as u64;
        let ms = self
            .config
            .base_ejection_time_ms
            .saturating_mul(factor)
            .min(self.config.max_ejection_time_ms);
        Duration::from_millis(ms)
    }

    /// Eject `hs`, respecting the `max_ejection_percent` cap.
    ///
    /// `total` and `current_ejected` must be computed by the caller **before**
    /// acquiring the mutable borrow on `hs` to avoid aliasing issues with the
    /// borrow checker.
    fn eject_host_inner(
        &self,
        hs: &mut HostState,
        reason: EjectionReason,
        total: usize,
        current_ejected: usize,
    ) {
        // Ensure at least 1 host can be ejected when the percentage is non-zero,
        // regardless of cluster size.  Envoy uses ceil for small clusters.
        let max_ejectable = if self.config.max_ejection_percent == 0 {
            0
        } else {
            ((total.max(1) as f64 * self.config.max_ejection_percent as f64) / 100.0)
                .ceil()
                .max(1.0) as usize
        };

        if current_ejected >= max_ejectable {
            debug!(
                host = %hs.address,
                cluster = %self.cluster,
                current_ejected,
                max_ejectable,
                "outlier: max_ejection_percent cap reached; not ejecting"
            );
            return;
        }

        hs.consecutive_ejections += 1;
        let duration = self.ejection_duration(hs.consecutive_ejections);
        let until = Instant::now() + duration;

        warn!(
            host = %hs.address,
            cluster = %self.cluster,
            reason = reason.as_str(),
            duration_ms = duration.as_millis(),
            consecutive_ejections = hs.consecutive_ejections,
            "outlier: ejecting host"
        );

        hs.ejection_state = EjectionState::Ejected { until };

        self.metrics
            .ejections_total
            .with_label_values(&[&self.cluster, reason.as_str()])
            .inc();
    }

    fn refresh_ejected_gauge_from_read(&self, state: &HashMap<String, HostState>) {
        let count = state
            .values()
            .filter(|hs| {
                matches!(hs.ejection_state, EjectionState::Ejected { until } if Instant::now() < until)
            })
            .count();
        self.metrics
            .hosts_ejected
            .with_label_values(&[&self.cluster])
            .set(count as f64);
    }

    fn window_size(&self) -> Option<usize> {
        if self.config.success_rate_enabled {
            Some(self.config.success_rate_request_volume as usize)
        } else {
            None
        }
    }
}

// -- FailureKind --

/// Classification of an upstream failure for outlier counting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureKind {
    /// HTTP 5xx response.
    Http5xx,
    /// TCP-level failure (connection refused, reset, etc.).
    GatewayFailure,
}

// -- tests --

#[cfg(test)]
mod tests {
    use super::*;
    use armageddon_common::types::OutlierDetectionConfig;

    fn cfg_with(consecutive_5xx: u32) -> OutlierDetectionConfig {
        OutlierDetectionConfig {
            consecutive_5xx,
            consecutive_gateway_failure: 5,
            base_ejection_time_ms: 100,
            max_ejection_time_ms: 1_000,
            max_ejection_percent: 50,
            ..Default::default()
        }
    }

    // -----------------------------------------------------------------------
    // Test 1: 5 consecutive 5xx → host ejected
    // -----------------------------------------------------------------------
    #[test]
    fn five_consecutive_5xx_ejects_host() {
        let det = OutlierDetector::new("cluster-a", cfg_with(5));
        let host = "10.0.0.1:8080";

        for _ in 0..4 {
            det.record_failure(host, FailureKind::Http5xx);
            assert!(!det.is_ejected(host), "should not be ejected yet");
        }
        det.record_failure(host, FailureKind::Http5xx); // 5th
        assert!(det.is_ejected(host), "should be ejected after 5 consecutive 5xx");
    }

    // -----------------------------------------------------------------------
    // Test 2: 5 gateway failures → host ejected
    // -----------------------------------------------------------------------
    #[test]
    fn five_gateway_failures_eject_host() {
        let det = OutlierDetector::new("cluster-b", cfg_with(100 /* 5xx threshold irrelevant */));
        let host = "10.0.0.2:8080";

        for _ in 0..4 {
            det.record_failure(host, FailureKind::GatewayFailure);
            assert!(!det.is_ejected(host));
        }
        det.record_failure(host, FailureKind::GatewayFailure); // 5th
        assert!(det.is_ejected(host));
    }

    // -----------------------------------------------------------------------
    // Test 3: Success resets counter → never ejected
    // -----------------------------------------------------------------------
    #[test]
    fn success_resets_consecutive_counter() {
        let det = OutlierDetector::new("cluster-c", cfg_with(3));
        let host = "10.0.0.3:8080";

        det.record_failure(host, FailureKind::Http5xx);
        det.record_failure(host, FailureKind::Http5xx);
        det.record_success(host); // reset
        det.record_failure(host, FailureKind::Http5xx);
        det.record_failure(host, FailureKind::Http5xx);
        // Only 2 consecutive after reset → should NOT be ejected
        assert!(!det.is_ejected(host), "counter should have reset on success");
    }

    // -----------------------------------------------------------------------
    // Test 4: After ejection timer expires, host returns (Probing)
    // -----------------------------------------------------------------------
    // Uses real wall-clock sleep (20ms ejection) to avoid tokio mock-clock
    // interaction with std::time::Instant used internally.
    #[tokio::test]
    async fn host_returns_after_ejection_timeout() {
        let det = OutlierDetector::new("cluster-d", OutlierDetectionConfig {
            consecutive_5xx: 1,
            base_ejection_time_ms: 20,
            max_ejection_time_ms: 500,
            max_ejection_percent: 100,
            ..Default::default()
        });
        let host = "10.0.0.4:8080";

        det.record_failure(host, FailureKind::Http5xx);
        assert!(det.is_ejected(host), "should be ejected immediately");

        // Wait past the ejection duration (20ms + margin).
        tokio::time::sleep(Duration::from_millis(30)).await;

        // After timeout: is_ejected returns false (transitions to Probing).
        assert!(!det.is_ejected(host), "should be available for probing after timeout");
    }

    // -----------------------------------------------------------------------
    // Test 5: Immediate probe failure → re-ejected with doubled timeout
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn probe_failure_doubles_ejection_timeout() {
        let det = OutlierDetector::new("cluster-e", OutlierDetectionConfig {
            consecutive_5xx: 1,
            base_ejection_time_ms: 20,
            max_ejection_time_ms: 10_000,
            max_ejection_percent: 100,
            ..Default::default()
        });
        let host = "10.0.0.5:8080";

        // First ejection: timeout = 20ms * 1 = 20ms.
        det.record_failure(host, FailureKind::Http5xx);
        assert!(det.is_ejected(host));
        assert_eq!(det.ejected_count(), 1);

        // Wait past first ejection (20ms + margin).
        tokio::time::sleep(Duration::from_millis(30)).await;
        assert!(!det.is_ejected(host), "should be probing");

        // Probe fails → second ejection: timeout = 20ms * 2 = 40ms.
        det.record_failure(host, FailureKind::Http5xx);
        assert!(det.is_ejected(host), "should be re-ejected after probe failure");

        // 30ms is not enough for the second ejection (needs 40ms).
        tokio::time::sleep(Duration::from_millis(30)).await;
        assert!(det.is_ejected(host), "should still be ejected (doubled timeout = 40ms)");

        // Wait past doubled timeout.
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(!det.is_ejected(host), "should be probing again after doubled timeout");
    }

    // -----------------------------------------------------------------------
    // Test 6: max_ejection_percent caps ejections
    // -----------------------------------------------------------------------
    #[test]
    fn max_ejection_percent_prevents_full_cluster_ejection() {
        // 4 hosts, max 50% ejectable = 2
        let det = OutlierDetector::new("cluster-f", OutlierDetectionConfig {
            consecutive_5xx: 1,
            max_ejection_percent: 50,
            base_ejection_time_ms: 60_000,
            max_ejection_time_ms: 600_000,
            ..Default::default()
        });

        // Pre-populate 4 hosts with successful state so the map has 4 entries.
        for i in 0..4u32 {
            det.record_success(&format!("10.0.0.{}:8080", i));
        }

        // Attempt to eject all 4.
        for i in 0..4u32 {
            det.record_failure(&format!("10.0.0.{}:8080", i), FailureKind::Http5xx);
        }

        let ejected = det.ejected_count();
        assert!(
            ejected <= 2,
            "max 50% of 4 hosts should be ejected, got {ejected}"
        );
    }

    // -----------------------------------------------------------------------
    // Test 7: ejected_count gauge decrements after timeout
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn ejected_count_decrements_after_timeout() {
        let det = OutlierDetector::new("cluster-g", OutlierDetectionConfig {
            consecutive_5xx: 1,
            base_ejection_time_ms: 20,
            max_ejection_time_ms: 500,
            max_ejection_percent: 100,
            ..Default::default()
        });
        det.record_failure("h1", FailureKind::Http5xx);
        assert_eq!(det.ejected_count(), 1);

        tokio::time::sleep(Duration::from_millis(30)).await;

        // Trigger transition via is_ejected (Ejected → Probing transition updates gauge).
        let _ = det.is_ejected("h1");
        assert_eq!(det.ejected_count(), 0, "ejected count should drop to 0 after timeout");
    }

    // -----------------------------------------------------------------------
    // Test 8: success-rate ejection
    // -----------------------------------------------------------------------
    #[test]
    fn success_rate_ejects_low_performing_host() {
        let det = OutlierDetector::new("cluster-h", OutlierDetectionConfig {
            consecutive_5xx: 100, // disable threshold-based ejection
            consecutive_gateway_failure: 100,
            success_rate_enabled: true,
            success_rate_minimum_hosts: 2,
            success_rate_request_volume: 10,
            success_rate_stdev_factor: 0.1, // tight threshold to force ejection
            base_ejection_time_ms: 60_000,
            max_ejection_time_ms: 600_000,
            max_ejection_percent: 50,
            ..Default::default()
        });

        // good_host: 10/10 success
        for _ in 0..10 {
            det.record_success("good");
        }
        // bad_host: 2/10 success
        for _ in 0..2 {
            det.record_success("bad");
        }
        for _ in 0..8 {
            det.record_failure("bad", FailureKind::Http5xx);
        }

        det.run_success_rate_check();

        assert!(
            det.is_ejected("bad"),
            "low success-rate host should be ejected"
        );
        assert!(
            !det.is_ejected("good"),
            "high success-rate host should not be ejected"
        );
    }
}

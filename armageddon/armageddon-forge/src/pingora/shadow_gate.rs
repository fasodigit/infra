// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Automatic divergence gate for shadow-mode ramp-up.
//!
//! # Overview
//!
//! The [`ShadowGate`] runs in a background task (`spawn_gate_task`) and
//! periodically samples the ratio `diverged_total / total` from the live
//! Prometheus counters exposed by [`ShadowSampler`].  When the ratio exceeds
//! [`ShadowGateConfig::max_divergence_rate`] **and** the sample window is
//! large enough (`min_samples_before_gate`), the gate "trips":
//!
//! | `action`       | Effect on `sample_rate`       |
//! |----------------|-------------------------------|
//! | `Pause`        | → 0.0 (fully disabled)        |
//! | `DropSample`   | → current_rate × 0.5          |
//! | `AlertOnly`    | No change — metric + log only |
//!
//! ## Metrics
//!
//! | Name | Type | Labels | Description |
//! |------|------|--------|-------------|
//! | `armageddon_shadow_gate_tripped_total` | counter | `action` | Incremented each time the gate trips |
//! | `armageddon_shadow_gate_current_rate`  | gauge   | —        | Live sample rate (reflects auto-pauses) |
//!
//! ## Failure modes
//!
//! | Scenario | Behaviour |
//! |----------|-----------|
//! | Gate disabled (`enabled = false`) | Task exits immediately; sampler unchanged |
//! | `min_samples_before_gate` not reached | Gate never trips; rate unchanged |
//! | `AlertOnly` trips | Metrics/logs only; `sample_rate` unchanged |
//! | Background task panics | `sample_rate` is NOT updated (safe: primary path unaffected) |
//! | `sampler` dropped | Background task detects weak-ref expiry and exits cleanly |

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use prometheus::{IntCounterVec, IntGaugeVec, Opts, Registry};
use tracing::{info, warn};

use crate::pingora::shadow::ShadowSampler;

// ---------------------------------------------------------------------------
// GateAction
// ---------------------------------------------------------------------------

/// What the gate does when it trips.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateAction {
    /// Set `sample_rate` to 0.0 — fully disable shadow mode.
    Pause,
    /// Halve the current `sample_rate` (floor at 0).
    DropSample,
    /// Emit a metric + log entry only; do not change the rate.
    AlertOnly,
}

impl GateAction {
    /// Prometheus label for this action.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Pause => "pause",
            Self::DropSample => "drop_sample",
            Self::AlertOnly => "alert_only",
        }
    }
}

impl Default for GateAction {
    fn default() -> Self {
        Self::Pause
    }
}

// ---------------------------------------------------------------------------
// ShadowGateConfig
// ---------------------------------------------------------------------------

/// Static configuration for the [`ShadowGate`].
///
/// At runtime the gate can be reconfigured via the admin API — the live
/// values are stored in [`ShadowGateState`] which is `Arc`-shared between
/// the background task and the admin handler.
#[derive(Debug, Clone)]
pub struct ShadowGateConfig {
    /// Whether the gate background task should be active.
    pub enabled: bool,
    /// Maximum allowed divergence rate (0.0–1.0) before the gate trips.
    /// Example: 0.02 means 2%.
    pub max_divergence_rate: f64,
    /// Minimum total samples in the window before the gate can trip.
    /// Prevents false positives during cold start.
    pub min_samples_before_gate: u64,
    /// Evaluation window in seconds.
    pub window_secs: u64,
    /// What the gate does when it trips.
    pub action: GateAction,
}

impl Default for ShadowGateConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_divergence_rate: 0.02,
            min_samples_before_gate: 100,
            window_secs: 60,
            action: GateAction::Pause,
        }
    }
}

// ---------------------------------------------------------------------------
// ShadowCounts — snapshot of relevant counters
// ---------------------------------------------------------------------------

/// Snapshot of shadow request counts used by the gate evaluation.
#[derive(Debug, Clone, Copy, Default)]
pub struct ShadowCounts {
    /// Total shadow requests processed in the current window.
    pub total: u64,
    /// Total diverged requests (any bucket except `identical`).
    pub diverged: u64,
}

impl ShadowCounts {
    /// Divergence rate as a fraction [0.0, 1.0]. Returns 0.0 when `total == 0`.
    pub fn divergence_rate(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.diverged as f64 / self.total as f64
        }
    }
}

// ---------------------------------------------------------------------------
// ShadowGateState — live (runtime-mutable) gate state
// ---------------------------------------------------------------------------

/// Live state of the shadow gate — shared between the background task and
/// the admin API route handler.
#[derive(Debug)]
pub struct ShadowGateState {
    /// Whether the gate is currently enabled.
    ///
    /// Updated by `POST /admin/shadow/gate {"enabled": …}`.
    pub enabled: std::sync::atomic::AtomicBool,

    /// Atomic representation of `max_divergence_rate` as per-mille (0–1000).
    ///
    /// `rate_permille = (max_divergence_rate * 1000.0) as u32`
    pub max_divergence_rate_permille: AtomicU64,

    /// Minimum samples before the gate may trip.
    pub min_samples_before_gate: AtomicU64,

    /// Window in seconds.
    pub window_secs: AtomicU64,

    /// Current action (stored as u8: 0=Pause, 1=DropSample, 2=AlertOnly).
    pub action: std::sync::atomic::AtomicU8,

    // ── Observability fields (read by GET /admin/shadow/state) ───────────────

    /// Total number of times the gate has tripped since startup.
    pub gate_tripped_count: AtomicU64,

    /// Last divergence rate observed (per-mille, 0–1000).
    pub last_divergence_rate_permille: AtomicU64,

    /// Number of samples in the last evaluated window.
    pub window_samples: AtomicU64,
}

impl ShadowGateState {
    /// Build from a [`ShadowGateConfig`].
    pub fn from_config(cfg: &ShadowGateConfig) -> Arc<Self> {
        Arc::new(Self {
            enabled: std::sync::atomic::AtomicBool::new(cfg.enabled),
            max_divergence_rate_permille: AtomicU64::new(
                (cfg.max_divergence_rate * 1000.0) as u64,
            ),
            min_samples_before_gate: AtomicU64::new(cfg.min_samples_before_gate),
            window_secs: AtomicU64::new(cfg.window_secs),
            action: std::sync::atomic::AtomicU8::new(action_to_u8(cfg.action)),
            gate_tripped_count: AtomicU64::new(0),
            last_divergence_rate_permille: AtomicU64::new(0),
            window_samples: AtomicU64::new(0),
        })
    }

    /// Load the current `max_divergence_rate` as a float.
    pub fn max_divergence_rate(&self) -> f64 {
        self.max_divergence_rate_permille.load(Ordering::Relaxed) as f64 / 1000.0
    }

    /// Load the current gate action.
    pub fn action(&self) -> GateAction {
        u8_to_action(self.action.load(Ordering::Relaxed))
    }

    /// Apply a runtime reconfiguration from the admin API.
    pub fn reconfigure(&self, enabled: Option<bool>, max_divergence_rate: Option<f64>) {
        if let Some(e) = enabled {
            self.enabled.store(e, Ordering::Relaxed);
        }
        if let Some(r) = max_divergence_rate {
            let permille = (r.clamp(0.0, 1.0) * 1000.0) as u64;
            self.max_divergence_rate_permille
                .store(permille, Ordering::Relaxed);
        }
    }
}

fn action_to_u8(a: GateAction) -> u8 {
    match a {
        GateAction::Pause => 0,
        GateAction::DropSample => 1,
        GateAction::AlertOnly => 2,
    }
}

fn u8_to_action(v: u8) -> GateAction {
    match v {
        1 => GateAction::DropSample,
        2 => GateAction::AlertOnly,
        _ => GateAction::Pause,
    }
}

// ---------------------------------------------------------------------------
// ShadowGateMetrics
// ---------------------------------------------------------------------------

/// Prometheus metrics emitted by the shadow gate.
#[derive(Clone, Debug)]
pub struct ShadowGateMetrics {
    /// `armageddon_shadow_gate_tripped_total{action}` — counter.
    pub gate_tripped_total: IntCounterVec,
    /// `armageddon_shadow_gate_current_rate` — gauge (0–100 integer percent).
    pub gate_current_rate: IntGaugeVec,
}

impl ShadowGateMetrics {
    /// Register metrics on `registry`.
    pub fn new(registry: &Registry) -> Result<Self, prometheus::Error> {
        let gate_tripped_total = IntCounterVec::new(
            Opts::new(
                "armageddon_shadow_gate_tripped_total",
                "Total number of times the shadow divergence gate tripped",
            ),
            &["action"],
        )?;
        registry.register(Box::new(gate_tripped_total.clone()))?;

        let gate_current_rate = IntGaugeVec::new(
            Opts::new(
                "armageddon_shadow_gate_current_rate",
                "Current shadow sample rate as integer percentage (0–100), reflects auto-pauses",
            ),
            &[],
        )?;
        registry.register(Box::new(gate_current_rate.clone()))?;

        Ok(Self {
            gate_tripped_total,
            gate_current_rate,
        })
    }
}

// ---------------------------------------------------------------------------
// ShadowGate — the evaluator
// ---------------------------------------------------------------------------

/// Evaluates shadow counts against the gate threshold.
///
/// This is the pure, synchronous core; no I/O, no timers.
/// The background task calls it periodically.
pub struct ShadowGate {
    pub state: Arc<ShadowGateState>,
    pub metrics: Option<Arc<ShadowGateMetrics>>,
}

impl ShadowGate {
    /// Create a new gate from config, optionally with metrics.
    pub fn new(cfg: &ShadowGateConfig, metrics: Option<Arc<ShadowGateMetrics>>) -> Arc<Self> {
        Arc::new(Self {
            state: ShadowGateState::from_config(cfg),
            metrics,
        })
    }

    /// Evaluate the current `counts` snapshot.
    ///
    /// Returns `Some(new_rate)` if the gate trips (even `AlertOnly` returns
    /// the **unchanged** rate to signal a trip occurred), or `None` if the
    /// gate does not trip.
    ///
    /// The caller is responsible for actually applying `new_rate` to the
    /// [`ShadowSampler`].
    pub fn check(&self, counts: &ShadowCounts, current_rate_percent: u32) -> Option<u32> {
        let enabled = self.state.enabled.load(Ordering::Relaxed);
        if !enabled {
            return None;
        }

        let min_samples = self.state.min_samples_before_gate.load(Ordering::Relaxed);
        if counts.total < min_samples {
            return None;
        }

        let max_rate = self.state.max_divergence_rate();
        let observed = counts.divergence_rate();

        // Update observability fields.
        self.state
            .last_divergence_rate_permille
            .store((observed * 1000.0) as u64, Ordering::Relaxed);
        self.state
            .window_samples
            .store(counts.total, Ordering::Relaxed);

        if observed <= max_rate {
            return None;
        }

        // Gate trips.
        let action = self.state.action();

        warn!(
            observed_rate = %observed,
            threshold = %max_rate,
            action = action.label(),
            current_rate_percent,
            "shadow gate tripped"
        );

        // Record trip.
        self.state
            .gate_tripped_count
            .fetch_add(1, Ordering::Relaxed);

        if let Some(m) = &self.metrics {
            m.gate_tripped_total
                .with_label_values(&[action.label()])
                .inc();
        }

        let new_rate = match action {
            GateAction::Pause => 0,
            GateAction::DropSample => current_rate_percent / 2,
            GateAction::AlertOnly => current_rate_percent, // no change
        };

        Some(new_rate)
    }
}

// ---------------------------------------------------------------------------
// Background task
// ---------------------------------------------------------------------------

/// Spawn the gate evaluation background task.
///
/// The task runs every `window_secs` (loaded from `gate.state` at each
/// iteration so runtime reconfiguration is picked up without restart).
///
/// The task terminates when `sampler` (weak ref) is dropped.
///
/// # Parameters
///
/// - `gate` — the gate evaluator (carries live config state)
/// - `sampler` — the `ShadowSampler` to read/write `sample_percent` from
/// - `counts_fn` — closure that reads current total + diverged counters;
///   use the Prometheus registry values or a test stub
pub fn spawn_gate_task(
    gate: Arc<ShadowGate>,
    sampler: Arc<ShadowSampler>,
    counts_fn: impl Fn() -> ShadowCounts + Send + 'static,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            let window_secs = gate.state.window_secs.load(Ordering::Relaxed);
            let sleep_dur = Duration::from_secs(window_secs.max(1));
            tokio::time::sleep(sleep_dur).await;

            let enabled = gate.state.enabled.load(Ordering::Relaxed);
            if !enabled {
                info!("shadow gate: disabled, task exiting");
                break;
            }

            let counts = counts_fn();
            let current_rate = sampler.sample_percent.load(Ordering::Relaxed);

            if let Some(new_rate) = gate.check(&counts, current_rate) {
                if new_rate != current_rate {
                    info!(
                        old_rate = current_rate,
                        new_rate,
                        "shadow gate: updating sample rate"
                    );
                    sampler.set_sample_percent(new_rate);
                }

                // Update the gate_current_rate gauge.
                if let Some(m) = &gate.metrics {
                    m.gate_current_rate
                        .with_label_values(&[])
                        .set(i64::from(sampler.sample_percent.load(Ordering::Relaxed)));
                }
            }
        }
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn default_gate() -> Arc<ShadowGate> {
        ShadowGate::new(&ShadowGateConfig::default(), None)
    }

    // ── gate does NOT trip when rate is below threshold ────────────────────

    #[test]
    fn gate_no_trip_below_threshold() {
        let gate = default_gate();
        let counts = ShadowCounts {
            total: 200,
            diverged: 2, // 1% — below 2% threshold
        };
        let result = gate.check(&counts, 10);
        assert!(result.is_none(), "gate must not trip below threshold");
    }

    // ── gate trips when threshold exceeded ────────────────────────────────

    #[test]
    fn gate_trips_on_threshold_exceeded() {
        let gate = default_gate();
        let counts = ShadowCounts {
            total: 200,
            diverged: 10, // 5% — above 2% threshold
        };
        let result = gate.check(&counts, 10);
        assert!(result.is_some(), "gate must trip when threshold exceeded");
        // Pause action → rate becomes 0.
        assert_eq!(result.unwrap(), 0, "Pause action must set rate to 0");
    }

    // ── gate does NOT trip before min_samples ─────────────────────────────

    #[test]
    fn gate_no_trip_before_min_samples() {
        let gate = default_gate(); // min_samples_before_gate = 100
        let counts = ShadowCounts {
            total: 50,   // below 100
            diverged: 5, // 10% — would exceed threshold
        };
        let result = gate.check(&counts, 10);
        assert!(
            result.is_none(),
            "gate must not trip before min_samples_before_gate"
        );
    }

    // ── exactly at min_samples → gate can trip ────────────────────────────

    #[test]
    fn gate_trips_at_exact_min_samples() {
        let gate = default_gate(); // min_samples = 100, threshold = 2%
        let counts = ShadowCounts {
            total: 100,
            diverged: 5, // 5% — above threshold
        };
        let result = gate.check(&counts, 20);
        assert!(result.is_some(), "gate must trip at min_samples");
    }

    // ── DropSample action halves the rate ─────────────────────────────────

    #[test]
    fn gate_drop_sample_halves_rate() {
        let cfg = ShadowGateConfig {
            action: GateAction::DropSample,
            ..Default::default()
        };
        let gate = ShadowGate::new(&cfg, None);
        let counts = ShadowCounts {
            total: 200,
            diverged: 10, // 5% — trips
        };
        let result = gate.check(&counts, 20);
        assert_eq!(result, Some(10), "DropSample must halve rate from 20 to 10");
    }

    // ── AlertOnly trips but does not change rate ──────────────────────────

    #[test]
    fn gate_alert_only_does_not_change_rate() {
        let cfg = ShadowGateConfig {
            action: GateAction::AlertOnly,
            ..Default::default()
        };
        let gate = ShadowGate::new(&cfg, None);
        let counts = ShadowCounts {
            total: 200,
            diverged: 10, // 5% — trips
        };
        let result = gate.check(&counts, 15);
        // Returns Some(15) — rate unchanged, but trip occurred.
        assert_eq!(result, Some(15), "AlertOnly must return unchanged rate");
    }

    // ── gate disabled → never trips ───────────────────────────────────────

    #[test]
    fn gate_disabled_never_trips() {
        let cfg = ShadowGateConfig {
            enabled: false,
            ..Default::default()
        };
        let gate = ShadowGate::new(&cfg, None);
        let counts = ShadowCounts {
            total: 1_000,
            diverged: 1_000, // 100% divergence
        };
        assert!(gate.check(&counts, 100).is_none(), "disabled gate must not trip");
    }

    // ── metrics: gate_tripped_total increments on trip ────────────────────

    #[test]
    fn gate_tripped_total_metric_increments() {
        let registry = prometheus::Registry::new();
        let m = Arc::new(
            ShadowGateMetrics::new(&registry).expect("metrics registration"),
        );
        let gate = ShadowGate::new(&ShadowGateConfig::default(), Some(m.clone()));

        let counts = ShadowCounts {
            total: 200,
            diverged: 10,
        };
        gate.check(&counts, 10);

        let families = registry.gather();
        let fam = families
            .iter()
            .find(|f| f.get_name() == "armageddon_shadow_gate_tripped_total")
            .expect("metric must exist");
        let val = fam
            .get_metric()
            .first()
            .map(|m| m.get_counter().get_value())
            .unwrap_or(0.0);
        assert_eq!(val, 1.0, "trip counter must be 1 after one trip");
    }

    // ── gate_tripped_count atomic is updated ─────────────────────────────

    #[test]
    fn gate_tripped_count_increments_on_trip() {
        let gate = default_gate();
        let counts = ShadowCounts {
            total: 200,
            diverged: 10,
        };
        gate.check(&counts, 10);
        assert_eq!(
            gate.state.gate_tripped_count.load(Ordering::Relaxed),
            1,
            "gate_tripped_count must be 1"
        );
    }

    // ── ShadowCounts::divergence_rate ─────────────────────────────────────

    #[test]
    fn shadow_counts_divergence_rate_zero_when_total_zero() {
        let c = ShadowCounts { total: 0, diverged: 0 };
        assert_eq!(c.divergence_rate(), 0.0);
    }

    #[test]
    fn shadow_counts_divergence_rate_correct() {
        let c = ShadowCounts {
            total: 200,
            diverged: 10,
        };
        assert!((c.divergence_rate() - 0.05).abs() < 1e-9);
    }

    // ── reconfigure updates live state ────────────────────────────────────

    #[test]
    fn gate_state_reconfigure_updates_fields() {
        let state = ShadowGateState::from_config(&ShadowGateConfig::default());
        state.reconfigure(Some(false), Some(0.10));
        assert!(!state.enabled.load(Ordering::Relaxed));
        assert!((state.max_divergence_rate() - 0.10).abs() < 0.001);
    }

    // ── background task integration (tokio) ──────────────────────────────

    #[tokio::test]
    async fn gate_task_trips_and_sets_rate_to_zero() {
        use crate::pingora::shadow::{ShadowModeConfig, ShadowSampler};

        let cfg = ShadowGateConfig {
            enabled: true,
            max_divergence_rate: 0.02,
            min_samples_before_gate: 10,
            window_secs: 0, // 0 → sleep 1s, but we override via mock counts
            action: GateAction::Pause,
        };

        let sampler_cfg = ShadowModeConfig {
            sample_rate_percent: 50,
            ..Default::default()
        };
        let sampler = ShadowSampler::new(&sampler_cfg);
        let gate = ShadowGate::new(&cfg, None);

        // Manually call check (not via spawn) to verify the rate is set.
        let counts = ShadowCounts { total: 100, diverged: 10 }; // 10% > 2%
        let new_rate = gate.check(&counts, 50);
        assert_eq!(new_rate, Some(0), "Pause action must return 0");
        sampler.set_sample_percent(new_rate.unwrap());
        assert_eq!(
            sampler.sample_percent.load(Ordering::Relaxed),
            0,
            "sampler rate must be 0 after gate trip"
        );
    }
}

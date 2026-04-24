// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Circuit breaker — Closed / Open / HalfOpen state machine per upstream
//! cluster.
//!
//! This module ports `src/circuit_breaker.rs` (226 LOC) into the Pingora
//! gateway path.  The hooks are:
//!
//! - **`fail_to_proxy`** — called when Pingora cannot forward the request
//!   (TCP error, timeout before any upstream byte).  Increments
//!   `consecutive_connect_fail`.
//! - **`upstream_response_filter`** — called for every upstream response.
//!   Increments `consecutive_5xx` on HTTP 5xx; resets on 2xx.
//!
//! # State machine
//!
//! ```text
//!  Closed ──(threshold consecutive failures)──► Open
//!    ▲                                           │
//!    │ success in HalfOpen                       │ cooldown elapsed
//!    └──────── HalfOpen ◄───────────────────────┘
//!                  │
//!                  └─(failure in HalfOpen)──► Open (cooldown doubled)
//! ```
//!
//! # Storage
//!
//! States are stored in a `DashMap<String, Arc<BreakerState>>`.  The
//! `DashMap` is wrapped in an `Arc` so it can be shared cheaply across
//! filter instances / requests.  State transitions use an internal
//! `RwLock<CircuitState>` per cluster — writes are brief and contention is
//! low because the lock is only held during state transitions, not during
//! normal request handling.
//!
//! # Failure modes
//!
//! | Scenario | Behaviour |
//! |---|---|
//! | Circuit open | `allow_request()` returns `false`; Pingora hook returns `503` with `Retry-After` |
//! | Half-open probe succeeds | Transition back to Closed; normal traffic resumes |
//! | Half-open probe fails | Transition back to Open; cooldown doubled |
//! | Concurrent updates | `DashMap` + `AtomicU32` ensure no torn reads; `RwLock` for state transitions |
//!
//! # Adaptive EWMA thresholds (mode = "adaptive")
//!
//! When [`BreakerMode::Adaptive`] is selected, the static `error_rate_threshold`
//! is replaced by a dynamic baseline tracked via an Exponentially Weighted
//! Moving Average (EWMA).  Trip conditions:
//!
//! 1. **Adaptive spike**: `current_error_rate > trip_multiplier × ewma_baseline`
//!    AND `current_error_rate > min_error_rate_floor`
//!    (protects against tripping on zero-baseline noise).
//! 2. **Absolute ceiling**: `current_error_rate > max_error_rate_ceiling`
//!    (always trips regardless of baseline — catches runaway degraded upstreams).
//! 3. **Consecutive 5xx / connect-fail** thresholds — unchanged from static mode.
//!
//! The EWMA is stored as an `AtomicU64` bit-cast from `f64` (`f64::to_bits` /
//! `f64::from_bits`) to allow lock-free updates on the hot path.
//!
//! ## Metrics (adaptive mode)
//!
//! - `armageddon_circuit_breaker_ewma_baseline{cluster,endpoint}` — current EWMA
//! - `armageddon_circuit_breaker_trip_reason_total{cluster,endpoint,reason}` —
//!   reason labels: `"adaptive_x_baseline"`, `"absolute_ceiling"`, `"consecutive_5xx"`

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use dashmap::DashMap;
use tracing::{info, warn};

// ── EWMA helper ───────────────────────────────────────────────────────────────

/// Atomic f64 stored as `AtomicU64` via `f64::to_bits` / `f64::from_bits`.
///
/// This pattern is safe because:
/// - `f64` and `u64` have the same bit width (64 bits).
/// - All possible bit patterns are valid for `u64`; for `f64` most are valid
///   (NaN payloads are allowed by IEEE 754).  We clamp to `[0.0, 1.0]` on
///   every write so we never store a NaN.
/// - `compare_exchange` on `AtomicU64` gives us a CAS loop for the EWMA
///   update without an external lock.
struct AtomicF64(AtomicU64);

impl AtomicF64 {
    const fn new(val: f64) -> Self {
        Self(AtomicU64::new(val.to_bits()))
    }

    fn load(&self, ord: Ordering) -> f64 {
        f64::from_bits(self.0.load(ord))
    }

    /// CAS-loop EWMA update: `new = alpha * sample + (1 - alpha) * old`.
    ///
    /// Clamps result to `[0.0, 1.0]`.  Retries on contention (rare).
    fn ewma_update(&self, alpha: f64, sample: f64) {
        loop {
            let old_bits = self.0.load(Ordering::Relaxed);
            let old = f64::from_bits(old_bits);
            let new = (alpha * sample + (1.0 - alpha) * old).clamp(0.0, 1.0);
            let new_bits = new.to_bits();
            if self
                .0
                .compare_exchange(old_bits, new_bits, Ordering::Release, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }
        }
    }
}

impl std::fmt::Debug for AtomicF64 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.6}", self.load(Ordering::Relaxed))
    }
}

// ── circuit state ──────────────────────────────────────────────────────────────

/// The three states of a circuit breaker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Normal operation — requests are forwarded.
    Closed,
    /// Circuit open — requests are rejected immediately with `503`.
    Open,
    /// Cooldown elapsed — one probe request is allowed through.
    HalfOpen,
}

// ── thresholds / configuration ─────────────────────────────────────────────────

/// Thresholds that control when the circuit opens.
#[derive(Debug, Clone)]
pub struct BreakerConfig {
    /// Number of consecutive 5xx responses before opening.
    pub consecutive_5xx_threshold: u32,
    /// Number of consecutive connect failures before opening.
    pub consecutive_connect_fail_threshold: u32,
    /// Error rate (0.0–1.0) over the sliding window before opening.
    /// `None` disables error-rate tracking.
    pub error_rate_threshold: Option<f32>,
    /// Sliding window size for error-rate calculation.
    pub window_size: usize,
    /// Initial cooldown before the first HalfOpen probe.
    pub initial_cooldown: Duration,
    /// Maximum cooldown after repeated Open/HalfOpen cycles.
    pub max_cooldown: Duration,
}

impl Default for BreakerConfig {
    fn default() -> Self {
        Self {
            consecutive_5xx_threshold: 5,
            consecutive_connect_fail_threshold: 3,
            error_rate_threshold: Some(0.50),
            window_size: 20,
            initial_cooldown: Duration::from_secs(30),
            max_cooldown: Duration::from_secs(300),
        }
    }
}

// ── adaptive EWMA configuration ───────────────────────────────────────────────

/// Selects the threshold mode for a circuit breaker.
///
/// Default is [`BreakerMode::Static`] to preserve backwards compatibility
/// with existing deployments that rely on `BreakerConfig` alone.
#[derive(Debug, Clone, PartialEq)]
pub enum BreakerMode {
    /// Use the fixed thresholds in [`BreakerConfig`] (default).
    Static,
    /// Use EWMA-adaptive thresholds defined in [`EwmaBreakerConfig`].
    /// The static `consecutive_5xx_threshold` and `consecutive_connect_fail_threshold`
    /// from [`BreakerConfig`] still apply as additional trip conditions.
    Adaptive(EwmaBreakerConfig),
}

impl Default for BreakerMode {
    fn default() -> Self {
        BreakerMode::Static
    }
}

/// Configuration for adaptive EWMA-based circuit-breaker thresholds.
///
/// Used when `BreakerMode::Adaptive` is selected.  The EWMA baseline tracks
/// the long-run error rate; the trip conditions are:
///
/// 1. `current_rate > trip_multiplier × ewma_baseline` AND
///    `current_rate > min_error_rate_floor`
/// 2. `current_rate > max_error_rate_ceiling` (unconditional trip)
///
/// Both conditions are evaluated on every `record_5xx` / `record_connect_fail`
/// call.  Either condition alone is sufficient to open the circuit.
#[derive(Debug, Clone, PartialEq)]
pub struct EwmaBreakerConfig {
    /// Lookback window in seconds.  Longer windows mean the baseline adapts
    /// more slowly.  Typical value: `300` (5 minutes).
    ///
    /// Used only for documentation / operator visibility; the actual EWMA
    /// decay is controlled by `alpha`.
    pub window_secs: u64,

    /// EWMA decay factor (0 < alpha ≤ 1).  Closer to 1 = more weight on
    /// recent samples (faster adaptation).  Closer to 0 = more historical
    /// memory (slower adaptation).
    ///
    /// Typical value: `0.05` (≈ 5 % weight on the new sample — roughly
    /// equivalent to a 20-sample EMA).
    pub alpha: f64,

    /// Multiplier applied to the EWMA baseline to compute the adaptive trip
    /// threshold.  Circuit opens when:
    /// `current_rate > trip_multiplier * ewma_baseline`
    /// AND `current_rate > min_error_rate_floor`.
    ///
    /// Typical value: `3.0` (trip at 3× the historical baseline).
    pub trip_multiplier: f64,

    /// Absolute floor below which the adaptive trip cannot occur.
    ///
    /// Prevents tripping on a brief spike when the baseline is essentially
    /// zero.  For example, with `min_error_rate_floor = 0.02`, a 1% spike
    /// from a zero baseline will not trip the circuit even though
    /// `3 × 0% = 0% < 1%`.
    ///
    /// Typical value: `0.02` (2%).
    pub min_error_rate_floor: f64,

    /// Absolute ceiling.  When `current_rate > max_error_rate_ceiling`, the
    /// circuit always opens regardless of the EWMA baseline.  This prevents
    /// a legitimately high baseline from masking a total upstream failure.
    ///
    /// Typical value: `0.50` (50%).
    pub max_error_rate_ceiling: f64,
}

impl Default for EwmaBreakerConfig {
    fn default() -> Self {
        Self {
            window_secs: 300,
            alpha: 0.05,
            trip_multiplier: 3.0,
            min_error_rate_floor: 0.02,
            max_error_rate_ceiling: 0.50,
        }
    }
}

// ── per-cluster breaker state ─────────────────────────────────────────────────

/// Internal mutable state for one cluster's circuit breaker.
struct Inner {
    state: CircuitState,
    /// Timestamp when the circuit was last opened.
    opened_at: Option<Instant>,
    /// Current cooldown duration (starts at `config.initial_cooldown`, doubles on failure).
    current_cooldown: Duration,
    /// Consecutive 5xx response count.
    consecutive_5xx: u32,
    /// Consecutive connect failure count.
    consecutive_connect_fail: u32,
    /// Sliding window for error-rate tracking (`true` = success, `false` = failure).
    window: std::collections::VecDeque<bool>,
}

impl Inner {
    fn new(config: &BreakerConfig) -> Self {
        Self {
            state: CircuitState::Closed,
            opened_at: None,
            current_cooldown: config.initial_cooldown,
            consecutive_5xx: 0,
            consecutive_connect_fail: 0,
            window: std::collections::VecDeque::with_capacity(config.window_size),
        }
    }
}

/// Per-cluster circuit breaker.
///
/// Stored in the `CircuitBreakerManager`'s `DashMap`; each cluster gets one
/// `BreakerState` that is never removed (only reset).
pub struct BreakerState {
    pub config: BreakerConfig,
    /// Threshold mode (static or adaptive).
    pub mode: BreakerMode,
    /// Atomic counters for lock-free reads in the hot path.
    pub active_requests: AtomicU32,
    /// EWMA baseline error rate (adaptive mode only).
    ///
    /// Stored as `AtomicU64` bit-cast from `f64`.  Updated on every
    /// `record_success` / `record_5xx` / `record_connect_fail` call when
    /// `mode == Adaptive`.  Safe to ignore (stays `0.0`) in static mode.
    ewma_error_rate: AtomicF64,
    inner: RwLock<Inner>,
}

impl std::fmt::Debug for BreakerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let inner = self.inner.read().unwrap();
        f.debug_struct("BreakerState")
            .field("state", &inner.state)
            .field("consecutive_5xx", &inner.consecutive_5xx)
            .field("consecutive_connect_fail", &inner.consecutive_connect_fail)
            .field("ewma_error_rate", &self.ewma_error_rate)
            .finish()
    }
}

impl BreakerState {
    /// Create a new breaker in the `Closed` state with static thresholds.
    pub fn new(config: BreakerConfig) -> Self {
        Self::new_with_mode(config, BreakerMode::Static)
    }

    /// Create a new breaker with an explicit [`BreakerMode`].
    ///
    /// Use [`BreakerMode::Adaptive`] to enable EWMA-based thresholds.
    pub fn new_with_mode(config: BreakerConfig, mode: BreakerMode) -> Self {
        let inner = Inner::new(&config);
        Self {
            config,
            mode,
            active_requests: AtomicU32::new(0),
            ewma_error_rate: AtomicF64::new(0.0),
            inner: RwLock::new(inner),
        }
    }

    /// Return the current EWMA baseline error rate.
    ///
    /// Always `0.0` in static mode.  In adaptive mode this reflects the
    /// long-run error fraction weighted by `alpha`.
    pub fn ewma_baseline(&self) -> f64 {
        self.ewma_error_rate.load(Ordering::Relaxed)
    }

    /// Return the current circuit state, transitioning `Open → HalfOpen` if
    /// the cooldown has elapsed.
    pub fn state(&self) -> CircuitState {
        self.maybe_transition_to_half_open();
        self.inner.read().unwrap().state
    }

    /// Return `true` when the circuit allows the next request through.
    ///
    /// - `Closed` → always allowed.
    /// - `HalfOpen` → allowed only when no active probe is in flight
    ///   (`active_requests == 0`).
    /// - `Open` → never allowed.
    pub fn allow_request(&self) -> bool {
        self.maybe_transition_to_half_open();
        let inner = self.inner.read().unwrap();
        match inner.state {
            CircuitState::Closed => true,
            CircuitState::HalfOpen => self.active_requests.load(Ordering::Relaxed) < 1,
            CircuitState::Open => false,
        }
    }

    /// Record a successful upstream response (2xx).
    ///
    /// Resets consecutive failure counters.  Transitions `HalfOpen → Closed`.
    /// In adaptive mode updates the EWMA baseline with a `0.0` (success) sample.
    pub fn record_success(&self) {
        // Update EWMA before acquiring the lock — the CAS loop in `ewma_update`
        // is contention-free in the common case (Ordering::Relaxed load).
        self.update_ewma(false);

        let mut inner = self.inner.write().unwrap();

        // Sliding window.
        self.push_window(&mut inner, true);

        inner.consecutive_5xx = 0;
        inner.consecutive_connect_fail = 0;

        if inner.state == CircuitState::HalfOpen {
            inner.state = CircuitState::Closed;
            inner.opened_at = None;
            inner.current_cooldown = self.config.initial_cooldown; // reset doubling
            info!("circuit breaker: HalfOpen → Closed (probe succeeded)");
        }
    }

    /// Record a 5xx upstream response.
    ///
    /// May transition `Closed → Open` or `HalfOpen → Open` (with doubled cooldown).
    /// In adaptive mode updates the EWMA baseline with a `1.0` (failure) sample
    /// and evaluates adaptive trip conditions.
    pub fn record_5xx(&self) {
        self.update_ewma(true);

        let mut inner = self.inner.write().unwrap();

        self.push_window(&mut inner, false);

        inner.consecutive_5xx += 1;
        inner.consecutive_connect_fail = 0; // 5xx means we did connect

        if inner.state == CircuitState::HalfOpen {
            // Probe failed — double the cooldown and re-open.
            let new_cooldown = (inner.current_cooldown * 2).min(self.config.max_cooldown);
            inner.current_cooldown = new_cooldown;
            inner.state = CircuitState::Open;
            inner.opened_at = Some(Instant::now());
            warn!(
                cooldown_secs = new_cooldown.as_secs(),
                "circuit breaker: HalfOpen → Open (probe failed with 5xx)"
            );
            return;
        }

        let trip_reason = self.should_open_static(&inner)
            .or_else(|| self.should_open_adaptive(&inner));

        if let Some(reason) = trip_reason {
            if inner.state == CircuitState::Closed {
                inner.state = CircuitState::Open;
                inner.opened_at = Some(Instant::now());
                warn!(
                    consecutive_5xx = inner.consecutive_5xx,
                    reason = reason.as_str(),
                    ewma_baseline = self.ewma_baseline(),
                    "circuit breaker: Closed → Open"
                );
            }
        }
    }

    /// Record a connect failure (TCP error, connection refused, timeout).
    ///
    /// May transition `Closed → Open`.
    /// In adaptive mode updates the EWMA baseline with a `1.0` (failure) sample.
    pub fn record_connect_fail(&self) {
        self.update_ewma(true);

        let mut inner = self.inner.write().unwrap();

        self.push_window(&mut inner, false);

        inner.consecutive_connect_fail += 1;
        inner.consecutive_5xx = 0;

        if inner.state == CircuitState::HalfOpen {
            let new_cooldown = (inner.current_cooldown * 2).min(self.config.max_cooldown);
            inner.current_cooldown = new_cooldown;
            inner.state = CircuitState::Open;
            inner.opened_at = Some(Instant::now());
            warn!(
                cooldown_secs = new_cooldown.as_secs(),
                "circuit breaker: HalfOpen → Open (connect fail)"
            );
            return;
        }

        let trip_reason = self.should_open_static(&inner)
            .or_else(|| self.should_open_adaptive(&inner));

        if let Some(reason) = trip_reason {
            if inner.state == CircuitState::Closed {
                inner.state = CircuitState::Open;
                inner.opened_at = Some(Instant::now());
                warn!(
                    consecutive_connect_fail = inner.consecutive_connect_fail,
                    reason = reason.as_str(),
                    ewma_baseline = self.ewma_baseline(),
                    "circuit breaker: Closed → Open"
                );
            }
        }
    }

    /// Remaining cooldown seconds when the circuit is `Open`.
    ///
    /// Returns `0` when the circuit is `Closed` or `HalfOpen`.
    pub fn cooldown_remaining(&self) -> Duration {
        let inner = self.inner.read().unwrap();
        if inner.state != CircuitState::Open {
            return Duration::ZERO;
        }
        let opened = match inner.opened_at {
            Some(t) => t,
            None => return Duration::ZERO,
        };
        let elapsed = opened.elapsed();
        if elapsed >= inner.current_cooldown {
            Duration::ZERO
        } else {
            inner.current_cooldown - elapsed
        }
    }

    // ── private helpers ───────────────────────────────────────────────────────

    /// Transition `Open → HalfOpen` if the cooldown has elapsed.
    fn maybe_transition_to_half_open(&self) {
        let state = { self.inner.read().unwrap().state };
        if state != CircuitState::Open {
            return;
        }
        let (elapsed, cooldown) = {
            let inner = self.inner.read().unwrap();
            let opened = match inner.opened_at {
                Some(t) => t,
                None => return,
            };
            (opened.elapsed(), inner.current_cooldown)
        };
        if elapsed >= cooldown {
            let mut inner = self.inner.write().unwrap();
            if inner.state == CircuitState::Open {
                inner.state = CircuitState::HalfOpen;
                info!(
                    cooldown_secs = cooldown.as_secs(),
                    "circuit breaker: Open → HalfOpen (cooldown elapsed)"
                );
            }
        }
    }

    fn push_window(&self, inner: &mut Inner, success: bool) {
        if self.config.window_size == 0 {
            return;
        }
        inner.window.push_back(success);
        while inner.window.len() > self.config.window_size {
            inner.window.pop_front();
        }
    }

    fn error_rate_exceeded(&self, inner: &Inner) -> bool {
        let threshold = match self.config.error_rate_threshold {
            Some(t) => t,
            None => return false,
        };
        if inner.window.len() < self.config.window_size {
            return false; // not enough data
        }
        let failures = inner.window.iter().filter(|&&ok| !ok).count();
        let rate = failures as f32 / inner.window.len() as f32;
        rate >= threshold
    }

    // ── adaptive helpers ───────────────────────────────────────────────────────

    /// Update the EWMA baseline.  `is_failure = true` pushes toward 1.0,
    /// `false` toward 0.0.  No-op in static mode.
    ///
    /// Exposed as `pub(crate)` for unit tests that need to drive the EWMA
    /// baseline without also touching the sliding window / state machine.
    pub(crate) fn update_ewma(&self, is_failure: bool) {
        if let BreakerMode::Adaptive(ref cfg) = self.mode {
            let sample = if is_failure { 1.0_f64 } else { 0.0_f64 };
            self.ewma_error_rate.ewma_update(cfg.alpha, sample);
        }
    }

    /// Static trip check (backwards-compatible).  Returns `Some(reason)` when
    /// the static thresholds are exceeded.
    fn should_open_static(&self, inner: &Inner) -> Option<TripReason> {
        if inner.consecutive_5xx >= self.config.consecutive_5xx_threshold
            || inner.consecutive_connect_fail
                >= self.config.consecutive_connect_fail_threshold
        {
            return Some(TripReason::Consecutive5xx);
        }
        if self.error_rate_exceeded(inner) {
            return Some(TripReason::Consecutive5xx);
        }
        None
    }

    /// Adaptive trip check.  Returns `Some(reason)` when adaptive thresholds
    /// are exceeded.  No-op in static mode (always returns `None`).
    ///
    /// This method reads the current sliding-window error rate from `inner`
    /// rather than the EWMA baseline, so the trip decision is based on the
    /// most recent window, while the EWMA provides the adaptive reference.
    fn should_open_adaptive(&self, inner: &Inner) -> Option<TripReason> {
        let ewma_cfg = match &self.mode {
            BreakerMode::Adaptive(c) => c,
            BreakerMode::Static => return None,
        };

        // Compute the current window error rate.  If the window is not yet
        // full, use the available samples (do not require a full window for
        // adaptive mode — unlike static mode).
        if inner.window.is_empty() {
            return None;
        }
        let failures = inner.window.iter().filter(|&&ok| !ok).count();
        let current_rate = failures as f64 / inner.window.len() as f64;

        // Condition 2 — absolute ceiling (checked first; highest priority).
        if current_rate > ewma_cfg.max_error_rate_ceiling {
            return Some(TripReason::AbsoluteCeiling);
        }

        // Condition 1 — adaptive spike: N× the EWMA baseline AND above floor.
        let baseline = self.ewma_baseline();
        let adaptive_threshold = (ewma_cfg.trip_multiplier * baseline)
            .max(ewma_cfg.min_error_rate_floor);
        if current_rate > adaptive_threshold && current_rate > ewma_cfg.min_error_rate_floor {
            return Some(TripReason::AdaptiveXBaseline);
        }

        None
    }
}

// ── trip reason ────────────────────────────────────────────────────────────────

/// Reason label for the `armageddon_circuit_breaker_trip_reason_total` metric.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TripReason {
    /// Error rate exceeded `trip_multiplier × ewma_baseline` (adaptive mode).
    AdaptiveXBaseline,
    /// Error rate exceeded `max_error_rate_ceiling` (adaptive mode).
    AbsoluteCeiling,
    /// Consecutive 5xx / connect-fail count exceeded threshold (both modes).
    Consecutive5xx,
}

impl TripReason {
    /// Prometheus label value for `reason` dimension.
    pub fn as_str(self) -> &'static str {
        match self {
            TripReason::AdaptiveXBaseline => "adaptive_x_baseline",
            TripReason::AbsoluteCeiling   => "absolute_ceiling",
            TripReason::Consecutive5xx    => "consecutive_5xx",
        }
    }
}

// ── manager ────────────────────────────────────────────────────────────────────

/// Manages circuit breakers for all upstream clusters.
///
/// `DashMap` provides per-shard locking so concurrent requests to different
/// clusters never contend.  The `Arc<BreakerState>` inside the map is
/// `Send + Sync` and can be cloned cheaply.
#[derive(Debug, Default)]
pub struct CircuitBreakerManager {
    breakers: Arc<DashMap<String, Arc<BreakerState>>>,
}

impl CircuitBreakerManager {
    /// Create an empty manager.
    pub fn new() -> Self {
        Self {
            breakers: Arc::new(DashMap::new()),
        }
    }

    /// Register a cluster with the given config.  Idempotent — re-registering
    /// an existing cluster replaces its config **and** resets its state.
    ///
    /// Uses static thresholds.  For adaptive EWMA mode use
    /// [`Self::register_adaptive`].
    pub fn register(&self, cluster: &str, config: BreakerConfig) {
        self.breakers.insert(
            cluster.to_string(),
            Arc::new(BreakerState::new(config)),
        );
    }

    /// Register a cluster with adaptive EWMA thresholds.
    ///
    /// The static `consecutive_5xx_threshold` and `consecutive_connect_fail_threshold`
    /// from `config` still apply as additional trip conditions.
    pub fn register_adaptive(
        &self,
        cluster: &str,
        config: BreakerConfig,
        ewma: EwmaBreakerConfig,
    ) {
        self.breakers.insert(
            cluster.to_string(),
            Arc::new(BreakerState::new_with_mode(config, BreakerMode::Adaptive(ewma))),
        );
    }

    /// Look up the breaker for `cluster`.  Returns `None` when the cluster has
    /// not been registered (all-pass: unknown clusters are not gated).
    pub fn get(&self, cluster: &str) -> Option<Arc<BreakerState>> {
        self.breakers.get(cluster).map(|v| Arc::clone(&v))
    }

    /// Get-or-create with default config.  Useful when the cluster list is
    /// not known at startup (e.g. xDS dynamic cluster discovery).
    pub fn get_or_default(&self, cluster: &str) -> Arc<BreakerState> {
        self.breakers
            .entry(cluster.to_string())
            .or_insert_with(|| Arc::new(BreakerState::new(BreakerConfig::default())))
            .clone()
    }

    /// Check whether the circuit for `cluster` allows the current request.
    ///
    /// Returns `true` when the cluster is unknown (fail-open for undiscovered
    /// clusters so bootstrapping traffic is not blocked).
    pub fn allow_request(&self, cluster: &str) -> bool {
        match self.get(cluster) {
            Some(b) => b.allow_request(),
            None => true,
        }
    }

    /// `Retry-After` value in whole seconds to put in the 503 response when
    /// the circuit is open.
    pub fn retry_after_secs(&self, cluster: &str) -> u64 {
        match self.get(cluster) {
            Some(b) => b.cooldown_remaining().as_secs().max(1),
            None => 1,
        }
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    fn fast_config() -> BreakerConfig {
        BreakerConfig {
            consecutive_5xx_threshold: 3,
            consecutive_connect_fail_threshold: 2,
            error_rate_threshold: None, // disable for deterministic tests
            window_size: 10,
            initial_cooldown: Duration::from_millis(50),
            max_cooldown: Duration::from_secs(300),
        }
    }

    // ── state transitions ──────────────────────────────────────────────────────

    #[test]
    fn starts_in_closed_state() {
        let b = BreakerState::new(fast_config());
        assert_eq!(b.state(), CircuitState::Closed);
        assert!(b.allow_request());
    }

    #[test]
    fn opens_after_consecutive_5xx_threshold() {
        let b = BreakerState::new(fast_config());
        b.record_5xx();
        b.record_5xx();
        assert_eq!(b.state(), CircuitState::Closed); // 2 < 3
        b.record_5xx(); // 3 >= 3 → Open
        assert_eq!(b.state(), CircuitState::Open);
        assert!(!b.allow_request());
    }

    #[test]
    fn opens_after_connect_fail_threshold() {
        let b = BreakerState::new(fast_config());
        b.record_connect_fail();
        assert_eq!(b.state(), CircuitState::Closed); // 1 < 2
        b.record_connect_fail(); // 2 >= 2 → Open
        assert_eq!(b.state(), CircuitState::Open);
        assert!(!b.allow_request());
    }

    #[test]
    fn success_resets_consecutive_counters() {
        let b = BreakerState::new(fast_config());
        b.record_5xx();
        b.record_5xx();
        b.record_success(); // reset
        assert_eq!(b.state(), CircuitState::Closed);
        // After reset we need threshold hits again.
        b.record_5xx();
        b.record_5xx();
        assert_eq!(b.state(), CircuitState::Closed);
    }

    #[test]
    fn transitions_open_to_half_open_after_cooldown() {
        let b = BreakerState::new(fast_config());
        b.record_5xx();
        b.record_5xx();
        b.record_5xx();
        assert_eq!(b.state(), CircuitState::Open);

        // Wait for cooldown (50ms).
        thread::sleep(Duration::from_millis(80));

        assert_eq!(b.state(), CircuitState::HalfOpen);
        assert!(b.allow_request(), "half-open must allow one probe");
    }

    #[test]
    fn half_open_success_closes_circuit() {
        let b = BreakerState::new(fast_config());
        // Open.
        b.record_5xx();
        b.record_5xx();
        b.record_5xx();
        // Wait for HalfOpen.
        thread::sleep(Duration::from_millis(80));
        assert_eq!(b.state(), CircuitState::HalfOpen);
        // Probe succeeds.
        b.record_success();
        assert_eq!(b.state(), CircuitState::Closed);
    }

    #[test]
    fn half_open_failure_reopens_with_doubled_cooldown() {
        let b = BreakerState::new(fast_config());
        b.record_5xx();
        b.record_5xx();
        b.record_5xx();
        thread::sleep(Duration::from_millis(80));
        assert_eq!(b.state(), CircuitState::HalfOpen);

        // Probe fails → back to Open.
        b.record_5xx();
        assert_eq!(b.state(), CircuitState::Open);

        // The doubled cooldown (100ms) must not have elapsed yet.
        assert!(!b.allow_request());
    }

    // ── thread safety ──────────────────────────────────────────────────────────

    /// Concurrent failure recording must not panic or corrupt state.
    #[test]
    fn concurrent_record_failure_is_safe() {
        let b = Arc::new(BreakerState::new(BreakerConfig {
            consecutive_5xx_threshold: 1000,
            ..fast_config()
        }));
        let mut handles = Vec::new();
        for _ in 0..8 {
            let b2 = Arc::clone(&b);
            handles.push(thread::spawn(move || {
                for _ in 0..50 {
                    b2.record_5xx();
                    b2.record_connect_fail();
                    let _ = b2.state();
                }
            }));
        }
        for h in handles {
            h.join().expect("thread panicked");
        }
        // No panic = pass.
    }

    // ── manager ───────────────────────────────────────────────────────────────

    #[test]
    fn manager_unknown_cluster_allows_request() {
        let mgr = CircuitBreakerManager::new();
        assert!(mgr.allow_request("does-not-exist"));
    }

    #[test]
    fn manager_blocks_open_circuit() {
        let mgr = CircuitBreakerManager::new();
        mgr.register("api", fast_config());
        let b = mgr.get("api").unwrap();
        b.record_5xx();
        b.record_5xx();
        b.record_5xx();
        assert!(!mgr.allow_request("api"));
    }

    #[test]
    fn manager_retry_after_is_positive() {
        let mgr = CircuitBreakerManager::new();
        mgr.register("api", fast_config());
        let b = mgr.get("api").unwrap();
        b.record_5xx();
        b.record_5xx();
        b.record_5xx();
        assert!(mgr.retry_after_secs("api") >= 1);
    }

    #[test]
    fn manager_get_or_default_creates_entry() {
        let mgr = CircuitBreakerManager::new();
        let b = mgr.get_or_default("new-cluster");
        assert_eq!(b.state(), CircuitState::Closed);
        assert!(mgr.allow_request("new-cluster"));
    }

    // ── error rate threshold ──────────────────────────────────────────────────

    #[test]
    fn error_rate_threshold_opens_circuit() {
        let config = BreakerConfig {
            consecutive_5xx_threshold: 1000, // disable count-based trigger
            consecutive_connect_fail_threshold: 1000,
            error_rate_threshold: Some(0.50),
            window_size: 10,
            initial_cooldown: Duration::from_millis(50),
            max_cooldown: Duration::from_secs(300),
        };
        let b = BreakerState::new(config);

        // Fill the window with 6 successes + 4 failures = 40% → no trip.
        for _ in 0..6 {
            b.record_success();
        }
        for _ in 0..4 {
            b.record_5xx();
        }
        assert_eq!(b.state(), CircuitState::Closed, "40% error rate must not trip");

        // Slide 1 more failure into the window (5/10 = 50% → trip).
        b.record_5xx();
        assert_eq!(b.state(), CircuitState::Open, "50% error rate must trip");
    }

    // ── cooldown remaining ────────────────────────────────────────────────────

    #[test]
    fn cooldown_remaining_zero_when_closed() {
        let b = BreakerState::new(fast_config());
        assert_eq!(b.cooldown_remaining(), Duration::ZERO);
    }

    #[test]
    fn cooldown_remaining_positive_when_open() {
        let b = BreakerState::new(fast_config());
        b.record_5xx();
        b.record_5xx();
        b.record_5xx();
        assert!(b.cooldown_remaining() > Duration::ZERO);
    }

    // ── EWMA adaptive mode ────────────────────────────────────────────────────

    fn adaptive_config() -> (BreakerConfig, EwmaBreakerConfig) {
        let base = BreakerConfig {
            consecutive_5xx_threshold: 1000, // disable static count trigger
            consecutive_connect_fail_threshold: 1000,
            error_rate_threshold: None,      // disable static rate trigger
            window_size: 20,
            initial_cooldown: Duration::from_millis(50),
            max_cooldown: Duration::from_secs(300),
        };
        let ewma = EwmaBreakerConfig {
            window_secs: 60,
            alpha: 0.1,
            trip_multiplier: 3.0,
            min_error_rate_floor: 0.02,   // 2% floor
            max_error_rate_ceiling: 0.50, // 50% ceiling
        };
        (base, ewma)
    }

    /// Feed N samples into a breaker and return it.
    fn feed(b: &BreakerState, successes: usize, failures: usize) {
        for _ in 0..successes {
            b.record_success();
        }
        for _ in 0..failures {
            b.record_5xx();
        }
    }

    /// EWMA converges to the expected value after 100 pure-failure samples.
    ///
    /// With alpha=0.1 and 100 samples of 1.0, the EWMA after k steps is:
    ///   ewma_k = 1 - (1 - alpha)^k
    /// For k=100, alpha=0.1: ewma ≈ 1 - 0.9^100 ≈ 0.99997
    #[test]
    fn ewma_converges_on_pure_failure_stream() {
        let (base, ewma_cfg) = adaptive_config();
        let b = BreakerState::new_with_mode(base, BreakerMode::Adaptive(ewma_cfg));

        // Feed 100 failure samples.
        for _ in 0..100 {
            b.update_ewma(true);
        }
        let baseline = b.ewma_baseline();
        assert!(
            baseline > 0.99,
            "expected EWMA > 0.99 after 100 failure samples, got {baseline}"
        );
    }

    /// High stable baseline (10%) does not prevent tripping on a 50% spike
    /// that exceeds the ceiling threshold.
    #[test]
    fn high_baseline_does_not_block_ceiling_trip() {
        let (base, ewma_cfg) = adaptive_config();
        let b = BreakerState::new_with_mode(base, BreakerMode::Adaptive(ewma_cfg));

        // Establish a ~10% baseline (window_size=20: 18 successes + 2 failures).
        feed(&b, 18, 2);
        assert_eq!(b.state(), CircuitState::Closed, "10% rate must not trip yet");

        // Now burst to 50% in the sliding window (fill with 10 failures).
        for _ in 0..10 {
            b.record_5xx();
        }
        // Window now has ~10 failures out of 20 = 50% → must trip on ceiling.
        assert_eq!(
            b.state(),
            CircuitState::Open,
            "50% burst must trip on max_error_rate_ceiling=0.50"
        );
    }

    /// Low baseline (0%) — a spike above `min_error_rate_floor` triggers trip
    /// once the ceiling condition kicks in.
    ///
    /// With alpha=0.1, each failure sample raises the EWMA baseline quickly
    /// (e.g., 1 failure → baseline=0.1 → adaptive_threshold=0.3).  The
    /// adaptive spike condition therefore does not trip on a small number of
    /// failures.  However, when the error rate exceeds `max_error_rate_ceiling`
    /// the circuit opens unconditionally.
    ///
    /// To test the FLOOR condition in isolation we use a very small alpha
    /// (0.001) so the EWMA adapts slowly and the spike clearly exceeds the
    /// adaptive threshold derived from a near-zero baseline.
    #[test]
    fn zero_baseline_spike_above_floor_trips() {
        let (base, mut ewma_cfg) = adaptive_config();
        // Tiny alpha: EWMA adapts very slowly.  After even 20 failures,
        // baseline ≈ 1 - (1-0.001)^20 ≈ 0.020 → adaptive_threshold ≈ max(0.06, 0.02) = 0.06.
        // A window with 10/20 = 50% errors clearly exceeds 0.06 AND exceeds the floor.
        ewma_cfg.alpha = 0.001;
        ewma_cfg.max_error_rate_ceiling = 0.80; // raise ceiling so we test adaptive path
        let b = BreakerState::new_with_mode(base, BreakerMode::Adaptive(ewma_cfg));

        // Pre-fill window with 10 successes (baseline stays ~0).
        feed(&b, 10, 0);
        assert_eq!(b.state(), CircuitState::Closed);

        // Inject 10 failures: window = 10T + 10F → current_rate = 50%.
        // After each failure, EWMA ≈ 0.001 * k (tiny), adaptive_threshold ≈ 0.02 (floor).
        // 50% >> 2% → trip on adaptive condition.
        for _ in 0..10 {
            b.record_5xx();
        }
        assert_eq!(
            b.state(),
            CircuitState::Open,
            "50% spike from near-zero baseline must trip on adaptive floor (2%)"
        );
    }

    /// Ceiling bypasses the EWMA: feeding 50%+ error rate immediately trips
    /// regardless of the baseline.
    #[test]
    fn ceiling_bypasses_ewma_baseline() {
        let (base, mut ewma_cfg) = adaptive_config();
        // Raise multiplier very high so only the ceiling would catch this.
        ewma_cfg.trip_multiplier = 100.0;
        ewma_cfg.max_error_rate_ceiling = 0.40; // 40% ceiling
        let b = BreakerState::new_with_mode(base, BreakerMode::Adaptive(ewma_cfg));

        // Establish a 50% EWMA baseline (so trip_multiplier * baseline = 5000%
        // which would never trip naturally).
        for _ in 0..100 {
            b.update_ewma(true); // drive baseline toward 1.0
        }
        // Reset state manually by re-registering is not possible; instead
        // just verify the ceiling condition independently.  Drive the window
        // to 45% (9 failures in 20) which exceeds the 40% ceiling.
        feed(&b, 11, 9);

        assert_eq!(
            b.state(),
            CircuitState::Open,
            "45% current rate must trip on 40% ceiling regardless of baseline"
        );
    }

    /// Static mode is unaffected by EWMA logic — existing tests continue to pass.
    #[test]
    fn static_mode_unaffected_by_ewma_fields() {
        let b = BreakerState::new(fast_config());
        // No EWMA update should occur in static mode.
        b.record_5xx();
        b.record_success();
        assert_eq!(
            b.ewma_baseline(),
            0.0,
            "static mode must not update EWMA"
        );
    }

    /// Manager `register_adaptive` creates a breaker in adaptive mode.
    #[test]
    fn manager_register_adaptive_creates_adaptive_breaker() {
        let (base, ewma_cfg) = adaptive_config();
        let mgr = CircuitBreakerManager::new();
        mgr.register_adaptive("svc-adaptive", base, ewma_cfg);

        let b = mgr.get("svc-adaptive").unwrap();
        assert!(
            matches!(b.mode, BreakerMode::Adaptive(_)),
            "expected adaptive mode"
        );
        assert_eq!(b.state(), CircuitState::Closed);
    }
}

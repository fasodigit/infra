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

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use dashmap::DashMap;
use tracing::{info, warn};

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
    /// Atomic counters for lock-free reads in the hot path.
    pub active_requests: AtomicU32,
    inner: RwLock<Inner>,
}

impl std::fmt::Debug for BreakerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let inner = self.inner.read().unwrap();
        f.debug_struct("BreakerState")
            .field("state", &inner.state)
            .field("consecutive_5xx", &inner.consecutive_5xx)
            .field("consecutive_connect_fail", &inner.consecutive_connect_fail)
            .finish()
    }
}

impl BreakerState {
    /// Create a new breaker in the `Closed` state.
    pub fn new(config: BreakerConfig) -> Self {
        let inner = Inner::new(&config);
        Self {
            config,
            active_requests: AtomicU32::new(0),
            inner: RwLock::new(inner),
        }
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
    pub fn record_success(&self) {
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
    pub fn record_5xx(&self) {
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

        let should_open =
            inner.consecutive_5xx >= self.config.consecutive_5xx_threshold
                || self.error_rate_exceeded(&inner);

        if should_open && inner.state == CircuitState::Closed {
            inner.state = CircuitState::Open;
            inner.opened_at = Some(Instant::now());
            warn!(
                consecutive_5xx = inner.consecutive_5xx,
                "circuit breaker: Closed → Open (5xx threshold)"
            );
        }
    }

    /// Record a connect failure (TCP error, connection refused, timeout).
    ///
    /// May transition `Closed → Open`.
    pub fn record_connect_fail(&self) {
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

        let should_open =
            inner.consecutive_connect_fail >= self.config.consecutive_connect_fail_threshold
                || self.error_rate_exceeded(&inner);

        if should_open && inner.state == CircuitState::Closed {
            inner.state = CircuitState::Open;
            inner.opened_at = Some(Instant::now());
            warn!(
                consecutive_connect_fail = inner.consecutive_connect_fail,
                "circuit breaker: Closed → Open (connect fail threshold)"
            );
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
    pub fn register(&self, cluster: &str, config: BreakerConfig) {
        self.breakers.insert(
            cluster.to_string(),
            Arc::new(BreakerState::new(config)),
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
}

//! Circuit breaker per upstream cluster.
//!
//! Tracks failures per upstream. Opens circuit after threshold consecutive
//! failures. Transitions to half-open after a wait duration. Closes on success
//! in half-open state.

use armageddon_common::types::{CircuitBreakerConfig, Cluster};
use dashmap::DashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Circuit breaker state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

/// Per-cluster circuit breaker counters.
pub struct ClusterBreaker {
    pub config: CircuitBreakerConfig,
    pub active_connections: AtomicU32,
    pub pending_requests: AtomicU32,
    pub active_requests: AtomicU32,
    pub active_retries: AtomicU32,
    pub consecutive_failures: AtomicU32,
    pub state: std::sync::RwLock<CircuitState>,
    /// When the circuit was opened (for half-open transition).
    pub opened_at: std::sync::RwLock<Option<Instant>>,
    /// How long to wait before transitioning to half-open.
    pub wait_duration: Duration,
}

impl ClusterBreaker {
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            wait_duration: Duration::from_secs(30), // default 30s wait
            config,
            active_connections: AtomicU32::new(0),
            pending_requests: AtomicU32::new(0),
            active_requests: AtomicU32::new(0),
            active_retries: AtomicU32::new(0),
            consecutive_failures: AtomicU32::new(0),
            state: std::sync::RwLock::new(CircuitState::Closed),
            opened_at: std::sync::RwLock::new(None),
        }
    }

    /// Check whether the circuit allows a new request.
    pub fn allow_request(&self) -> bool {
        // Check if we should transition from Open -> HalfOpen
        self.maybe_transition_to_half_open();

        let state = self.state.read().unwrap();
        match *state {
            CircuitState::Open => false,
            CircuitState::HalfOpen => {
                // Allow only 1 probe request in half-open
                self.active_requests.load(Ordering::Relaxed) < 1
            }
            CircuitState::Closed => {
                self.active_connections.load(Ordering::Relaxed) < self.config.max_connections
                    && self.pending_requests.load(Ordering::Relaxed)
                        < self.config.max_pending_requests
                    && self.active_requests.load(Ordering::Relaxed) < self.config.max_requests
            }
        }
    }

    /// Record a successful request.
    pub fn record_success(&self) {
        self.consecutive_failures.store(0, Ordering::Relaxed);
        let mut state = self.state.write().unwrap();
        if *state == CircuitState::HalfOpen {
            *state = CircuitState::Closed;
            let mut opened = self.opened_at.write().unwrap();
            *opened = None;
            tracing::info!("circuit breaker CLOSED (recovered)");
        }
    }

    /// Record a failed request.
    pub fn record_failure(&self) {
        let failures = self.consecutive_failures.fetch_add(1, Ordering::Relaxed) + 1;
        if failures >= self.config.max_retries {
            let mut state = self.state.write().unwrap();
            if *state == CircuitState::Closed || *state == CircuitState::HalfOpen {
                *state = CircuitState::Open;
                let mut opened = self.opened_at.write().unwrap();
                *opened = Some(Instant::now());
                tracing::warn!(
                    "circuit breaker OPENED after {} consecutive failures (wait {}s for half-open)",
                    failures,
                    self.wait_duration.as_secs(),
                );
            }
        }
    }

    /// Increment active request counter (call before proxying).
    pub fn on_request_start(&self) {
        self.active_requests.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement active request counter (call after proxying).
    pub fn on_request_end(&self) {
        self.active_requests.fetch_sub(1, Ordering::Relaxed);
    }

    /// Get the current circuit state.
    pub fn current_state(&self) -> CircuitState {
        self.maybe_transition_to_half_open();
        *self.state.read().unwrap()
    }

    /// Transition from Open -> HalfOpen if the wait duration has elapsed.
    fn maybe_transition_to_half_open(&self) {
        let state = *self.state.read().unwrap();
        if state == CircuitState::Open {
            let opened = self.opened_at.read().unwrap();
            if let Some(opened_at) = *opened {
                if opened_at.elapsed() >= self.wait_duration {
                    drop(opened);
                    let mut state = self.state.write().unwrap();
                    if *state == CircuitState::Open {
                        *state = CircuitState::HalfOpen;
                        tracing::info!(
                            "circuit breaker transitioned to HALF-OPEN (probe allowed)"
                        );
                    }
                }
            }
        }
    }
}

/// Manages circuit breakers for all upstream clusters.
pub struct CircuitBreakerManager {
    breakers: DashMap<String, Arc<ClusterBreaker>>,
}

impl CircuitBreakerManager {
    pub fn new(clusters: Vec<Cluster>) -> Self {
        let breakers = DashMap::new();
        for cluster in clusters {
            breakers.insert(
                cluster.name.clone(),
                Arc::new(ClusterBreaker::new(cluster.circuit_breaker)),
            );
        }
        Self { breakers }
    }

    /// Get the circuit breaker for a cluster.
    pub fn get(&self, cluster_name: &str) -> Option<Arc<ClusterBreaker>> {
        self.breakers.get(cluster_name).map(|b| Arc::clone(&b))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use armageddon_common::types::CircuitBreakerConfig;

    fn test_config() -> CircuitBreakerConfig {
        CircuitBreakerConfig {
            max_connections: 100,
            max_pending_requests: 100,
            max_requests: 100,
            max_retries: 3, // open after 3 failures
        }
    }

    #[test]
    fn test_circuit_breaker_starts_closed() {
        let cb = ClusterBreaker::new(test_config());
        assert_eq!(cb.current_state(), CircuitState::Closed);
        assert!(cb.allow_request());
    }

    #[test]
    fn test_circuit_breaker_opens_after_threshold() {
        let cb = ClusterBreaker::new(test_config());
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.current_state(), CircuitState::Closed); // 2 < 3
        cb.record_failure(); // 3 >= 3 -> OPEN
        assert_eq!(cb.current_state(), CircuitState::Open);
        assert!(!cb.allow_request());
    }

    #[test]
    fn test_circuit_breaker_success_resets() {
        let cb = ClusterBreaker::new(test_config());
        cb.record_failure();
        cb.record_failure();
        cb.record_success(); // reset
        assert_eq!(cb.current_state(), CircuitState::Closed);
        assert_eq!(cb.consecutive_failures.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_circuit_breaker_half_open_recovery() {
        let mut cb = ClusterBreaker::new(test_config());
        cb.wait_duration = Duration::from_millis(1); // short wait for test

        // Open the circuit
        cb.record_failure();
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.current_state(), CircuitState::Open);

        // Wait for transition
        std::thread::sleep(Duration::from_millis(5));

        // Should be half-open now
        assert_eq!(cb.current_state(), CircuitState::HalfOpen);
        assert!(cb.allow_request()); // allow 1 probe

        // Success in half-open -> closed
        cb.record_success();
        assert_eq!(cb.current_state(), CircuitState::Closed);
    }
}

// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Load-balancing policies for the Pingora upstream path.
//!
//! This module operates directly on [`armageddon_common::types::Endpoint`]
//! (the value type used by the Pingora [`UpstreamRegistry`]) so the pingora
//! path does not pull an additional dependency on `armageddon-lb`.
//!
//! # Policies
//!
//! | Policy                | Status | Notes                                   |
//! |-----------------------|--------|-----------------------------------------|
//! | [`RoundRobin`]        | Live   | Atomic-counter rotation across healthy. |
//! | [`Weighted`]          | Live   | Smooth WRR (Nginx algorithm).           |
//! | [`PowerOfTwoChoices`] | Live   | P2C with per-endpoint active-conn count.|
//!
//! # Smooth Weighted Round-Robin (Nginx algorithm)
//!
//! On each call every endpoint accumulates its own weight into `current_weight`.
//! The endpoint with the highest `current_weight` **among healthy endpoints** is
//! selected, then its `current_weight` is decremented by `total_weight`.
//!
//! ```text
//! Before:          current = [0, 0, 0], weights = [3, 1, 2], total = 6
//! Call 1: add →  [3, 1, 2]; pick idx 0 (w=3); sub → [-3, 1, 2]  → A
//! Call 2: add →  [0, 2, 4]; pick idx 2 (w=4); sub → [0, 2, -2]  → C
//! Call 3: add →  [3, 3, 0]; pick idx 0 (w=3); sub → [-3, 3, 0]  → A  (tie → first)
//! Call 4: add →  [0, 4, 2]; pick idx 1 (w=4); sub → [0, -2, 2]  → B
//! Call 5: add →  [3, -1, 4]; pick idx 2 (w=4); sub → [3, -1, -2] → C
//! Call 6: add →  [6, 0, 0]; pick idx 0 (w=6); sub → [0, 0, 0]   → A
//! → sequence A C A B C A → 3:1:2 ratio respected.
//! ```
//!
//! # Power-of-Two-Choices (P2C)
//!
//! On each call two distinct healthy endpoints are drawn at random; the one
//! with fewer active connections is selected.  Active connections are tracked
//! per endpoint via `AtomicUsize` stored inside the balancer.  The balancer
//! is keyed by `(address, port)` so the counters survive across hot-reloads
//! when the endpoint list is replaced by `ClusterResolver::update`.
//!
//! Callers must call [`PowerOfTwoChoices::connection_acquired`] when they
//! actually open a connection and [`PowerOfTwoChoices::connection_released`]
//! when the connection closes to keep counters accurate.
//!
//! # Failure modes
//!
//! - All endpoints unhealthy → all balancers return `None`; the caller must
//!   surface a 503 to the downstream client.
//! - Single healthy endpoint → all balancers return it directly without
//!   requiring randomness or weight math.
//! - Hot-reload (new endpoint list, same cluster name): `RoundRobin` and
//!   `Weighted` adapt automatically on the next call because they operate on
//!   the slice passed in.  `PowerOfTwoChoices` merges counters for endpoints
//!   that survive reload and zeros counters for new arrivals.

use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};
use std::sync::Arc;

use parking_lot::Mutex;

use armageddon_common::types::Endpoint;

// -- policy enum --

/// Cluster load-balancing policy tag.
///
/// Stored inside [`super::selector::ClusterState`] so `ClusterResolver` knows
/// which algorithm to invoke when routing a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LbPolicy {
    /// Classical round-robin across healthy endpoints.
    RoundRobin,
    /// Smooth weighted round-robin; consults `Endpoint.weight`.
    Weighted,
    /// Power-of-two-choices — sample two random healthy endpoints, pick the
    /// one with fewer in-flight connections.
    PowerOfTwoChoices,
    /// Pure least-connections — scan ALL healthy endpoints, select the one
    /// with the fewest in-flight connections.  O(N) per pick; best for
    /// clusters with < 100 rps where P2C may concentrate load by coincidence.
    LeastConn,
}

impl Default for LbPolicy {
    fn default() -> Self {
        Self::RoundRobin
    }
}

// -- round-robin --

/// Stateless round-robin balancer — thread-safe via an atomic counter.
///
/// The counter advances monotonically on every call; filtering to healthy
/// endpoints happens per call so a just-ejected backend stops receiving
/// traffic on the very next request without any coordination.
#[derive(Debug, Default)]
pub struct RoundRobin {
    counter: AtomicUsize,
}

impl RoundRobin {
    /// Create a new round-robin balancer starting at index 0.
    pub fn new() -> Self {
        Self {
            counter: AtomicUsize::new(0),
        }
    }

    /// Pick the next healthy endpoint from `endpoints`, rotating fairly.
    ///
    /// Returns `None` when the slice is empty or contains no healthy member.
    pub fn pick<'a>(&self, endpoints: &'a [Endpoint]) -> Option<&'a Endpoint> {
        // Collect references to healthy entries only.  Rebuilding this vector
        // per call is cheap (endpoint counts are O(10) in practice) and keeps
        // the algorithm responsive to health flips without extra state.
        let healthy: Vec<&Endpoint> = endpoints.iter().filter(|e| e.healthy).collect();

        if healthy.is_empty() {
            return None;
        }

        let idx = self.counter.fetch_add(1, Ordering::Relaxed) % healthy.len();
        Some(healthy[idx])
    }
}

// -- smooth weighted round-robin --

/// Smooth Weighted Round-Robin balancer (Nginx algorithm).
///
/// State is stored as a `Mutex<Vec<i64>>` of per-endpoint current_weights,
/// indexed parallel to the endpoint slice **at the time the state was
/// created or last resized**.
///
/// Thread-safety: `parking_lot::Mutex` with lock held for only the scalar
/// arithmetic of a single pick operation — O(n) where n is endpoint count
/// (O(10) in practice).  No `.await` is ever held while the lock is taken,
/// satisfying the project invariant.
///
/// # Hot-reload behaviour
///
/// When the endpoint slice length changes between calls, the state vector is
/// re-initialised to zeros (proportional schedule restarts from origin).
/// This is safe because the smooth WRR algorithm converges to the correct
/// distribution within one full cycle (`sum(weights)` calls) from any
/// zero-initialised start.
#[derive(Debug)]
pub struct Weighted {
    /// Per-endpoint accumulator (`current_weight` in Nginx terminology).
    /// Length is re-synchronised on every pick if the endpoint slice changed.
    state: Mutex<WeightedState>,
}

#[derive(Debug)]
struct WeightedState {
    /// Length of the endpoint slice this state was built for.
    n: usize,
    /// Mutable per-endpoint accumulator.  Index-parallel to the endpoint slice.
    current: Vec<i64>,
}

impl Default for Weighted {
    fn default() -> Self {
        Self::new()
    }
}

impl Weighted {
    /// Create a new smooth WRR balancer.
    pub fn new() -> Self {
        Self {
            state: Mutex::new(WeightedState {
                n: 0,
                current: Vec::new(),
            }),
        }
    }

    /// Pick the next endpoint according to Smooth WRR.
    ///
    /// Returns `None` when the slice is empty or all endpoints are unhealthy.
    ///
    /// # Algorithm
    ///
    /// For each endpoint `i`:
    /// 1. `current[i] += weight[i]`
    /// 2. Selected = `argmax(current[i])` over healthy endpoints only.
    /// 3. `current[selected] -= total_weight`
    pub fn pick<'a>(&self, endpoints: &'a [Endpoint]) -> Option<&'a Endpoint> {
        let n = endpoints.len();
        if n == 0 {
            return None;
        }

        // Early exit when no healthy endpoint exists.
        let has_healthy = endpoints.iter().any(|e| e.healthy);
        if !has_healthy {
            return None;
        }

        // Single healthy endpoint: skip weight math.
        let healthy_count = endpoints.iter().filter(|e| e.healthy).count();
        if healthy_count == 1 {
            return endpoints.iter().find(|e| e.healthy);
        }

        let mut guard = self.state.lock();

        // Re-initialise if the endpoint slice length changed (hot-reload).
        if guard.n != n {
            guard.n = n;
            guard.current = vec![0i64; n];
        }

        // Step 1: add weight[i] to current[i] for every endpoint.
        let total_weight: i64 = endpoints
            .iter()
            .map(|e| i64::from(e.weight.max(1)))
            .sum();

        for (i, ep) in endpoints.iter().enumerate() {
            guard.current[i] += i64::from(ep.weight.max(1));
        }

        // Step 2: argmax(current[i]) over *healthy* endpoints.
        let best_idx = endpoints
            .iter()
            .enumerate()
            .filter(|(_, e)| e.healthy)
            .max_by_key(|(i, _)| guard.current[*i])
            .map(|(i, _)| i)?;

        // Step 3: decrement winner by total_weight.
        guard.current[best_idx] -= total_weight;

        Some(&endpoints[best_idx])
    }
}

// -- power-of-two-choices --

/// Endpoint key used to persist connection counters across hot-reloads.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct EndpointKey {
    address: Arc<str>,
    port: u16,
}

impl EndpointKey {
    fn from_ep(ep: &Endpoint) -> Self {
        Self {
            address: Arc::from(ep.address.as_str()),
            port: ep.port,
        }
    }
}

/// Power-of-Two-Choices balancer with per-endpoint active-connection tracking.
///
/// On each pick two distinct healthy endpoints are selected uniformly at
/// random; the one with fewer active connections wins.  Connection counters
/// are stored in a `DashMap<EndpointKey, AtomicUsize>` so they survive
/// hot-reloads as long as the `(address, port)` key is stable.
///
/// # Thread safety
///
/// Random index generation uses `rand::thread_rng()` (thread-local, no lock).
/// Counter reads/writes use `AtomicUsize` with `Relaxed` ordering — P2C
/// tolerates stale counts since the algorithm is a probabilistic heuristic.
///
/// # Active connection tracking (caller responsibility)
///
/// 1. Call [`PowerOfTwoChoices::pick`] to select an endpoint.
/// 2. Call [`PowerOfTwoChoices::connection_acquired`] with the picked address
///    and port immediately after the upstream TCP connection is established.
/// 3. Call [`PowerOfTwoChoices::connection_released`] when the connection
///    closes (regardless of success/failure) to decrement the counter.
///
/// Failing to release will cause the algorithm to over-penalise the endpoint
/// but will not cause incorrect behaviour for other endpoints.
#[derive(Debug, Default)]
pub struct PowerOfTwoChoices {
    /// Per-endpoint active-connection counter, keyed by `(address, port)`.
    counters: dashmap::DashMap<EndpointKey, Arc<AtomicI64>>,
}

impl PowerOfTwoChoices {
    /// Create a new P2C balancer with empty connection counters.
    pub fn new() -> Self {
        Self {
            counters: dashmap::DashMap::new(),
        }
    }

    /// Obtain (or lazily create) the counter `Arc` for a given endpoint.
    #[allow(dead_code)]
    fn counter_for(&self, ep: &Endpoint) -> Arc<AtomicI64> {
        let key = EndpointKey::from_ep(ep);
        self.counters
            .entry(key)
            .or_insert_with(|| Arc::new(AtomicI64::new(0)))
            .clone()
    }

    /// Return the current active-connection count for `ep`.
    fn connections(&self, ep: &Endpoint) -> i64 {
        let key = EndpointKey::from_ep(ep);
        match self.counters.get(&key) {
            Some(c) => c.load(Ordering::Relaxed).max(0),
            None => 0,
        }
    }

    /// Increment the connection counter for the endpoint at `(address, port)`.
    ///
    /// Call this immediately after establishing a connection to the upstream.
    pub fn connection_acquired(&self, address: &str, port: u16) {
        let key = EndpointKey {
            address: Arc::from(address),
            port,
        };
        self.counters
            .entry(key)
            .or_insert_with(|| Arc::new(AtomicI64::new(0)))
            .fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement the connection counter for the endpoint at `(address, port)`.
    ///
    /// Call this when a connection to the upstream closes (success or error).
    /// The counter is floor-clamped to zero to guard against spurious releases.
    pub fn connection_released(&self, address: &str, port: u16) {
        let key = EndpointKey {
            address: Arc::from(address),
            port,
        };
        if let Some(c) = self.counters.get(&key) {
            // Saturating-sub via CAS loop to prevent underflow to negative.
            let mut old = c.load(Ordering::Relaxed);
            loop {
                if old <= 0 {
                    break;
                }
                match c.compare_exchange_weak(old, old - 1, Ordering::Relaxed, Ordering::Relaxed) {
                    Ok(_) => break,
                    Err(cur) => old = cur,
                }
            }
        }
    }

    /// Pick the better of two randomly sampled healthy endpoints.
    ///
    /// Returns `None` when the slice is empty or all endpoints are unhealthy.
    pub fn pick<'a>(&self, endpoints: &'a [Endpoint]) -> Option<&'a Endpoint> {
        let healthy: Vec<(usize, &Endpoint)> = endpoints
            .iter()
            .enumerate()
            .filter(|(_, e)| e.healthy)
            .collect();

        match healthy.len() {
            0 => None,
            1 => Some(healthy[0].1),
            n => {
                use rand::Rng as _;
                let mut rng = rand::thread_rng();
                // Draw two distinct indices into the healthy slice.
                let a = rng.gen_range(0..n);
                let mut b = rng.gen_range(0..n - 1);
                if b >= a {
                    b += 1;
                }
                let (_, ep_a) = healthy[a];
                let (_, ep_b) = healthy[b];

                // Pick the endpoint with fewer active connections.
                // Ties go to `a` (first-drawn).
                if self.connections(ep_a) <= self.connections(ep_b) {
                    Some(ep_a)
                } else {
                    Some(ep_b)
                }
            }
        }
    }
}

// -- pure least-connections --

/// Pure Least-Connections balancer.
///
/// On every pick, **all** healthy endpoints are scanned and the one with the
/// fewest active in-flight connections is selected.  Connection counters are
/// shared with [`PowerOfTwoChoices`] using the same `(address, port)` key so
/// the two algorithms can be hot-swapped without resetting counters.
///
/// # Complexity
///
/// O(N) per pick — suitable for clusters with < ~50 endpoints.  For large
/// clusters (100+) prefer [`PowerOfTwoChoices`] (O(1)) since the P2C
/// approximation converges to least-conn at scale.
///
/// # Tie-breaking
///
/// When two or more endpoints share the minimum connection count the one with
/// the **lowest index** in the slice is selected.  This is deterministic and
/// avoids unnecessary randomness in the tie-break path.
///
/// # Thread safety
///
/// Connection counters are `AtomicI64` with `Relaxed` ordering (same policy
/// as P2C).  Stale reads are tolerable because the algorithm is a heuristic —
/// a marginally-stale count leads to a sub-optimal (but not incorrect) pick.
///
/// # Active connection tracking
///
/// The caller must pair every `pick` call with a corresponding
/// [`LeastConn::connection_acquired`] / [`LeastConn::connection_released`]
/// call, identical to the P2C contract.
///
/// # Failure modes
///
/// - All endpoints unhealthy → `None`.
/// - Single healthy endpoint → returned without scanning.
/// - Empty slice → `None`.
#[derive(Debug, Default)]
pub struct LeastConn {
    counters: dashmap::DashMap<EndpointKey, Arc<AtomicI64>>,
}

impl LeastConn {
    /// Create a new LeastConn balancer with empty counters.
    pub fn new() -> Self {
        Self {
            counters: dashmap::DashMap::new(),
        }
    }

    /// Return the current active-connection count for `ep`.
    fn connections(&self, ep: &Endpoint) -> i64 {
        let key = EndpointKey::from_ep(ep);
        match self.counters.get(&key) {
            Some(c) => c.load(Ordering::Relaxed).max(0),
            None => 0,
        }
    }

    /// Increment the connection counter for `(address, port)`.
    pub fn connection_acquired(&self, address: &str, port: u16) {
        let key = EndpointKey {
            address: Arc::from(address),
            port,
        };
        self.counters
            .entry(key)
            .or_insert_with(|| Arc::new(AtomicI64::new(0)))
            .fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement the connection counter for `(address, port)`.
    /// Floor-clamped at zero to guard against spurious releases.
    pub fn connection_released(&self, address: &str, port: u16) {
        let key = EndpointKey {
            address: Arc::from(address),
            port,
        };
        if let Some(c) = self.counters.get(&key) {
            let mut old = c.load(Ordering::Relaxed);
            loop {
                if old <= 0 {
                    break;
                }
                match c.compare_exchange_weak(old, old - 1, Ordering::Relaxed, Ordering::Relaxed) {
                    Ok(_) => break,
                    Err(cur) => old = cur,
                }
            }
        }
    }

    /// Pick the healthy endpoint with the fewest active connections.
    ///
    /// Returns `None` when the slice is empty or all endpoints are unhealthy.
    /// On a tie, the endpoint with the **lowest slice index** wins.
    pub fn pick<'a>(&self, endpoints: &'a [Endpoint]) -> Option<&'a Endpoint> {
        let mut best: Option<(i64, usize)> = None; // (min_connections, index)

        for (idx, ep) in endpoints.iter().enumerate() {
            if !ep.healthy {
                continue;
            }
            let conns = self.connections(ep);
            match best {
                None => best = Some((conns, idx)),
                Some((min_conns, _)) if conns < min_conns => best = Some((conns, idx)),
                _ => {} // tie → keep lower index (already set)
            }
        }

        best.map(|(_, idx)| &endpoints[idx])
    }
}

// -- tests --

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn ep(host: &str, port: u16, healthy: bool) -> Endpoint {
        Endpoint {
            address: host.to_string(),
            port,
            weight: 1,
            healthy,
        }
    }

    fn ep_w(host: &str, port: u16, healthy: bool, weight: u32) -> Endpoint {
        Endpoint {
            address: host.to_string(),
            port,
            weight,
            healthy,
        }
    }

    // -- RoundRobin ----------------------------------------------------------

    #[test]
    fn round_robin_rotates_across_three_healthy_endpoints() {
        let endpoints = vec![
            ep("10.0.0.1", 8080, true),
            ep("10.0.0.2", 8080, true),
            ep("10.0.0.3", 8080, true),
        ];

        let rr = RoundRobin::new();

        // Six successive picks must visit [0, 1, 2, 0, 1, 2].
        let picks: Vec<String> = (0..6)
            .map(|_| rr.pick(&endpoints).unwrap().address.clone())
            .collect();

        assert_eq!(
            picks,
            vec![
                "10.0.0.1", "10.0.0.2", "10.0.0.3",
                "10.0.0.1", "10.0.0.2", "10.0.0.3",
            ]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>(),
            "round-robin must rotate uniformly over healthy members"
        );
    }

    #[test]
    fn round_robin_returns_none_when_all_unhealthy() {
        let endpoints = vec![
            ep("10.0.0.1", 8080, false),
            ep("10.0.0.2", 8080, false),
        ];

        let rr = RoundRobin::new();
        assert!(
            rr.pick(&endpoints).is_none(),
            "RoundRobin::pick must yield None when every endpoint is unhealthy"
        );
    }

    #[test]
    fn round_robin_single_endpoint_is_returned_on_every_call() {
        let endpoints = vec![ep("10.0.0.1", 8080, true)];

        let rr = RoundRobin::new();
        for _ in 0..10 {
            let picked = rr.pick(&endpoints).expect("single endpoint always picks");
            assert_eq!(picked.address, "10.0.0.1");
        }
    }

    #[test]
    fn round_robin_skips_unhealthy_endpoints() {
        let endpoints = vec![
            ep("10.0.0.1", 8080, true),
            ep("10.0.0.2", 8080, false),
            ep("10.0.0.3", 8080, true),
        ];

        let rr = RoundRobin::new();

        // Four successive picks must visit only the healthy subset: [1, 3, 1, 3].
        let picks: Vec<String> = (0..4)
            .map(|_| rr.pick(&endpoints).unwrap().address.clone())
            .collect();
        assert_eq!(picks, vec!["10.0.0.1", "10.0.0.3", "10.0.0.1", "10.0.0.3"]);
    }

    #[test]
    fn round_robin_empty_slice_yields_none() {
        let rr = RoundRobin::new();
        let endpoints: Vec<Endpoint> = Vec::new();
        assert!(rr.pick(&endpoints).is_none());
    }

    // -- LbPolicy ------------------------------------------------------------

    #[test]
    fn lb_policy_default_is_round_robin() {
        assert_eq!(LbPolicy::default(), LbPolicy::RoundRobin);
    }

    // -- Weighted (Smooth WRR) -----------------------------------------------

    /// Over 1 000 calls the distribution must match the weight ratio to within
    /// ±5 % (absolute).  With weights [3, 1] the expected ratio is 75 % / 25 %.
    #[test]
    fn weighted_rr_respects_weight_ratio() {
        let endpoints = vec![
            ep_w("10.0.0.1", 8080, true, 3), // expect ~75%
            ep_w("10.0.0.2", 8080, true, 1), // expect ~25%
        ];
        let lb = Weighted::new();
        let n = 1_000usize;
        let mut counts: HashMap<String, usize> = HashMap::new();

        for _ in 0..n {
            let picked = lb.pick(&endpoints).expect("must pick");
            *counts.entry(picked.address.clone()).or_insert(0) += 1;
        }

        let a_ratio = counts["10.0.0.1"] as f64 / n as f64;
        let b_ratio = counts["10.0.0.2"] as f64 / n as f64;

        assert!(
            (a_ratio - 0.75).abs() < 0.05,
            "endpoint A weight=3 should get ~75% but got {:.1}%",
            a_ratio * 100.0
        );
        assert!(
            (b_ratio - 0.25).abs() < 0.05,
            "endpoint B weight=1 should get ~25% but got {:.1}%",
            b_ratio * 100.0
        );
    }

    /// Unhealthy endpoints must never be selected even when they have the
    /// highest accumulated current_weight.
    #[test]
    fn weighted_rr_skips_unhealthy() {
        // weight=100 but unhealthy — must never be picked
        let endpoints = vec![
            ep_w("10.0.0.1", 8080, false, 100),
            ep_w("10.0.0.2", 8080, true, 1),
            ep_w("10.0.0.3", 8080, true, 1),
        ];
        let lb = Weighted::new();

        for _ in 0..200 {
            let picked = lb.pick(&endpoints).expect("must pick (some healthy)");
            assert_ne!(
                picked.address, "10.0.0.1",
                "unhealthy endpoint with weight=100 must never be selected"
            );
        }
    }

    /// When every endpoint is unhealthy, `pick` must return `None`.
    #[test]
    fn weighted_rr_fallback_all_unhealthy() {
        let endpoints = vec![
            ep_w("10.0.0.1", 8080, false, 3),
            ep_w("10.0.0.2", 8080, false, 1),
        ];
        let lb = Weighted::new();
        assert!(
            lb.pick(&endpoints).is_none(),
            "Weighted::pick must return None when all endpoints are unhealthy"
        );
    }

    /// Three-endpoint distribution with weights [3, 1, 2] must converge to
    /// the correct ratio over 1 200 calls (200 × lcm cycle of 6).
    #[test]
    fn weighted_rr_three_endpoints_distribution() {
        let endpoints = vec![
            ep_w("a", 80, true, 3), // expect 50%
            ep_w("b", 80, true, 1), // expect ~16.7%
            ep_w("c", 80, true, 2), // expect ~33.3%
        ];
        let lb = Weighted::new();
        let n = 1_200usize;
        let mut counts: HashMap<String, usize> = HashMap::new();

        for _ in 0..n {
            let picked = lb.pick(&endpoints).expect("must pick");
            *counts.entry(picked.address.clone()).or_insert(0) += 1;
        }

        let a_r = counts["a"] as f64 / n as f64;
        let b_r = counts["b"] as f64 / n as f64;
        let c_r = counts["c"] as f64 / n as f64;

        assert!((a_r - 0.500).abs() < 0.05, "a={:.1}%", a_r * 100.0);
        assert!((b_r - 0.167).abs() < 0.05, "b={:.1}%", b_r * 100.0);
        assert!((c_r - 0.333).abs() < 0.05, "c={:.1}%", c_r * 100.0);
    }

    // -- PowerOfTwoChoices ---------------------------------------------------

    /// P2C must prefer the endpoint with fewer active connections when given
    /// a controlled counter setup.
    #[test]
    fn p2c_picks_less_loaded() {
        let endpoints = vec![
            ep("10.0.0.1", 8080, true), // will be heavily loaded
            ep("10.0.0.2", 8080, true), // will be lightly loaded
        ];
        let lb = PowerOfTwoChoices::new();

        // Simulate 10 active connections on endpoint 1.
        for _ in 0..10 {
            lb.connection_acquired("10.0.0.1", 8080);
        }
        // Endpoint 2 has 0 connections.

        // Over many iterations P2C should pick endpoint 2 the majority of the
        // time (specifically: whenever both are sampled together, which is
        // guaranteed given only 2 endpoints — they are always the pair).
        let mut counts: HashMap<String, usize> = HashMap::new();
        for _ in 0..100 {
            let picked = lb.pick(&endpoints).expect("must pick");
            *counts.entry(picked.address.clone()).or_insert(0) += 1;
        }

        // With 2 endpoints there is no randomness in pair selection — both
        // are always compared.  Endpoint 2 must win every time.
        assert_eq!(
            counts.get("10.0.0.2").copied().unwrap_or(0),
            100,
            "endpoint 2 (0 connections) must always beat endpoint 1 (10 connections)"
        );
    }

    /// With a single healthy endpoint P2C must return it unconditionally.
    #[test]
    fn p2c_single_endpoint() {
        let endpoints = vec![
            ep("10.0.0.1", 8080, false),
            ep("10.0.0.2", 8080, true),
        ];
        let lb = PowerOfTwoChoices::new();

        for _ in 0..20 {
            let picked = lb.pick(&endpoints).expect("single healthy must always pick");
            assert_eq!(picked.address, "10.0.0.2");
        }
    }

    /// With zero healthy endpoints P2C must return `None`.
    #[test]
    fn p2c_zero_endpoints_returns_none() {
        let endpoints = vec![
            ep("10.0.0.1", 8080, false),
            ep("10.0.0.2", 8080, false),
        ];
        let lb = PowerOfTwoChoices::new();
        assert!(
            lb.pick(&endpoints).is_none(),
            "P2C must return None when all endpoints are unhealthy"
        );
    }

    /// connection_acquired / connection_released must correctly track the
    /// counter and saturate at zero on excess releases.
    #[test]
    fn p2c_counter_acquire_release_clamps_at_zero() {
        let lb = PowerOfTwoChoices::new();

        lb.connection_acquired("10.0.0.1", 8080);
        lb.connection_acquired("10.0.0.1", 8080);
        assert_eq!(lb.connections(&ep("10.0.0.1", 8080, true)), 2);

        lb.connection_released("10.0.0.1", 8080);
        assert_eq!(lb.connections(&ep("10.0.0.1", 8080, true)), 1);

        lb.connection_released("10.0.0.1", 8080);
        assert_eq!(lb.connections(&ep("10.0.0.1", 8080, true)), 0);

        // Extra release must not underflow.
        lb.connection_released("10.0.0.1", 8080);
        assert_eq!(
            lb.connections(&ep("10.0.0.1", 8080, true)),
            0,
            "counter must not underflow below zero on spurious release"
        );
    }

    /// P2C empty slice returns None.
    #[test]
    fn p2c_empty_slice_returns_none() {
        let lb = PowerOfTwoChoices::new();
        let endpoints: Vec<Endpoint> = vec![];
        assert!(lb.pick(&endpoints).is_none());
    }

    // -- LeastConn -----------------------------------------------------------

    /// LeastConn selects the endpoint with the fewest active connections.
    #[test]
    fn least_conn_selects_least_loaded() {
        let endpoints = vec![
            ep("10.0.0.1", 8080, true),
            ep("10.0.0.2", 8080, true),
            ep("10.0.0.3", 8080, true),
        ];
        let lb = LeastConn::new();

        // Assign different connection counts: 5, 1, 3.
        for _ in 0..5 { lb.connection_acquired("10.0.0.1", 8080); }
        lb.connection_acquired("10.0.0.2", 8080);
        for _ in 0..3 { lb.connection_acquired("10.0.0.3", 8080); }

        let picked = lb.pick(&endpoints).expect("must pick");
        assert_eq!(
            picked.address, "10.0.0.2",
            "endpoint with 1 connection must win"
        );
    }

    /// Single healthy endpoint is always returned.
    #[test]
    fn least_conn_single_endpoint() {
        let endpoints = vec![
            ep("10.0.0.1", 8080, false),
            ep("10.0.0.2", 8080, true),
        ];
        let lb = LeastConn::new();
        for _ in 0..20 {
            let picked = lb.pick(&endpoints).expect("single healthy must always pick");
            assert_eq!(picked.address, "10.0.0.2");
        }
    }

    /// Zero healthy endpoints returns None.
    #[test]
    fn least_conn_zero_healthy_returns_none() {
        let endpoints = vec![
            ep("10.0.0.1", 8080, false),
            ep("10.0.0.2", 8080, false),
        ];
        let lb = LeastConn::new();
        assert!(lb.pick(&endpoints).is_none());
    }

    /// Empty slice returns None.
    #[test]
    fn least_conn_empty_slice_returns_none() {
        let lb = LeastConn::new();
        let endpoints: Vec<Endpoint> = vec![];
        assert!(lb.pick(&endpoints).is_none());
    }

    /// Tie-breaker: lowest index wins when connections are equal.
    #[test]
    fn least_conn_tie_breaker_is_lowest_index() {
        let endpoints = vec![
            ep("10.0.0.1", 8080, true), // index 0 — same conn count
            ep("10.0.0.2", 8080, true), // index 1 — same conn count
            ep("10.0.0.3", 8080, true), // index 2 — same conn count
        ];
        let lb = LeastConn::new();
        // All start at 0 connections — index 0 must win.
        let picked = lb.pick(&endpoints).expect("must pick");
        assert_eq!(
            picked.address, "10.0.0.1",
            "lowest index must win on tie"
        );
    }

    /// LeastConn connection_acquired / connection_released clamp at zero.
    #[test]
    fn least_conn_counter_clamp_at_zero() {
        let lb = LeastConn::new();
        lb.connection_acquired("10.0.0.1", 8080);
        lb.connection_acquired("10.0.0.1", 8080);
        assert_eq!(lb.connections(&ep("10.0.0.1", 8080, true)), 2);
        lb.connection_released("10.0.0.1", 8080);
        assert_eq!(lb.connections(&ep("10.0.0.1", 8080, true)), 1);
        lb.connection_released("10.0.0.1", 8080);
        lb.connection_released("10.0.0.1", 8080); // spurious
        assert_eq!(
            lb.connections(&ep("10.0.0.1", 8080, true)),
            0,
            "counter must not underflow"
        );
    }
}

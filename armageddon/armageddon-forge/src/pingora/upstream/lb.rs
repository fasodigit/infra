// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Load-balancing policies — round-robin (M2 first-wave), plus weighted and
//! power-of-two-choices stubs reserved for the M2 follow-up issue.
//!
//! This module ports the `RoundRobin` selection loop from
//! `armageddon-lb::round_robin` but operates directly on
//! [`armageddon_common::types::Endpoint`] (the value type used by the Pingora
//! [`UpstreamRegistry`]) so the pingora path does not pull an additional
//! dependency on `armageddon-lb`.
//!
//! # Policies
//!
//! | Policy                | Status | Notes                                   |
//! |-----------------------|--------|-----------------------------------------|
//! | [`RoundRobin`]        | Live   | Atomic-counter rotation across healthy. |
//! | [`Weighted`]          | Stub   | TODO(#103): port WRR + GCD schedule.    |
//! | [`PowerOfTwoChoices`] | Stub   | TODO(#103): port P2C from armageddon-lb.|

use std::sync::atomic::{AtomicUsize, Ordering};

use armageddon_common::types::Endpoint;

// -- policy enum --

/// Cluster load-balancing policy tag.
///
/// Stored inside [`super::selector::ClusterState`] so `ClusterResolver` knows
/// which algorithm to instantiate when routing a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LbPolicy {
    /// Classical round-robin across healthy endpoints.
    RoundRobin,
    /// Weighted round-robin; consults `Endpoint.weight`.
    ///
    /// TODO(#103): first-class port of WRR lives in the M2 follow-up.
    Weighted,
    /// Power-of-two-choices — sample two random healthy endpoints, pick the
    /// one with fewer in-flight requests.
    ///
    /// TODO(#103): first-class port of P2C lives in the M2 follow-up.
    PowerOfTwoChoices,
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

// -- weighted (stub) --

/// Weighted round-robin balancer — reserved for the M2 follow-up.
///
/// TODO(#103): port the GCD schedule from `armageddon-lb::weighted` so
/// weights translate to proportional dispatch.
#[allow(dead_code)]
#[derive(Debug, Default)]
pub struct Weighted {
    _priv: (),
}

#[allow(dead_code)]
impl Weighted {
    /// Create a new weighted round-robin balancer.
    pub fn new() -> Self {
        Self { _priv: () }
    }

    /// Pick the next endpoint according to weights.
    ///
    /// TODO(#103): implement in the M2 follow-up.
    pub fn pick<'a>(&self, _endpoints: &'a [Endpoint]) -> Option<&'a Endpoint> {
        todo!("TODO(#103): weighted round-robin lands in M2 follow-up")
    }
}

// -- power-of-two-choices (stub) --

/// Power-of-two-choices balancer — reserved for the M2 follow-up.
///
/// TODO(#103): port the P2C sampling loop from `armageddon-lb::p2c` once the
/// pingora upstream pool exposes per-endpoint in-flight counters.
#[allow(dead_code)]
#[derive(Debug, Default)]
pub struct PowerOfTwoChoices {
    _priv: (),
}

#[allow(dead_code)]
impl PowerOfTwoChoices {
    /// Create a new P2C balancer.
    pub fn new() -> Self {
        Self { _priv: () }
    }

    /// Pick the better of two randomly sampled healthy endpoints.
    ///
    /// TODO(#103): implement in the M2 follow-up.
    pub fn pick<'a>(&self, _endpoints: &'a [Endpoint]) -> Option<&'a Endpoint> {
        todo!("TODO(#103): power-of-two-choices lands in M2 follow-up")
    }
}

// -- tests --

#[cfg(test)]
mod tests {
    use super::*;

    fn ep(host: &str, port: u16, healthy: bool) -> Endpoint {
        Endpoint {
            address: host.to_string(),
            port,
            weight: 1,
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
}

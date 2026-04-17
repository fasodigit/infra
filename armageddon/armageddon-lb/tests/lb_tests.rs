// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Integration tests for all armageddon-lb load balancing algorithms.
//!
//! Each algorithm is exercised on the following axes:
//! - Happy path: N healthy endpoints, expected distribution.
//! - Edge: 1 healthy endpoint → always that endpoint.
//! - Error: 0 healthy endpoints → `None`.

use armageddon_lb::{
    Endpoint, LeastConnections, LoadBalancer, Maglev, PowerOfTwoChoices, Random, RingHash,
    RoundRobin, WeightedRoundRobin,
};
use std::sync::Arc;

// -- helpers --

fn make_eps(n: usize) -> Vec<Arc<Endpoint>> {
    (0..n)
        .map(|i| Arc::new(Endpoint::new(format!("ep-{}", i), format!("10.0.0.{}:8080", i), 1)))
        .collect()
}

fn all_unhealthy(n: usize) -> Vec<Arc<Endpoint>> {
    let eps = make_eps(n);
    for ep in &eps {
        ep.set_healthy(false);
    }
    eps
}

fn single_healthy() -> Vec<Arc<Endpoint>> {
    make_eps(1)
}

// -- Round-Robin tests --

/// RR distributes uniformly across N endpoints (±5% tolerance).
#[test]
fn rr_uniform_distribution() {
    let eps = make_eps(4);
    let lb = RoundRobin::new();
    let mut counts = [0usize; 4];
    let n_requests = 1_000;

    for _ in 0..n_requests {
        let ep = lb.select(&eps, None).unwrap();
        let idx: usize = ep.id.split('-').nth(1).unwrap().parse().unwrap();
        counts[idx] += 1;
    }

    let expected = n_requests / 4;
    let tolerance = (expected as f64 * 0.05) as usize + 1;
    for (i, &c) in counts.iter().enumerate() {
        assert!(
            c.abs_diff(expected) <= tolerance,
            "endpoint {} got {} requests; expected {} ±{}",
            i, c, expected, tolerance
        );
    }
}

/// RR must skip unhealthy endpoints entirely.
#[test]
fn rr_skips_unhealthy() {
    let eps = make_eps(3);
    eps[1].set_healthy(false); // ep-1 is down

    let lb = RoundRobin::new();
    for _ in 0..20 {
        let ep = lb.select(&eps, None).unwrap();
        assert_ne!(ep.id, "ep-1", "RR selected unhealthy endpoint");
    }
}

/// RR returns None when all endpoints are unhealthy.
#[test]
fn rr_no_healthy_returns_none() {
    let eps = all_unhealthy(3);
    let lb = RoundRobin::new();
    assert!(lb.select(&eps, None).is_none());
}

/// RR with a single healthy endpoint always returns that endpoint.
#[test]
fn rr_single_endpoint() {
    let eps = single_healthy();
    let lb = RoundRobin::new();
    for _ in 0..10 {
        let ep = lb.select(&eps, None).unwrap();
        assert_eq!(ep.id, "ep-0");
    }
}

// -- Least-Connections tests --

/// LC selects the endpoint with fewest active connections.
#[test]
fn lc_picks_min_connections() {
    let eps = make_eps(3);
    eps[0].inc_connections(); // ep-0: 1 conn
    eps[0].inc_connections(); // ep-0: 2 conns
    eps[1].inc_connections(); // ep-1: 1 conn
    // ep-2: 0 conns → must be chosen

    let lb = LeastConnections::new();
    let chosen = lb.select(&eps, None).unwrap();
    assert_eq!(chosen.id, "ep-2");
}

/// LC returns None when all endpoints are unhealthy.
#[test]
fn lc_no_healthy_returns_none() {
    let eps = all_unhealthy(2);
    let lb = LeastConnections::new();
    assert!(lb.select(&eps, None).is_none());
}

/// LC with one endpoint returns that endpoint.
#[test]
fn lc_single_endpoint() {
    let eps = single_healthy();
    let lb = LeastConnections::new();
    assert_eq!(lb.select(&eps, None).unwrap().id, "ep-0");
}

// -- Power-of-Two-Choices tests --

/// P2C always picks from healthy endpoints; spot-check over many calls.
#[test]
fn p2c_only_healthy() {
    let eps = make_eps(4);
    eps[2].set_healthy(false);
    let lb = PowerOfTwoChoices::new();
    for _ in 0..200 {
        let ep = lb.select(&eps, None).unwrap();
        assert_ne!(ep.id, "ep-2");
    }
}

/// P2C returns None when all endpoints are unhealthy.
#[test]
fn p2c_no_healthy_returns_none() {
    let eps = all_unhealthy(3);
    let lb = PowerOfTwoChoices::new();
    assert!(lb.select(&eps, None).is_none());
}

/// P2C with a single endpoint always returns it.
#[test]
fn p2c_single_endpoint() {
    let eps = single_healthy();
    let lb = PowerOfTwoChoices::new();
    for _ in 0..10 {
        assert_eq!(lb.select(&eps, None).unwrap().id, "ep-0");
    }
}

/// P2C prefers the less-loaded choice when connections differ substantially.
#[test]
fn p2c_prefers_less_loaded() {
    // Two endpoints: ep-0 overloaded, ep-1 idle.
    let eps = make_eps(2);
    for _ in 0..100 {
        eps[0].inc_connections();
    }
    let lb = PowerOfTwoChoices::new();
    // With only 2 healthy endpoints P2C always compares both → picks ep-1.
    for _ in 0..20 {
        let ep = lb.select(&eps, None).unwrap();
        assert_eq!(ep.id, "ep-1", "P2C should prefer less-loaded ep-1");
    }
}

// -- Ring-Hash tests --

/// Same hash_key must always produce the same endpoint (determinism).
#[test]
fn ring_hash_deterministic() {
    let endpoints: Vec<Arc<Endpoint>> = (0..5)
        .map(|i| Arc::new(Endpoint::new(format!("ep-{}", i), format!("10.0.0.{}:8080", i), 1)))
        .collect();
    let lb = RingHash::new(endpoints.clone());

    let key = b"user-12345";
    let first = lb.select(&endpoints, Some(key)).unwrap().id.clone();
    for _ in 0..50 {
        let got = lb.select(&endpoints, Some(key)).unwrap().id.clone();
        assert_eq!(got, first, "ring_hash must be deterministic for the same key");
    }
}

/// Ring-hash skips an unhealthy endpoint and routes elsewhere.
#[test]
fn ring_hash_skips_unhealthy() {
    let endpoints: Vec<Arc<Endpoint>> = (0..4)
        .map(|i| Arc::new(Endpoint::new(format!("ep-{}", i), format!("10.0.0.{}:8080", i), 1)))
        .collect();

    // Mark all unhealthy except ep-3.
    endpoints[0].set_healthy(false);
    endpoints[1].set_healthy(false);
    endpoints[2].set_healthy(false);

    let lb = RingHash::new(endpoints.clone());
    for i in 0u64..20 {
        let key = i.to_le_bytes();
        let ep = lb.select(&endpoints, Some(&key)).unwrap();
        assert_eq!(ep.id, "ep-3", "only ep-3 is healthy");
    }
}

/// Ring-hash returns None when all endpoints are unhealthy.
#[test]
fn ring_hash_no_healthy_returns_none() {
    let endpoints: Vec<Arc<Endpoint>> = make_eps(3);
    for ep in &endpoints {
        ep.set_healthy(false);
    }
    let lb = RingHash::new(endpoints.clone());
    assert!(lb.select(&endpoints, Some(b"any-key")).is_none());
}

// -- Maglev tests --

/// Maglev lookup is deterministic: same key → same endpoint.
#[test]
fn maglev_deterministic() {
    let endpoints: Vec<Arc<Endpoint>> = (0..4)
        .map(|i| Arc::new(Endpoint::new(format!("ep-{}", i), format!("10.0.0.{}:8080", i), 1)))
        .collect();
    let lb = Maglev::new(endpoints.clone());
    let key = b"session-abc";
    let first = lb.select(&endpoints, Some(key)).unwrap().id.clone();
    for _ in 0..50 {
        let got = lb.select(&endpoints, Some(key)).unwrap().id.clone();
        assert_eq!(got, first);
    }
}

/// Maglev distributes load across all endpoints (none left empty over 1000 keys).
#[test]
fn maglev_distribution() {
    let n = 4usize;
    let endpoints: Vec<Arc<Endpoint>> = (0..n)
        .map(|i| Arc::new(Endpoint::new(format!("ep-{}", i), format!("10.0.0.{}:8080", i), 1)))
        .collect();
    let lb = Maglev::new(endpoints.clone());

    let mut hits = vec![0usize; n];
    for i in 0u64..1_000 {
        let key = i.to_le_bytes();
        let ep = lb.select(&endpoints, Some(&key)).unwrap();
        let idx: usize = ep.id.split('-').nth(1).unwrap().parse().unwrap();
        hits[idx] += 1;
    }

    // Each endpoint should receive at least 1% of traffic (far below 25% theoretical).
    for (i, &h) in hits.iter().enumerate() {
        assert!(h >= 10, "endpoint {} received only {} hits out of 1000", i, h);
    }
}

/// Maglev returns None when all endpoints are unhealthy.
#[test]
fn maglev_no_healthy_returns_none() {
    let eps = all_unhealthy(3);
    let lb = Maglev::new(eps.clone());
    assert!(lb.select(&eps, Some(b"key")).is_none());
}

// -- Weighted Round-Robin tests --

/// WRR respects weight ratios 1:2:3 with ±15% tolerance over 600 calls.
#[test]
fn wrr_weight_ratio() {
    let endpoints = vec![
        Arc::new(Endpoint::new("ep-0", "10.0.0.0:8080", 1)),
        Arc::new(Endpoint::new("ep-1", "10.0.0.1:8080", 2)),
        Arc::new(Endpoint::new("ep-2", "10.0.0.2:8080", 3)),
    ];
    let lb = WeightedRoundRobin::new(endpoints.clone());

    let mut counts = [0usize; 3];
    let total = 600usize;
    for _ in 0..total {
        let ep = lb.select(&endpoints, None).unwrap();
        let idx: usize = ep.id.split('-').nth(1).unwrap().parse().unwrap();
        counts[idx] += 1;
    }

    // Expected: 100, 200, 300.
    let expected = [100usize, 200, 300];
    for (i, (&got, &exp)) in counts.iter().zip(expected.iter()).enumerate() {
        let tolerance = (exp as f64 * 0.15) as usize + 1;
        assert!(
            got.abs_diff(exp) <= tolerance,
            "endpoint {} got {} (expected {} ±{})",
            i, got, exp, tolerance
        );
    }
}

/// WRR returns None when all endpoints are unhealthy.
#[test]
fn wrr_no_healthy_returns_none() {
    let eps = all_unhealthy(3);
    let lb = WeightedRoundRobin::new(eps.clone());
    assert!(lb.select(&eps, None).is_none());
}

/// WRR with single endpoint always returns it.
#[test]
fn wrr_single_endpoint() {
    let eps = single_healthy();
    let lb = WeightedRoundRobin::new(eps.clone());
    for _ in 0..10 {
        assert_eq!(lb.select(&eps, None).unwrap().id, "ep-0");
    }
}

// -- Random tests --

/// Random distributes uniformly across 4 endpoints (±10% on 10k samples).
#[test]
fn random_uniform_distribution() {
    let eps = make_eps(4);
    let lb = Random::new();
    let mut counts = [0usize; 4];
    let n = 10_000usize;
    for _ in 0..n {
        let ep = lb.select(&eps, None).unwrap();
        let idx: usize = ep.id.split('-').nth(1).unwrap().parse().unwrap();
        counts[idx] += 1;
    }
    let expected = n / 4;
    let tolerance = (expected as f64 * 0.10) as usize + 1;
    for (i, &c) in counts.iter().enumerate() {
        assert!(
            c.abs_diff(expected) <= tolerance,
            "random: endpoint {} got {} (expected {} ±{})",
            i, c, expected, tolerance
        );
    }
}

/// Random returns None when all endpoints are unhealthy.
#[test]
fn random_no_healthy_returns_none() {
    let eps = all_unhealthy(3);
    let lb = Random::new();
    assert!(lb.select(&eps, None).is_none());
}

/// Random with single endpoint always returns it.
#[test]
fn random_single_endpoint() {
    let eps = single_healthy();
    let lb = Random::new();
    for _ in 0..10 {
        assert_eq!(lb.select(&eps, None).unwrap().id, "ep-0");
    }
}

// -- `name()` sanity checks --

#[test]
fn algorithm_names() {
    assert_eq!(RoundRobin::new().name(), "round_robin");
    assert_eq!(LeastConnections::new().name(), "least_connections");
    assert_eq!(PowerOfTwoChoices::new().name(), "p2c");
    assert_eq!(Random::new().name(), "random");

    let eps = make_eps(2);
    let rh = RingHash::new(eps.clone());
    assert_eq!(rh.name(), "ring_hash");

    let m = Maglev::new(eps.clone());
    assert_eq!(m.name(), "maglev");

    let w = WeightedRoundRobin::new(eps);
    assert_eq!(w.name(), "weighted_round_robin");
}

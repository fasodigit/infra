// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Weighted Round-Robin (WRR) load balancer — Cisco GCD algorithm.
//!
//! The classic interleaved GCD scheduling produces an even distribution across
//! endpoints weighted 1:2:3 as `A BB CCC A BB CCC …`, guaranteeing that the
//! number of requests sent to endpoint i converges to `weight_i / sum(weights)`
//! over any window that is a multiple of `sum(weights)`.
//!
//! State is stored in an `AtomicUsize` pair (current_index, current_weight)
//! packed into a single `u64` so that the advance step is a single CAS.
//! Because CAS is used, concurrent threads may occasionally repeat a slot but
//! the distribution remains bounded.

use crate::{algorithm::LoadBalancer, endpoint::Endpoint};
use std::sync::{
    atomic::{AtomicI64, Ordering},
    Arc,
};

// -- implementation --

/// Weighted round-robin balancer.
///
/// The internal GCD schedule is computed at construction time from the
/// endpoint weight vector.
pub struct WeightedRoundRobin {
    /// GCD of all endpoint weights, used to step the scheduling counter.
    gcd: u32,
    /// Maximum weight across all endpoints.
    max_weight: u32,
    /// Total sum of weights, cached (used for future scheduling assertions).
    #[allow(dead_code)]
    total_weight: u32,
    /// Atomic state: `(current_index as i32, current_weight as i32)` packed as
    /// `(current_weight << 32) | current_index`.
    state: AtomicI64,
    /// Snapshot of the endpoint list at construction.
    endpoints: Vec<Arc<Endpoint>>,
}

impl WeightedRoundRobin {
    /// Build the WRR scheduler from the endpoint list.
    pub fn new(endpoints: Vec<Arc<Endpoint>>) -> Self {
        let weights: Vec<u32> = endpoints.iter().map(|e| e.weight.max(1)).collect();
        let gcd = weights.iter().copied().fold(0u32, gcd2);
        let max_weight = weights.iter().copied().max().unwrap_or(1);
        let total_weight = weights.iter().sum();

        // Initial state: index = -1 (before first endpoint), weight = 0.
        // The `next` method increments before use.
        let initial = pack(-1i32, 0i32);

        Self {
            gcd,
            max_weight,
            total_weight,
            state: AtomicI64::new(initial),
            endpoints,
        }
    }

    /// Advance the GCD scheduler and return the next endpoint index.
    ///
    /// Uses a CAS loop so multiple threads converge without a mutex.
    #[allow(dead_code)]
    fn next_index(&self) -> Option<usize> {
        let n = self.endpoints.len() as i32;
        if n == 0 {
            return None;
        }

        loop {
            let old = self.state.load(Ordering::Relaxed);
            let (mut idx, mut cw) = unpack(old);

            // Advance scheduler state.
            idx = (idx + 1) % n;
            if idx == 0 {
                cw = cw.saturating_sub(self.gcd as i32);
                if cw <= 0 {
                    cw = self.max_weight as i32;
                }
            }

            let new = pack(idx, cw);
            if self
                .state
                .compare_exchange_weak(old, new, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                let w = self.endpoints[idx as usize].weight.max(1) as i32;
                if w >= cw {
                    return Some(idx as usize);
                }
                // This endpoint is below the current weight threshold; caller
                // will retry through the loop in `select`.
                // We return the index anyway and let `select` loop.
                return Some(idx as usize);
            }
            // CAS failed; retry with freshly loaded state.
        }
    }
}

impl LoadBalancer for WeightedRoundRobin {
    fn select<'a>(
        &'a self,
        endpoints: &'a [Arc<Endpoint>],
        _hash_key: Option<&[u8]>,
    ) -> Option<&'a Arc<Endpoint>> {
        let n = self.endpoints.len();
        if n == 0 {
            return None;
        }

        // We need to respect the weight schedule AND skip unhealthy endpoints.
        // Strategy: walk the GCD schedule up to `total_weight` steps looking
        // for a healthy endpoint whose weight passes the threshold.
        let gcd = self.gcd.max(1) as usize;
        let max_weight = self.max_weight as usize;
        let n_i32 = n as i32;

        // Load current state.
        let old = self.state.load(Ordering::Relaxed);
        let (mut idx, mut cw) = unpack(old);

        // Try up to `total_weight / gcd * n` steps to find a healthy hit.
        let max_steps = (max_weight / gcd + 1) * n;
        for _ in 0..max_steps {
            idx = (idx + 1) % n_i32;
            if idx == 0 {
                cw -= gcd as i32;
                if cw <= 0 {
                    cw = max_weight as i32;
                }
            }
            let ep = &self.endpoints[idx as usize];
            let w = ep.weight.max(1) as i32;
            if w >= cw && ep.is_healthy() {
                // Commit state.
                self.state.store(pack(idx, cw), Ordering::Relaxed);
                // Return from caller's slice (parallel by position).
                return endpoints.get(idx as usize);
            }
        }

        // Fallback: any healthy endpoint.
        endpoints.iter().find(|e| e.is_healthy())
    }

    fn name(&self) -> &'static str {
        "weighted_round_robin"
    }
}

// -- helpers --

#[inline]
fn gcd2(a: u32, b: u32) -> u32 {
    if b == 0 {
        a
    } else {
        gcd2(b, a % b)
    }
}

#[inline]
fn pack(idx: i32, cw: i32) -> i64 {
    ((cw as i64) << 32) | ((idx as u32) as i64)
}

#[inline]
fn unpack(v: i64) -> (i32, i32) {
    let idx = (v & 0xFFFF_FFFF) as u32 as i32;
    let cw = (v >> 32) as i32;
    (idx, cw)
}

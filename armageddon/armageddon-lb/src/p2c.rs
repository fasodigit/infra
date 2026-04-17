// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Power-of-Two-Choices (P2C) load balancer.
//!
//! On each call the algorithm draws two distinct healthy endpoints uniformly at
//! random and returns the one with fewer active connections.  This achieves
//! O(log log n) max-load improvement over pure random selection while avoiding
//! the thundering-herd effect of global least-connections under high concurrency.
//!
//! Reference: Mitzenmacher (1996), "The Power of Two Random Choices".

use crate::{algorithm::LoadBalancer, endpoint::Endpoint};
use rand::Rng;
use std::sync::Arc;

// -- implementation --

/// Power-of-Two-Choices balancer.  Stateless beyond thread-local RNG.
#[derive(Debug, Default)]
pub struct PowerOfTwoChoices;

impl PowerOfTwoChoices {
    pub fn new() -> Self {
        Self
    }
}

impl LoadBalancer for PowerOfTwoChoices {
    fn select<'a>(
        &'a self,
        endpoints: &'a [Arc<Endpoint>],
        _hash_key: Option<&[u8]>,
    ) -> Option<&'a Arc<Endpoint>> {
        let healthy: Vec<&Arc<Endpoint>> = endpoints.iter().filter(|e| e.is_healthy()).collect();

        match healthy.len() {
            0 => None,
            1 => Some(healthy[0]),
            _ => {
                let mut rng = rand::thread_rng();
                // Pick two distinct indices.
                let a = rng.gen_range(0..healthy.len());
                let mut b = rng.gen_range(0..healthy.len() - 1);
                if b >= a {
                    b += 1;
                }
                if healthy[a].connections() <= healthy[b].connections() {
                    Some(healthy[a])
                } else {
                    Some(healthy[b])
                }
            }
        }
    }

    fn name(&self) -> &'static str {
        "p2c"
    }
}

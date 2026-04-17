// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Round-Robin load balancer.
//!
//! A single atomic counter advances monotonically.  On each call the algorithm
//! filters the endpoint slice to only healthy members and picks
//! `counter % healthy_count`, guaranteeing uniform distribution across the
//! healthy pool.

use crate::{algorithm::LoadBalancer, endpoint::Endpoint};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

// -- implementation --

/// Stateless round-robin; distributes evenly across healthy endpoints.
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
}

impl LoadBalancer for RoundRobin {
    fn select<'a>(
        &'a self,
        endpoints: &'a [Arc<Endpoint>],
        _hash_key: Option<&[u8]>,
    ) -> Option<&'a Arc<Endpoint>> {
        let healthy: Vec<&Arc<Endpoint>> = endpoints.iter().filter(|e| e.is_healthy()).collect();

        if healthy.is_empty() {
            return None;
        }

        let idx = self.counter.fetch_add(1, Ordering::Relaxed) % healthy.len();
        Some(healthy[idx])
    }

    fn name(&self) -> &'static str {
        "round_robin"
    }
}

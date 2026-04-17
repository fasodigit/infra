// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Uniform-random load balancer.
//!
//! Selects uniformly at random from the pool of healthy endpoints using
//! `rand::thread_rng`, which provides a per-thread seeded PRNG without any
//! shared state.

use crate::{algorithm::LoadBalancer, endpoint::Endpoint};
use rand::Rng;
use std::sync::Arc;

// -- implementation --

/// Uniform-random endpoint selection.
#[derive(Debug, Default)]
pub struct Random;

impl Random {
    pub fn new() -> Self {
        Self
    }
}

impl LoadBalancer for Random {
    fn select<'a>(
        &'a self,
        endpoints: &'a [Arc<Endpoint>],
        _hash_key: Option<&[u8]>,
    ) -> Option<&'a Arc<Endpoint>> {
        let healthy: Vec<&Arc<Endpoint>> = endpoints.iter().filter(|e| e.is_healthy()).collect();

        if healthy.is_empty() {
            return None;
        }

        let idx = rand::thread_rng().gen_range(0..healthy.len());
        Some(healthy[idx])
    }

    fn name(&self) -> &'static str {
        "random"
    }
}

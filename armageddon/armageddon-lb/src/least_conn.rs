// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Least-Connections load balancer.
//!
//! Among all healthy endpoints, always picks the one with the fewest
//! in-flight connections.  Ties are broken by position (first encountered),
//! which in practice yields fair behaviour when weights are equal.

use crate::{algorithm::LoadBalancer, endpoint::Endpoint};
use std::sync::Arc;

// -- implementation --

/// Picks the healthy endpoint with the minimum active-connection count.
#[derive(Debug, Default)]
pub struct LeastConnections;

impl LeastConnections {
    pub fn new() -> Self {
        Self
    }
}

impl LoadBalancer for LeastConnections {
    fn select<'a>(
        &'a self,
        endpoints: &'a [Arc<Endpoint>],
        _hash_key: Option<&[u8]>,
    ) -> Option<&'a Arc<Endpoint>> {
        endpoints
            .iter()
            .filter(|e| e.is_healthy())
            .min_by_key(|e| e.connections())
    }

    fn name(&self) -> &'static str {
        "least_connections"
    }
}

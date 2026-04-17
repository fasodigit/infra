// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Endpoint definition for the ARMAGEDDON load balancer.
//!
//! Each `Endpoint` is a reference-counted, atomically-updated upstream target.
//! All health / connection state is mutated via atomics so it is safe to share
//! across threads without a mutex.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

// -- types --

/// A single upstream backend target.
///
/// Weights are read when building consistent-hash rings or when the weighted
/// round-robin algorithm computes the GCD schedule.  `active_connections` is
/// incremented by the proxy layer when a connection is checked out and
/// decremented when it is returned; `healthy` is toggled by the health-check
/// subsystem.
#[derive(Debug)]
pub struct Endpoint {
    /// Stable, human-readable identifier (e.g. `"backend-0"`).
    pub id: String,
    /// Socket address of the upstream, e.g. `"10.0.0.1:8080"`.
    pub address: String,
    /// Relative weight used by WRR and consistent-hash virtual-node expansion.
    /// Must be >= 1.
    pub weight: u32,
    /// Number of in-flight connections currently routed to this endpoint.
    pub active_connections: AtomicUsize,
    /// Whether the health-check subsystem considers this endpoint alive.
    pub healthy: AtomicBool,
}

impl Endpoint {
    /// Construct a new healthy endpoint with zero active connections.
    pub fn new(id: impl Into<String>, address: impl Into<String>, weight: u32) -> Self {
        Self {
            id: id.into(),
            address: address.into(),
            weight,
            active_connections: AtomicUsize::new(0),
            healthy: AtomicBool::new(true),
        }
    }

    /// Return true when the health-check subsystem considers this endpoint reachable.
    #[inline]
    pub fn is_healthy(&self) -> bool {
        self.healthy.load(Ordering::Acquire)
    }

    /// Snapshot of currently active connections (relaxed; used for selection heuristics).
    #[inline]
    pub fn connections(&self) -> usize {
        self.active_connections.load(Ordering::Relaxed)
    }

    /// Mark the endpoint healthy or unhealthy.
    #[inline]
    pub fn set_healthy(&self, v: bool) {
        self.healthy.store(v, Ordering::Release);
    }

    /// Increment the active-connection counter (call when a request is dispatched).
    #[inline]
    pub fn inc_connections(&self) {
        self.active_connections.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement the active-connection counter (call when a request completes).
    #[inline]
    pub fn dec_connections(&self) {
        self.active_connections.fetch_sub(1, Ordering::Relaxed);
    }
}

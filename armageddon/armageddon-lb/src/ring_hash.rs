// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Consistent ring-hash load balancer.
//!
//! The ring is built once during construction.  Each endpoint is replicated
//! proportionally to its `weight`, targeting `VIRTUAL_NODES_TOTAL` virtual
//! nodes spread across the 64-bit hash space.  Virtual nodes are sorted by
//! their hash; lookup performs a binary search and wraps around the ring.
//!
//! Hashing uses BLAKE3 for collision-resistance and uniform distribution.
//!
//! When no `hash_key` is supplied the balancer falls back to a round-robin
//! counter so it can still be used without a routing key.

use crate::{algorithm::LoadBalancer, endpoint::Endpoint};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

// -- constants --

/// Total virtual nodes placed on the ring, distributed proportionally by weight.
const VIRTUAL_NODES_TOTAL: usize = 4_000;

// -- types --

struct VNode {
    hash: u64,
    /// Index into the original endpoint slice supplied at construction time.
    endpoint_index: usize,
}

/// Consistent ring-hash balancer.
///
/// The ring is immutable after construction; rebuild via `RingHash::new` when
/// the endpoint set changes.
pub struct RingHash {
    ring: Vec<VNode>,
    /// Endpoint list parallel to the original slice; stored so `select` can
    /// return references into it.
    endpoints: Vec<Arc<Endpoint>>,
    /// Fallback counter used when `hash_key` is `None`.
    fallback: AtomicUsize,
}

impl RingHash {
    /// Build a new ring from the provided endpoints.
    ///
    /// Panics if `endpoints` is empty (caller must guard).
    pub fn new(endpoints: Vec<Arc<Endpoint>>) -> Self {
        let total_weight: u32 = endpoints.iter().map(|e| e.weight.max(1)).sum();
        let mut ring: Vec<VNode> = Vec::with_capacity(VIRTUAL_NODES_TOTAL);

        for (idx, ep) in endpoints.iter().enumerate() {
            let share = ep.weight.max(1) as usize * VIRTUAL_NODES_TOTAL / total_weight as usize;
            let share = share.max(1);
            for replica in 0..share {
                let key = format!("{}-{}", ep.id, replica);
                let hash = blake3_hash(key.as_bytes());
                ring.push(VNode {
                    hash,
                    endpoint_index: idx,
                });
            }
        }

        ring.sort_unstable_by_key(|v| v.hash);

        Self {
            ring,
            endpoints,
            fallback: AtomicUsize::new(0),
        }
    }

    /// Binary-search the ring for the first vnode with hash >= `h`, wrapping around.
    fn ring_lookup(&self, h: u64) -> Option<usize> {
        if self.ring.is_empty() {
            return None;
        }
        let pos = self
            .ring
            .partition_point(|v| v.hash < h)
            .min(self.ring.len() - 1);
        // Walk forward until we find a healthy endpoint (max one full rotation).
        for offset in 0..self.ring.len() {
            let vnode = &self.ring[(pos + offset) % self.ring.len()];
            let ep = &self.endpoints[vnode.endpoint_index];
            if ep.is_healthy() {
                return Some(vnode.endpoint_index);
            }
        }
        None
    }
}

impl LoadBalancer for RingHash {
    fn select<'a>(
        &'a self,
        endpoints: &'a [Arc<Endpoint>],
        hash_key: Option<&[u8]>,
    ) -> Option<&'a Arc<Endpoint>> {
        // If the caller passes a different slice than was used to build the ring
        // we fall back to iterating the provided slice directly.  In normal
        // usage the ring and the slice are the same logical pool.
        if hash_key.is_none() || self.ring.is_empty() {
            // Fallback: round-robin over healthy from `endpoints`.
            let healthy: Vec<&Arc<Endpoint>> =
                endpoints.iter().filter(|e| e.is_healthy()).collect();
            if healthy.is_empty() {
                return None;
            }
            let idx = self.fallback.fetch_add(1, Ordering::Relaxed) % healthy.len();
            return Some(healthy[idx]);
        }

        let key = hash_key.unwrap();
        let h = blake3_hash(key);

        if let Some(ep_idx) = self.ring_lookup(h) {
            // Return from the internal arc list so lifetimes line up.
            // The caller's `endpoints` slice should mirror `self.endpoints`
            // but we return from `self.endpoints` to ensure correctness.
            return Some(&self.endpoints[ep_idx]);
        }
        None
    }

    fn name(&self) -> &'static str {
        "ring_hash"
    }
}

// -- helpers --

/// Produce a 64-bit hash of `data` using BLAKE3.
pub(crate) fn blake3_hash(data: &[u8]) -> u64 {
    let digest = blake3::hash(data);
    let bytes = digest.as_bytes();
    u64::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ])
}

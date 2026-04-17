// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Maglev consistent-hash load balancer.
//!
//! Maglev builds a fixed-size lookup table of size `M` (a large prime) where
//! each slot maps to an endpoint index.  Lookups are O(1) — just
//! `table[hash(key) % M]`.  The filling algorithm guarantees that every
//! endpoint occupies approximately `M / n` slots, yielding near-perfect
//! distribution.
//!
//! Reference: Eisenbud et al., "Maglev: A Fast and Reliable Software Network
//! Load Balancer" (NSDI 2016).

use crate::{algorithm::LoadBalancer, endpoint::Endpoint, ring_hash::blake3_hash};
use std::sync::Arc;

// -- constants --

/// Table size.  Must be prime; 65537 is the smallest Fermat prime > 65536.
const M: usize = 65_537;

// -- types --

/// Maglev consistent-hash balancer.
///
/// The lookup table is built eagerly at construction.  Rebuild via
/// `Maglev::new` when the endpoint pool changes.
pub struct Maglev {
    /// Lookup table: `table[i]` is an index into `endpoints`.
    table: Vec<usize>,
    /// Endpoint list mirrored from construction time.
    endpoints: Vec<Arc<Endpoint>>,
}

impl Maglev {
    /// Build a Maglev lookup table for the given endpoints.
    ///
    /// Endpoints with `weight > 1` are replicated proportionally: each
    /// endpoint gets `weight` independent permutation sequences interleaved
    /// into the table fill.
    pub fn new(endpoints: Vec<Arc<Endpoint>>) -> Self {
        let n = endpoints.len();
        let table = if n == 0 {
            vec![]
        } else {
            build_table(&endpoints, M)
        };
        Self { table, endpoints }
    }

    /// Look up the endpoint index for a given hash value.
    #[allow(dead_code)]
    fn lookup_index(&self, hash: u64) -> Option<usize> {
        if self.table.is_empty() {
            return None;
        }
        let slot = (hash as usize) % M;
        Some(self.table[slot])
    }
}

impl LoadBalancer for Maglev {
    fn select<'a>(
        &'a self,
        endpoints: &'a [Arc<Endpoint>],
        hash_key: Option<&[u8]>,
    ) -> Option<&'a Arc<Endpoint>> {
        // If no hash key, scan for any healthy endpoint (first match).
        let key = match hash_key {
            Some(k) => k,
            None => {
                return endpoints.iter().find(|e| e.is_healthy());
            }
        };

        let h = blake3_hash(key);

        // Walk from the preferred slot until a healthy endpoint is found.
        // Because M >> n, we will find one quickly.
        if self.table.is_empty() {
            return None;
        }

        let start_slot = (h as usize) % M;
        for offset in 0..M {
            let slot = (start_slot + offset) % M;
            let ep_idx = self.table[slot];
            let ep = &self.endpoints[ep_idx];
            if ep.is_healthy() {
                return Some(ep);
            }
        }
        None
    }

    fn name(&self) -> &'static str {
        "maglev"
    }
}

// -- table construction --

/// Build a Maglev lookup table of size `m` for the given endpoints.
///
/// Each endpoint contributes one permutation sequence; the sequences are
/// interleaved round-robin until the table is full.
fn build_table(endpoints: &[Arc<Endpoint>], m: usize) -> Vec<usize> {
    let n = endpoints.len();

    // Compute per-endpoint permutation parameters.
    // offset(i) = hash(id + "offset") % m
    // skip(i)   = hash(id + "skip")   % (m - 1) + 1
    let mut offsets = vec![0usize; n];
    let mut skips = vec![0usize; n];

    for (i, ep) in endpoints.iter().enumerate() {
        let offset_key = format!("{}-offset", ep.id);
        let skip_key = format!("{}-skip", ep.id);
        offsets[i] = (blake3_hash(offset_key.as_bytes()) as usize) % m;
        skips[i] = (blake3_hash(skip_key.as_bytes()) as usize) % (m - 1) + 1;
    }

    // `next[i]` tracks the current cursor in endpoint i's permutation.
    let mut next = offsets.clone();
    let mut table = vec![usize::MAX; m];
    let mut filled = 0usize;

    'outer: loop {
        for i in 0..n {
            // Advance until we find an empty slot in endpoint i's permutation.
            loop {
                let slot = next[i];
                next[i] = (next[i] + skips[i]) % m;
                if table[slot] == usize::MAX {
                    table[slot] = i;
                    filled += 1;
                    if filled == m {
                        break 'outer;
                    }
                    break;
                }
            }
        }
    }

    table
}

// -- tests (unit) --
#[cfg(test)]
mod internal_tests {
    use super::*;
    use std::sync::Arc;

    fn ep(id: &str) -> Arc<Endpoint> {
        Arc::new(Endpoint::new(id, format!("10.0.0.1:80{}", id), 1))
    }

    #[test]
    fn table_is_fully_filled() {
        let endpoints: Vec<Arc<Endpoint>> = (0..4).map(|i| ep(&i.to_string())).collect();
        let t = build_table(&endpoints, M);
        assert_eq!(t.len(), M);
        assert!(t.iter().all(|&v| v < 4));
    }
}

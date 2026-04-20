// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Upstream selection for the Pingora gateway.
//!
//! This module ports the **SPIFFE-ID-aware [`PoolKey`]** from the hyper path
//! ([`crate::upstream_pool::PoolKey`]) and introduces a [`ClusterResolver`]
//! that the Pingora [`upstream_peer`] hook calls to translate a logical
//! cluster name into a concrete [`ResolvedPeer`].
//!
//! # Why `PoolKey` matters (security invariant — bug_006)
//!
//! Keying the upstream connection map by `SocketAddr` alone was previously
//! sufficient — until mTLS entered the picture.  Two problems then emerged:
//!
//! 1. **Crypto downgrade.**  A plaintext connection cached for `addr`
//!    could be served to an mTLS caller, silently bypassing TLS.
//! 2. **Cross-identity impersonation.**  Two clusters at the same `addr`
//!    with different peer SPIFFE IDs would share the same pool slot, so
//!    traffic intended for `spiffe://faso/ns/a` could be handed to a
//!    connection authenticated against `spiffe://faso/ns/b`.
//!
//! The [`PoolKey::Plain`] / [`PoolKey::Mtls`] split prevents both: the
//! variant tag isolates plaintext from mTLS buckets, and the embedded
//! `Arc<str>` SPIFFE ID differentiates mTLS peers at the same socket.  The
//! regression tests at the bottom of this file lock the invariant in.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;

use arc_swap::ArcSwap;
use tracing::{debug, error, warn};

use armageddon_common::types::Endpoint;

use super::lb::{LbPolicy, RoundRobin};

// -- pool key --

/// Key for the upstream connection map, segregating plaintext from mTLS
/// connections and isolating mTLS entries by peer SPIFFE ID.
///
/// This type is the **byte-for-byte semantic twin** of
/// [`crate::upstream_pool::PoolKey`] on the hyper path.  Both paths need
/// identical hashing / equality semantics so the ARMAGEDDON security
/// invariants (no crypto downgrade, no cross-SPIFFE-ID impersonation)
/// hold regardless of which backend is compiled in.
///
/// # Invariants
///
/// - `PoolKey::Plain(a)` and `PoolKey::Mtls(a, _)` are never equal even for
///   the same `a`; they hash into different buckets.
/// - `PoolKey::Mtls(a, id1)` and `PoolKey::Mtls(a, id2)` with
///   `id1 != id2` are distinct.  `Arc<str>` is hashed and compared by
///   value (string contents), not by Arc pointer identity.
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum PoolKey {
    /// Plain TCP connection to `addr`.
    Plain(SocketAddr),
    /// mTLS connection to `addr` with peer identified by the contained
    /// SPIFFE ID.  `Arc<str>` is hashed / compared by string contents.
    Mtls(SocketAddr, Arc<str>),
}

impl PoolKey {
    /// Return the underlying `SocketAddr` regardless of variant.
    pub fn addr(&self) -> SocketAddr {
        match self {
            PoolKey::Plain(a) | PoolKey::Mtls(a, _) => *a,
        }
    }

    /// Return the peer SPIFFE ID when this is an mTLS key.
    pub fn spiffe_id(&self) -> Option<&str> {
        match self {
            PoolKey::Plain(_) => None,
            PoolKey::Mtls(_, id) => Some(id.as_ref()),
        }
    }
}

// -- cluster state --

/// Immutable snapshot of one cluster's routing decision inputs.
///
/// Stored inside an [`ArcSwap`] so `update` is a single atomic pointer swap
/// and `resolve` never blocks writers.
#[derive(Debug, Clone)]
pub struct ClusterState {
    /// Endpoints that make up the cluster.  Health is read from
    /// `Endpoint.healthy`.
    pub endpoints: Vec<Endpoint>,
    /// Whether the cluster mandates mTLS to the upstream.
    pub tls_required: bool,
    /// Expected peer SPIFFE ID when `tls_required` is `true`.  Required for
    /// safe mTLS resolution — if `None`, `resolve` returns `None` with an
    /// error log rather than accept an unauthenticated peer.
    pub expected_spiffe_id: Option<Arc<str>>,
    /// Load-balancing policy applied by `resolve` when more than one healthy
    /// endpoint is available.
    pub lb_policy: LbPolicy,
}

impl ClusterState {
    /// Build a new cluster state.  Convenience constructor for tests and
    /// external callers.
    pub fn new(
        endpoints: Vec<Endpoint>,
        tls_required: bool,
        expected_spiffe_id: Option<Arc<str>>,
        lb_policy: LbPolicy,
    ) -> Self {
        Self {
            endpoints,
            tls_required,
            expected_spiffe_id,
            lb_policy,
        }
    }
}

// -- resolved peer --

/// Outcome of [`ClusterResolver::resolve`] — carries both the resolved socket
/// and the [`PoolKey`] that the upstream connection pool must use for the
/// dispatch.
#[derive(Debug, Clone)]
pub struct ResolvedPeer {
    /// Resolved upstream socket address.
    pub addr: SocketAddr,
    /// Pool key to use when checking out a connection — `Plain` for
    /// plaintext clusters, `Mtls` for SPIFFE-annotated clusters.
    pub pool_key: PoolKey,
    /// `true` when the cluster mandates TLS.  Mirrors the
    /// `Plain` / `Mtls` distinction on `pool_key` but is convenient for
    /// callers that only need the flag (e.g. `pingora::HttpPeer::new`).
    pub tls: bool,
}

// -- resolver --

/// Thread-safe, hot-reloadable cluster-to-peer resolver.
///
/// The resolver holds an `ArcSwap<HashMap<String, Arc<ClusterState>>>` so:
///
/// - Readers (the Pingora `upstream_peer` hook) dereference the map via
///   `load()` in a few nanoseconds, never touching a lock.
/// - Writers (xDS / admin API hot-reload paths) build a fresh map and
///   atomic-swap it in; readers see the old snapshot until the swap
///   publishes.
///
/// The round-robin counter is stored **inside** the resolver rather than
/// inside `ClusterState` so a hot-reload of the cluster (new endpoint
/// list, same name) does not reset the counter and introduce traffic
/// bursts to the first endpoint.
pub struct ClusterResolver {
    clusters: Arc<ArcSwap<HashMap<String, Arc<ClusterState>>>>,
    /// Per-cluster round-robin cursor, keyed by cluster name.
    rr_cursors: dashmap::DashMap<String, Arc<RoundRobinCursor>>,
}

impl std::fmt::Debug for ClusterResolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClusterResolver")
            .field("clusters", &self.clusters.load().keys().collect::<Vec<_>>())
            .finish()
    }
}

/// Per-cluster round-robin cursor.  Boxed separately from `RoundRobin` so
/// the resolver can hand out `Arc` handles without forcing `RoundRobin`
/// itself to be `Clone`.
#[derive(Debug)]
struct RoundRobinCursor {
    inner: RoundRobin,
    /// Counter used by `RoundRobin::pick` when no healthy endpoint check is
    /// required (defence in depth; `RoundRobin` already owns an atomic).
    _generation: AtomicUsize,
}

impl Default for ClusterResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl ClusterResolver {
    /// Create an empty resolver.
    pub fn new() -> Self {
        Self {
            clusters: Arc::new(ArcSwap::from_pointee(HashMap::new())),
            rr_cursors: dashmap::DashMap::new(),
        }
    }

    /// Replace the routing state for `cluster`.  Hot-reload safe.
    ///
    /// The swap is atomic: readers either see the previous `ClusterState`
    /// for `cluster` or the new one, never a torn read.  The round-robin
    /// cursor for `cluster` is preserved so dispatch fairness survives
    /// reload.
    pub fn update(&self, cluster: &str, state: ClusterState) {
        let state = Arc::new(state);

        let current = self.clusters.load();
        let mut next = HashMap::with_capacity(current.len() + 1);
        for (k, v) in current.iter() {
            next.insert(k.clone(), v.clone());
        }
        next.insert(cluster.to_string(), state);
        self.clusters.store(Arc::new(next));

        // Ensure a cursor exists (does not reset an existing one).
        self.rr_cursors
            .entry(cluster.to_string())
            .or_insert_with(|| {
                Arc::new(RoundRobinCursor {
                    inner: RoundRobin::new(),
                    _generation: AtomicUsize::new(0),
                })
            });

        debug!(cluster, "ClusterResolver: state updated");
    }

    /// Resolve `cluster` to a concrete peer.
    ///
    /// Returns `None` when:
    /// - the cluster is unknown,
    /// - no healthy endpoint is available, or
    /// - the cluster requires mTLS but `expected_spiffe_id` is `None`
    ///   (config error — logged at `error!` level).
    ///
    /// The returned `pool_key` is:
    /// - `PoolKey::Plain(addr)` when `tls_required == false`,
    /// - `PoolKey::Mtls(addr, spiffe_id)` when both `tls_required == true`
    ///   and `expected_spiffe_id == Some(id)`.
    pub fn resolve(&self, cluster: &str) -> Option<ResolvedPeer> {
        let snapshot = self.clusters.load();
        let state = snapshot.get(cluster)?.clone();

        // SECURITY: TLS required but no expected peer SPIFFE ID → refuse to
        // route rather than build a `PoolKey::Plain` (which would fall back
        // to the plaintext bucket and leak the request to an unauthenticated
        // peer).
        if state.tls_required && state.expected_spiffe_id.is_none() {
            error!(
                cluster,
                "ClusterResolver: tls_required=true but expected_spiffe_id is None — \
                 refusing to resolve (config error)"
            );
            return None;
        }

        // Pick an endpoint using the cluster's policy.
        let picked = match state.lb_policy {
            LbPolicy::RoundRobin => {
                let cursor = self
                    .rr_cursors
                    .entry(cluster.to_string())
                    .or_insert_with(|| {
                        Arc::new(RoundRobinCursor {
                            inner: RoundRobin::new(),
                            _generation: AtomicUsize::new(0),
                        })
                    })
                    .clone();
                cursor.inner.pick(&state.endpoints).cloned()
            }
            LbPolicy::Weighted | LbPolicy::PowerOfTwoChoices => {
                // TODO(#103): fall through to RR until these policies are
                // implemented in the M2 follow-up.  For now we preserve
                // fairness by routing via the existing RR cursor.
                warn!(
                    cluster,
                    policy = ?state.lb_policy,
                    "ClusterResolver: policy not yet implemented — falling back to round-robin"
                );
                let cursor = self
                    .rr_cursors
                    .entry(cluster.to_string())
                    .or_insert_with(|| {
                        Arc::new(RoundRobinCursor {
                            inner: RoundRobin::new(),
                            _generation: AtomicUsize::new(0),
                        })
                    })
                    .clone();
                cursor.inner.pick(&state.endpoints).cloned()
            }
        };

        let endpoint = picked?;

        // Parse `address:port` into a `SocketAddr`.  xDS already validates
        // the IP form on ingress, so a parse failure here is a bug — log
        // and return `None` rather than panic.
        let addr: SocketAddr =
            match format!("{}:{}", endpoint.address, endpoint.port).parse() {
                Ok(a) => a,
                Err(e) => {
                    error!(
                        cluster,
                        address = %endpoint.address,
                        port = endpoint.port,
                        error = %e,
                        "ClusterResolver: endpoint address failed to parse as SocketAddr"
                    );
                    return None;
                }
            };

        let pool_key = if state.tls_required {
            // The `None` case was handled above; `expect` here is sound.
            let spiffe = state
                .expected_spiffe_id
                .as_ref()
                .expect("tls_required=true was validated above")
                .clone();
            PoolKey::Mtls(addr, spiffe)
        } else {
            PoolKey::Plain(addr)
        };

        Some(ResolvedPeer {
            addr,
            pool_key,
            tls: state.tls_required,
        })
    }

    /// Return the number of registered clusters.
    pub fn len(&self) -> usize {
        self.clusters.load().len()
    }

    /// Return `true` when no cluster is registered.
    pub fn is_empty(&self) -> bool {
        self.clusters.load().is_empty()
    }
}

// -- tests --

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    fn hash_of<H: Hash>(v: &H) -> u64 {
        let mut h = DefaultHasher::new();
        v.hash(&mut h);
        h.finish()
    }

    fn ep(host: &str, port: u16, healthy: bool) -> Endpoint {
        Endpoint {
            address: host.to_string(),
            port,
            weight: 1,
            healthy,
        }
    }

    // -- PoolKey regressions (bug_006) --------------------------------------

    /// Plaintext and mTLS keys for the same address must never collide.
    /// Regression guard for the crypto-downgrade attack in bug_006.
    #[test]
    fn pool_key_plain_vs_mtls_distinct_hash() {
        let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
        let plain = PoolKey::Plain(addr);
        let mtls = PoolKey::Mtls(addr, Arc::from("spiffe://faso.gov.bf/ns/kaya/sa/shard-0"));

        assert_ne!(plain, mtls, "Plain and Mtls must compare unequal");
        assert_ne!(
            hash_of(&plain),
            hash_of(&mtls),
            "Plain and Mtls must hash into different buckets"
        );

        // HashMap sanity: both keys must coexist with distinct values.
        let mut map = HashMap::new();
        map.insert(plain.clone(), "plain-conn");
        map.insert(mtls.clone(), "mtls-conn");
        assert_eq!(map.get(&plain), Some(&"plain-conn"));
        assert_eq!(map.get(&mtls), Some(&"mtls-conn"));
        assert_eq!(map.len(), 2);
    }

    /// mTLS keys sharing an address but differing on SPIFFE ID must never
    /// collide.  Regression guard for the cross-identity impersonation
    /// attack in bug_006.
    #[test]
    fn pool_key_mtls_different_spiffe_distinct_hash() {
        let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
        let k1 = PoolKey::Mtls(addr, Arc::from("spiffe://faso.gov.bf/ns/a/sa/x"));
        let k2 = PoolKey::Mtls(addr, Arc::from("spiffe://faso.gov.bf/ns/b/sa/y"));

        assert_ne!(k1, k2, "same addr + distinct SPIFFE ID must be unequal");
        assert_ne!(
            hash_of(&k1),
            hash_of(&k2),
            "distinct SPIFFE IDs must hash into different buckets"
        );

        let mut map = HashMap::new();
        map.insert(k1.clone(), "ns-a");
        map.insert(k2.clone(), "ns-b");
        assert_eq!(map.len(), 2, "two distinct mTLS keys must coexist");
        assert_eq!(map.get(&k1), Some(&"ns-a"));
        assert_eq!(map.get(&k2), Some(&"ns-b"));
    }

    /// Equality of `Arc<str>` is by contents, not pointer identity.  Two
    /// separately-allocated Arcs holding the same SPIFFE ID must collapse
    /// into a single HashMap entry.
    #[test]
    fn pool_key_mtls_same_spiffe_collides() {
        let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
        let k1 = PoolKey::Mtls(addr, Arc::from("spiffe://faso.gov.bf/ns/a/sa/x"));
        let k2 = PoolKey::Mtls(addr, Arc::from("spiffe://faso.gov.bf/ns/a/sa/x"));

        assert_eq!(k1, k2);
        assert_eq!(hash_of(&k1), hash_of(&k2));
    }

    #[test]
    fn pool_key_addr_accessor_works_for_both_variants() {
        let addr: SocketAddr = "10.0.0.1:9000".parse().unwrap();
        let plain = PoolKey::Plain(addr);
        let mtls = PoolKey::Mtls(addr, Arc::from("spiffe://x/y"));
        assert_eq!(plain.addr(), addr);
        assert_eq!(mtls.addr(), addr);
        assert_eq!(plain.spiffe_id(), None);
        assert_eq!(mtls.spiffe_id(), Some("spiffe://x/y"));
    }

    // -- ClusterResolver -----------------------------------------------------

    #[test]
    fn resolver_plain_cluster_returns_plain_pool_key() {
        let r = ClusterResolver::new();
        r.update(
            "api",
            ClusterState::new(
                vec![ep("127.0.0.1", 8080, true)],
                false,
                None,
                LbPolicy::RoundRobin,
            ),
        );

        let resolved = r.resolve("api").expect("must resolve");
        assert_eq!(resolved.addr, "127.0.0.1:8080".parse().unwrap());
        assert!(!resolved.tls);
        match resolved.pool_key {
            PoolKey::Plain(a) => assert_eq!(a, "127.0.0.1:8080".parse().unwrap()),
            PoolKey::Mtls(_, _) => panic!("plaintext cluster must yield PoolKey::Plain"),
        }
    }

    #[test]
    fn resolver_mtls_cluster_returns_mtls_pool_key_with_spiffe() {
        let r = ClusterResolver::new();
        let spiffe: Arc<str> = Arc::from("spiffe://faso.gov.bf/ns/kaya/sa/shard-0");
        r.update(
            "kaya",
            ClusterState::new(
                vec![ep("127.0.0.1", 6380, true)],
                true,
                Some(spiffe.clone()),
                LbPolicy::RoundRobin,
            ),
        );

        let resolved = r.resolve("kaya").expect("must resolve");
        assert_eq!(resolved.addr, "127.0.0.1:6380".parse().unwrap());
        assert!(resolved.tls);
        match resolved.pool_key {
            PoolKey::Mtls(a, id) => {
                assert_eq!(a, "127.0.0.1:6380".parse().unwrap());
                assert_eq!(id.as_ref(), "spiffe://faso.gov.bf/ns/kaya/sa/shard-0");
                assert!(
                    Arc::ptr_eq(&id, &spiffe) || id == spiffe,
                    "SPIFFE ID must match the expected value"
                );
            }
            PoolKey::Plain(_) => panic!("mTLS cluster must yield PoolKey::Mtls"),
        }
    }

    /// SECURITY: a cluster with `tls_required=true` but no
    /// `expected_spiffe_id` is a config error.  The resolver must refuse to
    /// route rather than silently fall back to plaintext.
    #[test]
    fn resolver_mtls_without_expected_spiffe_fails() {
        let r = ClusterResolver::new();
        r.update(
            "broken",
            ClusterState::new(
                vec![ep("127.0.0.1", 9000, true)],
                true, // tls_required
                None, // but no SPIFFE ID expected — misconfigured
                LbPolicy::RoundRobin,
            ),
        );

        let resolved = r.resolve("broken");
        assert!(
            resolved.is_none(),
            "resolver must refuse to return a peer when mTLS is required \
             but no SPIFFE ID is configured — got {resolved:?}"
        );
    }

    #[test]
    fn resolver_unknown_cluster_returns_none() {
        let r = ClusterResolver::new();
        assert!(r.resolve("nope").is_none());
    }

    #[test]
    fn resolver_all_unhealthy_endpoints_returns_none() {
        let r = ClusterResolver::new();
        r.update(
            "api",
            ClusterState::new(
                vec![
                    ep("127.0.0.1", 8080, false),
                    ep("127.0.0.2", 8080, false),
                ],
                false,
                None,
                LbPolicy::RoundRobin,
            ),
        );
        assert!(r.resolve("api").is_none());
    }

    #[test]
    fn hot_reload_update_replaces_cluster_state() {
        let r = ClusterResolver::new();

        // First version: single plaintext endpoint.
        r.update(
            "svc",
            ClusterState::new(
                vec![ep("127.0.0.1", 5000, true)],
                false,
                None,
                LbPolicy::RoundRobin,
            ),
        );
        let before = r.resolve("svc").unwrap();
        assert_eq!(before.addr, "127.0.0.1:5000".parse().unwrap());
        assert!(matches!(before.pool_key, PoolKey::Plain(_)));

        // Hot-reload: new endpoint, now mTLS-protected.
        r.update(
            "svc",
            ClusterState::new(
                vec![ep("127.0.0.1", 6000, true)],
                true,
                Some(Arc::from("spiffe://faso.gov.bf/ns/svc/sa/v2")),
                LbPolicy::RoundRobin,
            ),
        );
        let after = r.resolve("svc").unwrap();
        assert_eq!(after.addr, "127.0.0.1:6000".parse().unwrap());
        assert!(after.tls);
        match after.pool_key {
            PoolKey::Mtls(_, id) => assert_eq!(id.as_ref(), "spiffe://faso.gov.bf/ns/svc/sa/v2"),
            PoolKey::Plain(_) => panic!("post-reload cluster must be mTLS"),
        }
    }

    #[test]
    fn resolver_round_robin_rotates_across_updates() {
        let r = ClusterResolver::new();
        r.update(
            "api",
            ClusterState::new(
                vec![
                    ep("127.0.0.1", 8001, true),
                    ep("127.0.0.1", 8002, true),
                ],
                false,
                None,
                LbPolicy::RoundRobin,
            ),
        );

        let a = r.resolve("api").unwrap().addr;
        let b = r.resolve("api").unwrap().addr;
        assert_ne!(a, b, "round-robin must alternate between the two endpoints");
    }

    #[test]
    fn resolver_default_is_empty() {
        let r = ClusterResolver::default();
        assert!(r.is_empty());
        assert_eq!(r.len(), 0);
    }
}

// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! In-memory resource cache — the hot-path store for the latest known
//! configuration pushed by the xDS control plane.
//!
//! `ResourceCache` is updated by `AdsClient` on each ACK'd response.
//! Downstream ARMAGEDDON subsystems (LB, mesh, veil) read from it via
//! `ArcSwap` — zero-copy snapshot without holding a lock across `.await`.

use arc_swap::ArcSwap;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

use crate::proto::{
    cluster::Cluster,
    endpoint::ClusterLoadAssignment,
    listener::Listener,
    route::RouteConfiguration,
    tls::Secret,
};

/// Snapshot of all currently-known xDS resources.
///
/// Callers obtain an `Arc<ResourceSnapshot>` via `ResourceCache::load()`.
/// This is lock-free on the read path.
#[derive(Debug, Default, Clone)]
pub struct ResourceSnapshot {
    pub clusters: HashMap<String, Cluster>,
    pub endpoints: HashMap<String, ClusterLoadAssignment>,
    pub listeners: HashMap<String, Listener>,
    pub routes: HashMap<String, RouteConfiguration>,
    pub secrets: HashMap<String, Secret>,
}

/// Thread-safe store for xDS resources, updated by the ADS consumer.
///
/// Uses `ArcSwap` for atomic snapshot replacement.  Reads are always
/// wait-free.  Writes acquire an internal `RwLock` only to produce the
/// new snapshot; the lock is never held across an `.await`.
#[derive(Debug)]
pub struct ResourceCache {
    snapshot: ArcSwap<ResourceSnapshot>,
    /// Write-side lock protects the mutation of the next snapshot.
    write_lock: RwLock<()>,
}

impl Default for ResourceCache {
    fn default() -> Self {
        Self {
            snapshot: ArcSwap::from_pointee(ResourceSnapshot::default()),
            write_lock: RwLock::new(()),
        }
    }
}

impl ResourceCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Load the current snapshot (zero-copy, wait-free).
    pub fn load(&self) -> Arc<ResourceSnapshot> {
        self.snapshot.load_full()
    }

    /// Replace the entire clusters map atomically.
    pub fn update_clusters(&self, clusters: HashMap<String, Cluster>) {
        let _guard = self.write_lock.write();
        let prev = self.snapshot.load();
        let mut next = (**prev).clone();
        next.clusters = clusters;
        self.snapshot.store(Arc::new(next));
    }

    /// Replace the entire endpoints map atomically.
    pub fn update_endpoints(&self, endpoints: HashMap<String, ClusterLoadAssignment>) {
        let _guard = self.write_lock.write();
        let prev = self.snapshot.load();
        let mut next = (**prev).clone();
        next.endpoints = endpoints;
        self.snapshot.store(Arc::new(next));
    }

    /// Replace the entire listeners map atomically.
    pub fn update_listeners(&self, listeners: HashMap<String, Listener>) {
        let _guard = self.write_lock.write();
        let prev = self.snapshot.load();
        let mut next = (**prev).clone();
        next.listeners = listeners;
        self.snapshot.store(Arc::new(next));
    }

    /// Replace the entire routes map atomically.
    pub fn update_routes(&self, routes: HashMap<String, RouteConfiguration>) {
        let _guard = self.write_lock.write();
        let prev = self.snapshot.load();
        let mut next = (**prev).clone();
        next.routes = routes;
        self.snapshot.store(Arc::new(next));
    }

    /// Replace the entire secrets map atomically.
    pub fn update_secrets(&self, secrets: HashMap<String, Secret>) {
        let _guard = self.write_lock.write();
        let prev = self.snapshot.load();
        let mut next = (**prev).clone();
        next.secrets = secrets;
        self.snapshot.store(Arc::new(next));
    }
}

// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Shared state injected into every Axum handler via `axum::extract::State`.

use arc_swap::ArcSwap;
use armageddon_common::types::Cluster;
use armageddon_config::GatewayConfig;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::stats::StatsRegistry;

// -- cluster breaker registry --

/// Runtime state tracked for a single cluster.
#[derive(Debug)]
pub struct ClusterRuntimeState {
    /// Cluster definition (static part from config).
    pub cluster: Cluster,
    /// Whether this cluster is currently being drained.
    pub draining: AtomicBool,
}

impl ClusterRuntimeState {
    /// Build initial state from the cluster config.
    pub fn new(cluster: Cluster) -> Self {
        Self {
            cluster,
            draining: AtomicBool::new(false),
        }
    }

    /// Mark the cluster as draining (idempotent).
    pub fn drain(&self) {
        self.draining.store(true, Ordering::SeqCst);
        tracing::info!(cluster = %self.cluster.name, "cluster drain initiated");
    }

    pub fn is_draining(&self) -> bool {
        self.draining.load(Ordering::SeqCst)
    }
}

/// JSON-serializable view of a cluster's runtime state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterView {
    pub name: String,
    pub endpoints: Vec<EndpointView>,
    pub draining: bool,
    pub circuit_breaker: CircuitBreakerView,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointView {
    pub address: String,
    pub port: u16,
    pub weight: u32,
    pub healthy: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerView {
    pub max_connections: u32,
    pub max_pending_requests: u32,
    pub max_requests: u32,
    pub max_retries: u32,
}

impl From<&ClusterRuntimeState> for ClusterView {
    fn from(state: &ClusterRuntimeState) -> Self {
        Self {
            name: state.cluster.name.clone(),
            endpoints: state
                .cluster
                .endpoints
                .iter()
                .map(|e| EndpointView {
                    address: e.address.clone(),
                    port: e.port,
                    weight: e.weight,
                    healthy: e.healthy,
                })
                .collect(),
            draining: state.is_draining(),
            circuit_breaker: CircuitBreakerView {
                max_connections: state.cluster.circuit_breaker.max_connections,
                max_pending_requests: state.cluster.circuit_breaker.max_pending_requests,
                max_requests: state.cluster.circuit_breaker.max_requests,
                max_retries: state.cluster.circuit_breaker.max_retries,
            },
        }
    }
}

/// Registry of per-cluster runtime state.
pub struct ClusterBreakerRegistry {
    pub states: DashMap<String, Arc<ClusterRuntimeState>>,
}

impl ClusterBreakerRegistry {
    pub fn new(clusters: Vec<Cluster>) -> Arc<Self> {
        let states: DashMap<String, Arc<ClusterRuntimeState>> = DashMap::new();
        for cluster in clusters {
            states.insert(
                cluster.name.clone(),
                Arc::new(ClusterRuntimeState::new(cluster)),
            );
        }
        Arc::new(Self { states })
    }

    /// Rebuild registry from a new cluster list (called after config reload).
    pub fn refresh(&self, clusters: &[Cluster]) {
        // Remove entries no longer in config.
        self.states
            .retain(|k, _| clusters.iter().any(|c| &c.name == k));
        // Add new entries, preserve existing draining state.
        for cluster in clusters {
            if !self.states.contains_key(&cluster.name) {
                self.states.insert(
                    cluster.name.clone(),
                    Arc::new(ClusterRuntimeState::new(cluster.clone())),
                );
            }
        }
    }

    pub fn all_views(&self) -> Vec<ClusterView> {
        self.states
            .iter()
            .map(|entry: dashmap::mapref::multiple::RefMulti<String, Arc<ClusterRuntimeState>>| {
                ClusterView::from(entry.value().as_ref())
            })
            .collect()
    }

    pub fn get(&self, name: &str) -> Option<Arc<ClusterRuntimeState>> {
        self.states.get(name).map(|v| Arc::clone(&v))
    }
}

// -- admin state --

/// Shared mutable state for the Admin HTTP server.
///
/// Wrapped in `Arc` and injected via `axum::extract::State<Arc<AdminState>>`.
pub struct AdminState {
    /// Hot-swappable gateway configuration.
    pub config_store: Arc<ArcSwap<GatewayConfig>>,
    /// Path from which the config was originally loaded (used for reload).
    pub config_path: Arc<parking_lot::Mutex<String>>,
    /// Prometheus metrics registry wrapper.
    pub stats: Arc<StatsRegistry>,
    /// Per-cluster runtime state (draining, breaker view).
    pub cluster_breakers: Arc<ClusterBreakerRegistry>,
}

impl AdminState {
    /// Build initial state from a gateway config path.
    pub fn new(config: GatewayConfig, config_path: String) -> Arc<Self> {
        Self::with_stats(config, config_path, StatsRegistry::new())
    }

    /// Build initial state with a caller-supplied [`StatsRegistry`].
    ///
    /// Used by `armageddon` to share the same `prometheus::Registry`
    /// between the gateway data-plane and the admin server, and by tests
    /// that need to inject a registry containing known metrics.
    pub fn with_stats(
        config: GatewayConfig,
        config_path: String,
        stats: Arc<StatsRegistry>,
    ) -> Arc<Self> {
        let cluster_breakers =
            ClusterBreakerRegistry::new(config.clusters.clone());
        Arc::new(Self {
            config_store: Arc::new(ArcSwap::from_pointee(config)),
            config_path: Arc::new(parking_lot::Mutex::new(config_path)),
            stats,
            cluster_breakers,
        })
    }
}

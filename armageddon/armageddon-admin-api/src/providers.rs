// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Provider traits exposed by the admin API.
//!
//! Other ARMAGEDDON crates implement these traits to feed data into the
//! admin API. The admin API itself carries only a single light dependency
//! on `armageddon-common`; real wiring happens in the main binary crate.
//!
//! Empty `Null*` implementations are provided so the crate builds and can
//! be used stand-alone in tests.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A single upstream endpoint exposed by `/clusters`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointSnapshot {
    pub address: String,
    pub port: u16,
    pub weight: u32,
    pub health: EndpointHealth,
    pub active_connections: u64,
    pub total_requests: u64,
}

/// Per-endpoint health state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EndpointHealth {
    #[serde(rename = "HEALTHY")]
    Healthy,
    #[serde(rename = "UNHEALTHY")]
    Unhealthy,
    #[serde(rename = "OUTLIER_EJECTED")]
    OutlierEjected,
    #[serde(rename = "UNKNOWN")]
    Unknown,
}

/// State of a circuit breaker attached to a cluster.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CircuitBreakerState {
    #[serde(rename = "CLOSED")]
    Closed,
    #[serde(rename = "OPEN")]
    Open,
    #[serde(rename = "HALF_OPEN")]
    HalfOpen,
}

/// A cluster snapshot exposed by `/clusters`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterSnapshot {
    pub name: String,
    pub endpoints: Vec<EndpointSnapshot>,
    pub circuit_breaker: CircuitBreakerState,
    pub active_connections: u64,
    pub total_upstream_rq_total: u64,
}

/// A single listener snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListenerSnapshot {
    pub name: String,
    pub address: String,
    pub port: u16,
    pub protocol: String,
    pub tls_enabled: bool,
    pub tls_mode: Option<String>,
}

/// Result of a health aggregation across engines.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub status: &'static str,
    pub components: BTreeMap<String, ComponentHealth>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    pub status: String,
    pub details: Option<String>,
}

// -- traits --

/// Provider for Prometheus-style stats text and JSON tree.
#[async_trait]
pub trait StatsProvider: Send + Sync + 'static {
    /// Return a Prometheus text exposition of all ARMAGEDDON metrics.
    async fn prometheus_text(&self) -> String;

    /// Return a hierarchical JSON tree of metrics (flat fallback is fine).
    async fn prometheus_json(&self) -> serde_json::Value;
}

/// Provider for cluster / endpoint snapshots.
#[async_trait]
pub trait ClusterProvider: Send + Sync + 'static {
    async fn clusters(&self) -> Vec<ClusterSnapshot>;
    async fn listeners(&self) -> Vec<ListenerSnapshot>;
}

/// Provider for configuration dumps (YAML + JSON).
#[async_trait]
pub trait ConfigDumper: Send + Sync + 'static {
    /// Dump current configuration as JSON (safe to redact secrets).
    async fn dump_json(&self) -> serde_json::Value;

    /// Dump current configuration as YAML.
    async fn dump_yaml(&self) -> String;
}

/// Provider for runtime flag / feature-flag inspection.
#[async_trait]
pub trait RuntimeProvider: Send + Sync + 'static {
    async fn runtime_flags(&self) -> BTreeMap<String, serde_json::Value>;
}

/// Provider for aggregated health.
#[async_trait]
pub trait HealthProvider: Send + Sync + 'static {
    async fn aggregated_health(&self) -> HealthStatus;
}

// -- default / null implementations --

/// No-op `StatsProvider` returning an empty exposition.
///
/// Useful for tests and as a placeholder until the real Prometheus
/// registry is wired in.
#[derive(Default, Clone, Copy)]
pub struct NullStatsProvider;

#[async_trait]
impl StatsProvider for NullStatsProvider {
    async fn prometheus_text(&self) -> String {
        // Return a minimal but valid Prometheus exposition.
        "# ARMAGEDDON admin-api: no StatsProvider wired in yet.\n".to_string()
    }

    async fn prometheus_json(&self) -> serde_json::Value {
        serde_json::json!({})
    }
}

/// No-op `ClusterProvider` returning empty lists.
#[derive(Default, Clone, Copy)]
pub struct NullClusterProvider;

#[async_trait]
impl ClusterProvider for NullClusterProvider {
    async fn clusters(&self) -> Vec<ClusterSnapshot> {
        Vec::new()
    }

    async fn listeners(&self) -> Vec<ListenerSnapshot> {
        Vec::new()
    }
}

/// No-op `ConfigDumper` returning empty dumps.
#[derive(Default, Clone, Copy)]
pub struct NullConfigDumper;

#[async_trait]
impl ConfigDumper for NullConfigDumper {
    async fn dump_json(&self) -> serde_json::Value {
        serde_json::json!({ "note": "no ConfigDumper wired in yet" })
    }

    async fn dump_yaml(&self) -> String {
        "# ARMAGEDDON admin-api: no ConfigDumper wired in yet.\n".to_string()
    }
}

/// No-op `RuntimeProvider` returning an empty flag map.
#[derive(Default, Clone, Copy)]
pub struct NullRuntimeProvider;

#[async_trait]
impl RuntimeProvider for NullRuntimeProvider {
    async fn runtime_flags(&self) -> BTreeMap<String, serde_json::Value> {
        BTreeMap::new()
    }
}

/// No-op `HealthProvider` returning `SERVING`.
#[derive(Default, Clone, Copy)]
pub struct NullHealthProvider;

#[async_trait]
impl HealthProvider for NullHealthProvider {
    async fn aggregated_health(&self) -> HealthStatus {
        HealthStatus {
            status: "SERVING",
            components: BTreeMap::new(),
        }
    }
}

// -- Shadow mode provider ---------------------------------------------------

/// Snapshot returned by `GET /admin/shadow/state`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowStateSnapshot {
    /// Current sample rate as an integer percentage (0–100).
    pub sample_rate: u32,
    /// Total number of times the gate has tripped since startup.
    pub gate_tripped_count: u64,
    /// Last observed divergence rate (fraction 0.0–1.0).
    pub last_divergence_rate: f64,
    /// Number of samples in the last evaluated window.
    pub window_samples: u64,
    /// Whether the gate is currently enabled.
    pub gate_enabled: bool,
    /// Current gate `max_divergence_rate` threshold (fraction 0.0–1.0).
    pub gate_max_divergence_rate: f64,
}

/// Provider for shadow-mode runtime state.
///
/// Implementations wrap the `ShadowSampler` + `ShadowGateState` from
/// `armageddon-forge`. The null impl returns safe defaults.
#[async_trait]
pub trait ShadowProvider: Send + Sync + 'static {
    /// Read current shadow state.
    async fn shadow_state(&self) -> ShadowStateSnapshot;

    /// Update the sample rate (0–100 integer percent). Returns the new rate.
    async fn set_sample_rate(&self, percent: u32) -> u32;

    /// Reconfigure the gate at runtime.
    async fn reconfigure_gate(&self, enabled: Option<bool>, max_divergence_rate: Option<f64>);
}

/// No-op `ShadowProvider` returning static defaults.
#[derive(Default, Clone, Copy)]
pub struct NullShadowProvider;

#[async_trait]
impl ShadowProvider for NullShadowProvider {
    async fn shadow_state(&self) -> ShadowStateSnapshot {
        ShadowStateSnapshot {
            sample_rate: 0,
            gate_tripped_count: 0,
            last_divergence_rate: 0.0,
            window_samples: 0,
            gate_enabled: false,
            gate_max_divergence_rate: 0.02,
        }
    }

    async fn set_sample_rate(&self, percent: u32) -> u32 {
        percent.min(100)
    }

    async fn reconfigure_gate(&self, _enabled: Option<bool>, _max_divergence_rate: Option<f64>) {}
}

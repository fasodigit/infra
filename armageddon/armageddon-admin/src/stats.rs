// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Stats registry: wraps `prometheus::Registry` and exposes JSON / text snapshots.

use parking_lot::RwLock;
use prometheus::{proto::MetricFamily, Encoder, Registry, TextEncoder};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

// -- types --

/// A named counter entry for the JSON dump.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CounterSnapshot {
    pub name: String,
    pub help: String,
    pub value: f64,
    pub labels: HashMap<String, String>,
}

/// Aggregated stats snapshot returned by `GET /admin/stats`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsSnapshot {
    pub counters: Vec<CounterSnapshot>,
    pub raw_families: usize,
}

/// Registry wrapper.
///
/// Holds a `prometheus::Registry` plus a set of manually tracked counters
/// that callers can reset with `reset_all()`.
pub struct StatsRegistry {
    registry: Registry,
    /// Per-name reset values (for soft-reset: record the current value to subtract).
    reset_offsets: RwLock<HashMap<String, f64>>,
}

impl StatsRegistry {
    /// Create a new registry (uses the default global Prometheus registry).
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            registry: prometheus::default_registry().clone(),
            reset_offsets: RwLock::new(HashMap::new()),
        })
    }

    /// Build a `StatsRegistry` from an explicit `Registry` (mostly useful for
    /// tests and for wiring an isolated, non-global registry into the gateway).
    pub fn from_registry(registry: Registry) -> Arc<Self> {
        Arc::new(Self {
            registry,
            reset_offsets: RwLock::new(HashMap::new()),
        })
    }

    /// Produce a JSON snapshot of all current metric families.
    pub fn snapshot_json(&self) -> Value {
        let families = self.gather();
        let offsets = self.reset_offsets.read();
        let mut counters: Vec<CounterSnapshot> = Vec::new();

        for mf in &families {
            for metric in mf.get_metric() {
                // Collect label pairs into a map.
                let labels: HashMap<String, String> = metric
                    .get_label()
                    .iter()
                    .map(|lp| (lp.get_name().to_string(), lp.get_value().to_string()))
                    .collect();

                // We handle counter and gauge families.
                let raw_value = if metric.has_counter() {
                    metric.get_counter().get_value()
                } else if metric.has_gauge() {
                    metric.get_gauge().get_value()
                } else {
                    continue;
                };

                let offset = offsets.get(mf.get_name()).copied().unwrap_or(0.0);
                let value = (raw_value - offset).max(0.0);

                counters.push(CounterSnapshot {
                    name: mf.get_name().to_string(),
                    help: mf.get_help().to_string(),
                    value,
                    labels,
                });
            }
        }

        let snapshot = StatsSnapshot {
            raw_families: families.len(),
            counters,
        };

        serde_json::to_value(&snapshot).unwrap_or(Value::Null)
    }

    /// Produce Prometheus text-format output (for `/admin/stats/prometheus`).
    pub fn snapshot_prometheus_text(&self) -> String {
        let families = self.gather();
        let encoder = TextEncoder::new();
        let mut buf = Vec::new();
        if let Err(e) = encoder.encode(&families, &mut buf) {
            tracing::warn!(error = %e, "prometheus text encode failed");
            return String::new();
        }
        String::from_utf8_lossy(&buf).into_owned()
    }

    /// Encode the registry to Prometheus text exposition format, surfacing
    /// any encoder error so the caller can map it to a 500.
    ///
    /// Used by `/admin/metrics`, the canonical scrape endpoint.
    pub fn encode_prometheus(&self) -> Result<String, prometheus::Error> {
        let families = self.gather();
        let encoder = TextEncoder::new();
        let mut buf = Vec::new();
        encoder.encode(&families, &mut buf)?;
        String::from_utf8(buf).map_err(|e| {
            prometheus::Error::Msg(format!("non-utf8 prometheus output: {e}"))
        })
    }

    /// Reset all counter soft-offsets to the current values so subsequent
    /// reads return 0.
    pub fn reset_counters(&self) {
        let families = self.gather();
        let mut offsets = self.reset_offsets.write();
        offsets.clear();
        for mf in &families {
            for metric in mf.get_metric() {
                let value = if metric.has_counter() {
                    metric.get_counter().get_value()
                } else {
                    continue;
                };
                offsets
                    .entry(mf.get_name().to_string())
                    .and_modify(|v| *v += value)
                    .or_insert(value);
            }
        }
        tracing::info!("admin: stats counters reset");
    }

    // -- private --

    fn gather(&self) -> Vec<MetricFamily> {
        self.registry.gather()
    }
}

impl Default for StatsRegistry {
    fn default() -> Self {
        Self {
            registry: prometheus::default_registry().clone(),
            reset_offsets: RwLock::new(HashMap::new()),
        }
    }
}

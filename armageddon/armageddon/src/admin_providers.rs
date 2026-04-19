// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Real provider implementations plugged into `armageddon-admin-api`.
//!
//! This module wires the admin-api provider traits to live ARMAGEDDON
//! subsystems (Prometheus registry, Forge circuit-breakers / health /
//! clusters, Pentagon engines, gateway config).  Each impl is deliberately
//! small: the heavy lifting lives in the backing crates.

use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use prometheus::{Registry, TextEncoder};

use armageddon_admin_api::providers::{
    CircuitBreakerState, ClusterProvider, ClusterSnapshot, ComponentHealth, ConfigDumper,
    EndpointHealth, EndpointSnapshot, HealthProvider, HealthStatus, ListenerSnapshot,
    RuntimeProvider, StatsProvider,
};
use armageddon_config::ArmageddonConfig;
use armageddon_forge::circuit_breaker::CircuitState;
use armageddon_forge::ForgeServer;

use crate::pipeline::Pentagon;

// ---------------------------------------------------------------------------
// Stats provider — wraps a Prometheus registry.
// ---------------------------------------------------------------------------

/// `StatsProvider` backed by a Prometheus `Registry`.
///
/// When the gateway uses the process-wide `prometheus::default_registry()`,
/// pass a clone of that via `default_registry()`.  When a dedicated
/// `Registry` is constructed in `main.rs`, pass it wrapped in `Arc`.
pub struct ForgePrometheusStatsProvider {
    /// The explicit registry used by code that calls `registry.register(...)`.
    /// `None` means "use `prometheus::default_registry()`".
    registry: Option<Arc<Registry>>,
}

impl ForgePrometheusStatsProvider {
    /// Construct a provider that reads from an explicit registry.
    pub fn from_registry(registry: Arc<Registry>) -> Self {
        Self {
            registry: Some(registry),
        }
    }

    /// Construct a provider that reads from `prometheus::default_registry()`.
    ///
    /// Most ARMAGEDDON crates use the `register_*!` macros which target the
    /// default registry, so this is the right choice when no dedicated
    /// registry is threaded through the binary.
    pub fn default_registry() -> Self {
        Self { registry: None }
    }

    fn metric_families(&self) -> Vec<prometheus::proto::MetricFamily> {
        match &self.registry {
            Some(r) => r.gather(),
            None => prometheus::default_registry().gather(),
        }
    }
}

#[async_trait]
impl StatsProvider for ForgePrometheusStatsProvider {
    async fn prometheus_text(&self) -> String {
        let families = self.metric_families();
        let encoder = TextEncoder::new();
        encoder.encode_to_string(&families).unwrap_or_else(|e| {
            format!("# ARMAGEDDON admin-api: encoding error: {e}\n")
        })
    }

    async fn prometheus_json(&self) -> serde_json::Value {
        use prometheus::proto::MetricType;

        let families = self.metric_families();
        let mut out = serde_json::Map::with_capacity(families.len());

        for family in families {
            let name = family.get_name().to_string();
            let help = family.get_help().to_string();
            let kind = match family.get_field_type() {
                MetricType::COUNTER => "counter",
                MetricType::GAUGE => "gauge",
                MetricType::HISTOGRAM => "histogram",
                MetricType::SUMMARY => "summary",
                MetricType::UNTYPED => "untyped",
            };

            let mut metrics = Vec::with_capacity(family.get_metric().len());
            for m in family.get_metric() {
                let labels: BTreeMap<String, String> = m
                    .get_label()
                    .iter()
                    .map(|l| (l.get_name().to_string(), l.get_value().to_string()))
                    .collect();

                let value = if m.has_counter() {
                    serde_json::json!(m.get_counter().get_value())
                } else if m.has_gauge() {
                    serde_json::json!(m.get_gauge().get_value())
                } else if m.has_histogram() {
                    let h = m.get_histogram();
                    serde_json::json!({
                        "sample_count": h.get_sample_count(),
                        "sample_sum": h.get_sample_sum(),
                    })
                } else if m.has_summary() {
                    let s = m.get_summary();
                    serde_json::json!({
                        "sample_count": s.get_sample_count(),
                        "sample_sum": s.get_sample_sum(),
                    })
                } else if m.has_untyped() {
                    serde_json::json!(m.get_untyped().get_value())
                } else {
                    serde_json::Value::Null
                };

                metrics.push(serde_json::json!({
                    "labels": labels,
                    "value": value,
                }));
            }

            out.insert(
                name,
                serde_json::json!({
                    "help": help,
                    "type": kind,
                    "metrics": metrics,
                }),
            );
        }

        serde_json::Value::Object(out)
    }
}

// ---------------------------------------------------------------------------
// Cluster provider — reads ForgeServer cluster / breaker / health state.
// ---------------------------------------------------------------------------

/// `ClusterProvider` backed by a live `ForgeServer`.
///
/// Reads the static cluster list from Forge, queries per-endpoint health
/// via `HealthManager`, circuit-breaker state via `CircuitBreakerManager`,
/// and enumerates listener configs from the loaded `GatewayConfig`.
///
/// Per-endpoint `active_connections` and `total_requests` are not tracked
/// per-endpoint today (only cluster-wide via Prometheus metrics), so these
/// values are reported as `0` until the Forge pool grows that accounting.
pub struct RuntimeClusterProvider {
    forge: Arc<ForgeServer>,
    config: Arc<ArmageddonConfig>,
}

impl RuntimeClusterProvider {
    pub fn new(forge: Arc<ForgeServer>, config: Arc<ArmageddonConfig>) -> Self {
        Self { forge, config }
    }
}

#[async_trait]
impl ClusterProvider for RuntimeClusterProvider {
    async fn clusters(&self) -> Vec<ClusterSnapshot> {
        let mut out = Vec::with_capacity(self.forge.clusters().len());

        for cluster in self.forge.clusters().iter() {
            let breaker_state = self
                .forge
                .circuit_breakers()
                .get(&cluster.name)
                .map(|b| match b.current_state() {
                    CircuitState::Closed => CircuitBreakerState::Closed,
                    CircuitState::Open => CircuitBreakerState::Open,
                    CircuitState::HalfOpen => CircuitBreakerState::HalfOpen,
                })
                .unwrap_or(CircuitBreakerState::Closed);

            let mut endpoints = Vec::with_capacity(cluster.endpoints.len());
            for ep in &cluster.endpoints {
                let healthy = self
                    .forge
                    .health_manager()
                    .is_healthy(&cluster.name, &ep.address, ep.port);
                let ejected = self
                    .forge
                    .health_manager()
                    .is_ejected(&cluster.name, &ep.address, ep.port);

                let health = if ejected {
                    EndpointHealth::OutlierEjected
                } else if healthy {
                    EndpointHealth::Healthy
                } else {
                    EndpointHealth::Unhealthy
                };

                endpoints.push(EndpointSnapshot {
                    address: ep.address.clone(),
                    port: ep.port,
                    weight: ep.weight,
                    health,
                    // Per-endpoint counters are not tracked yet — TODO: wire
                    // from UpstreamPool once it exposes per-endpoint stats.
                    active_connections: 0,
                    total_requests: 0,
                });
            }

            out.push(ClusterSnapshot {
                name: cluster.name.clone(),
                endpoints,
                circuit_breaker: breaker_state,
                active_connections: 0,
                total_upstream_rq_total: 0,
            });
        }

        out
    }

    async fn listeners(&self) -> Vec<ListenerSnapshot> {
        let mut out = Vec::new();

        // HTTP/1+2 listeners declared in the gateway config.
        for l in &self.config.gateway.listeners {
            let protocol = match l.protocol {
                armageddon_config::gateway::ListenerProtocol::Http => "http",
                armageddon_config::gateway::ListenerProtocol::Https => "https",
                armageddon_config::gateway::ListenerProtocol::Grpc => "grpc",
            };
            out.push(ListenerSnapshot {
                name: l.name.clone(),
                address: l.address.clone(),
                port: l.port,
                protocol: protocol.to_string(),
                tls_enabled: l.tls.is_some(),
                tls_mode: l
                    .tls
                    .as_ref()
                    .map(|t| format!("TLS v{}", t.min_version)),
            });
        }

        // Optional HTTP/3 QUIC listener.
        if let Some(q) = self.config.gateway.quic.as_ref() {
            out.push(ListenerSnapshot {
                name: "http3-quic".to_string(),
                address: q.address.clone(),
                port: q.port,
                protocol: "http3".to_string(),
                tls_enabled: true,
                tls_mode: Some("QUIC (TLS 1.3)".to_string()),
            });
        }

        out
    }
}

// ---------------------------------------------------------------------------
// Config dumper — serialises GatewayConfig with secret redaction.
// ---------------------------------------------------------------------------

/// Substitution value for redacted secret fields.
const REDACTED: &str = "***REDACTED***";

/// Suffixes / substrings which indicate a secret field.
///
/// Matched case-insensitively against the JSON object key.  The
/// `safe_suffixes` list carves out names that *contain* "key" (e.g.
/// `signing_key_id`, `public_key`, `kid`) but are not secrets themselves.
fn is_secret_key(key: &str) -> bool {
    let k = key.to_ascii_lowercase();

    // Explicit allow-list of "contains sensitive word but actually safe" names.
    let safe_substrings = [
        "key_id",
        "public_key",
        "keyring_ref",
        "key_path", // filesystem path, not the key material
        "key_algorithm",
        "key_type",
        "key_size",
    ];
    if safe_substrings.iter().any(|s| k.contains(s)) {
        return false;
    }

    let sensitive_substrings = ["token", "secret", "password", "passwd", "private_key"];
    if sensitive_substrings.iter().any(|s| k.contains(s)) {
        return true;
    }

    // Bare "key" at the end only (covers `jwt_key`, `api_key`, `hmac_key`).
    k == "key" || k.ends_with("_key")
}

/// Recursively redact secret fields in a JSON value.
fn redact(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            for (k, v) in map.iter_mut() {
                if is_secret_key(k) && !v.is_null() {
                    // Only redact when the value is a non-null leaf.
                    if matches!(
                        v,
                        serde_json::Value::String(_)
                            | serde_json::Value::Number(_)
                            | serde_json::Value::Bool(_)
                    ) {
                        *v = serde_json::Value::String(REDACTED.to_string());
                        continue;
                    }
                }
                redact(v);
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr.iter_mut() {
                redact(v);
            }
        }
        _ => {}
    }
}

/// `ConfigDumper` backed by a shared `ArmageddonConfig`.
pub struct GatewayConfigDumper {
    config: Arc<ArmageddonConfig>,
}

impl GatewayConfigDumper {
    pub fn new(config: Arc<ArmageddonConfig>) -> Self {
        Self { config }
    }

    fn redacted_value(&self) -> serde_json::Value {
        let mut v = serde_json::to_value(self.config.as_ref())
            .unwrap_or_else(|_| serde_json::json!({}));
        redact(&mut v);
        v
    }
}

#[async_trait]
impl ConfigDumper for GatewayConfigDumper {
    async fn dump_json(&self) -> serde_json::Value {
        self.redacted_value()
    }

    async fn dump_yaml(&self) -> String {
        let v = self.redacted_value();
        serde_yaml::to_string(&v).unwrap_or_else(|e| {
            format!("# ARMAGEDDON admin-api: YAML encoding error: {e}\n")
        })
    }
}

// ---------------------------------------------------------------------------
// Health provider — aggregates Pentagon engine readiness.
// ---------------------------------------------------------------------------

/// `HealthProvider` that aggregates readiness of the 5 Pentagon engines
/// (plus WASM) via `Pentagon::engine_readiness()`.
///
/// Overall status is `SERVING` iff every sub-component reports ready,
/// `NOT_SERVING` otherwise.
pub struct PentagonHealthProvider {
    pentagon: Arc<Pentagon>,
}

impl PentagonHealthProvider {
    pub fn new(pentagon: Arc<Pentagon>) -> Self {
        Self { pentagon }
    }
}

#[async_trait]
impl HealthProvider for PentagonHealthProvider {
    async fn aggregated_health(&self) -> HealthStatus {
        let readiness = self.pentagon.engine_readiness();
        let mut components = BTreeMap::new();
        let mut all_ok = true;

        for (name, ready) in readiness {
            if !ready {
                all_ok = false;
            }
            components.insert(
                name.to_string(),
                ComponentHealth {
                    status: if ready { "SERVING" } else { "NOT_SERVING" }.to_string(),
                    details: None,
                },
            );
        }

        HealthStatus {
            status: if all_ok { "SERVING" } else { "NOT_SERVING" },
            components,
        }
    }
}

// ---------------------------------------------------------------------------
// Runtime provider — surfaces select gateway feature flags from config.
// ---------------------------------------------------------------------------

/// `RuntimeProvider` that exposes a static snapshot of feature-flag-shaped
/// config booleans.
///
/// `FeatureFlagService` (in `armageddon-forge::feature_flags`) keeps a live
/// cache of GrowthBook flags but does not today publish the flag list for
/// introspection; when it does, swap this for a live snapshot.
pub struct StaticRuntimeProvider {
    snapshot: BTreeMap<String, serde_json::Value>,
}

impl StaticRuntimeProvider {
    pub fn from_config(config: &ArmageddonConfig) -> Self {
        let mut snapshot = BTreeMap::new();
        let g = &config.gateway;
        snapshot.insert(
            "websocket_enabled".to_string(),
            serde_json::json!(g.websocket_enabled),
        );
        snapshot.insert(
            "grpc_web_enabled".to_string(),
            serde_json::json!(g.grpc_web_enabled),
        );
        snapshot.insert(
            "quic_enabled".to_string(),
            serde_json::json!(g.quic.is_some()),
        );
        snapshot.insert(
            "mesh_enabled".to_string(),
            serde_json::json!(g.mesh.is_some()),
        );
        snapshot.insert(
            "xds_consumer_enabled".to_string(),
            serde_json::json!(g.xds_consumer.is_some()),
        );
        snapshot.insert(
            "cache_enabled".to_string(),
            serde_json::json!(g.cache.as_ref().map(|c| c.enabled).unwrap_or(false)),
        );
        snapshot.insert(
            "rate_limit_enabled".to_string(),
            serde_json::json!(g.rate_limit.is_some()),
        );
        snapshot.insert(
            "graphql_limiter_enabled".to_string(),
            serde_json::json!(config.security.graphql_limits.enabled),
        );
        Self { snapshot }
    }
}

#[async_trait]
impl RuntimeProvider for StaticRuntimeProvider {
    async fn runtime_flags(&self) -> BTreeMap<String, serde_json::Value> {
        self.snapshot.clone()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use prometheus::{Counter, Opts};

    #[test]
    fn is_secret_key_behaviour() {
        assert!(is_secret_key("token"));
        assert!(is_secret_key("admin_token"));
        assert!(is_secret_key("password"));
        assert!(is_secret_key("client_secret"));
        assert!(is_secret_key("private_key"));
        assert!(is_secret_key("jwt_key"));

        assert!(!is_secret_key("public_key"));
        assert!(!is_secret_key("signing_key_id"));
        assert!(!is_secret_key("key_path"));
        assert!(!is_secret_key("key_algorithm"));
        assert!(!is_secret_key("name"));
    }

    #[test]
    fn redact_replaces_secret_leaves() {
        let mut v = serde_json::json!({
            "gateway": {
                "jwt": {
                    "admin_token": "supersecret",
                    "public_key": "ssh-rsa AAA...",
                    "signing_key_id": "kid-1",
                    "hmac_key": "hex-bytes"
                },
                "kratos": {
                    "password": "plain",
                    "url": "http://kratos:4433"
                }
            }
        });
        redact(&mut v);

        assert_eq!(v["gateway"]["jwt"]["admin_token"], REDACTED);
        assert_eq!(v["gateway"]["jwt"]["hmac_key"], REDACTED);
        assert_eq!(v["gateway"]["kratos"]["password"], REDACTED);
        // Not redacted — these look sensitive but are not secrets.
        assert_eq!(v["gateway"]["jwt"]["public_key"], "ssh-rsa AAA...");
        assert_eq!(v["gateway"]["jwt"]["signing_key_id"], "kid-1");
        assert_eq!(v["gateway"]["kratos"]["url"], "http://kratos:4433");
    }

    #[tokio::test]
    async fn prometheus_stats_encodes_registered_counter() {
        let registry = Arc::new(Registry::new());
        let c = Counter::with_opts(Opts::new(
            "armageddon_admin_test_counter",
            "Unit-test counter exposed through ForgePrometheusStatsProvider",
        ))
        .expect("counter");
        registry
            .register(Box::new(c.clone()))
            .expect("register in isolated registry");

        c.inc();
        c.inc();

        let provider = ForgePrometheusStatsProvider::from_registry(registry);
        let text = provider.prometheus_text().await;

        assert!(text.contains("# HELP armageddon_admin_test_counter"));
        assert!(text.contains("# TYPE armageddon_admin_test_counter counter"));
        assert!(text.contains("armageddon_admin_test_counter 2"));

        let json = provider.prometheus_json().await;
        assert_eq!(
            json["armageddon_admin_test_counter"]["metrics"][0]["value"],
            2.0
        );
    }
}

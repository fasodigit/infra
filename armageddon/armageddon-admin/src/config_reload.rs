// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Hot-reload logic: parse, validate, ArcSwap, return diff.

use arc_swap::ArcSwap;
use armageddon_config::GatewayConfig;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;

use crate::error::AdminError;

// -- diff types --

/// Human-readable diff summary returned after a successful reload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigDiff {
    /// Number of listeners before / after.
    pub listeners_before: usize,
    pub listeners_after: usize,
    /// Number of clusters before / after.
    pub clusters_before: usize,
    pub clusters_after: usize,
    /// Number of routes before / after.
    pub routes_before: usize,
    pub routes_after: usize,
    /// Cluster names added by the reload.
    pub clusters_added: Vec<String>,
    /// Cluster names removed by the reload.
    pub clusters_removed: Vec<String>,
}

impl ConfigDiff {
    fn compute(before: &GatewayConfig, after: &GatewayConfig) -> Self {
        let before_cluster_names: std::collections::HashSet<&str> =
            before.clusters.iter().map(|c| c.name.as_str()).collect();
        let after_cluster_names: std::collections::HashSet<&str> =
            after.clusters.iter().map(|c| c.name.as_str()).collect();

        let clusters_added = after_cluster_names
            .difference(&before_cluster_names)
            .map(|s| s.to_string())
            .collect();
        let clusters_removed = before_cluster_names
            .difference(&after_cluster_names)
            .map(|s| s.to_string())
            .collect();

        Self {
            listeners_before: before.listeners.len(),
            listeners_after: after.listeners.len(),
            clusters_before: before.clusters.len(),
            clusters_after: after.clusters.len(),
            routes_before: before.routes.len(),
            routes_after: after.routes.len(),
            clusters_added,
            clusters_removed,
        }
    }
}

// -- reload --

/// Read YAML from `path`, parse into [`GatewayConfig`], validate, atomic-swap,
/// and return a [`ConfigDiff`].
///
/// On any parse or validation error the swap is **not** performed and the
/// old configuration remains live.
pub async fn reload(
    path: &Path,
    swap: &Arc<ArcSwap<GatewayConfig>>,
) -> Result<ConfigDiff, AdminError> {
    tracing::info!(path = %path.display(), "admin: config reload requested");

    // Read file (async via spawn_blocking to avoid blocking the executor).
    let path_owned = path.to_path_buf();
    let content = tokio::task::spawn_blocking(move || std::fs::read_to_string(&path_owned))
        .await
        .map_err(|e| AdminError::Validation(format!("spawn_blocking join: {e}")))?
        .map_err(AdminError::ReadFile)?;

    // Parse YAML.
    let new_cfg: GatewayConfig =
        serde_yaml::from_str(&content).map_err(AdminError::Parse)?;

    // Validate (basic sanity checks).
    validate(&new_cfg)?;

    // Compute diff against the currently-live config.
    let current = swap.load_full();
    let diff = ConfigDiff::compute(&current, &new_cfg);

    // Atomic swap — zero downtime.
    swap.store(Arc::new(new_cfg));

    tracing::info!(
        listeners_after = diff.listeners_after,
        clusters_after = diff.clusters_after,
        routes_after = diff.routes_after,
        "admin: config reloaded successfully"
    );

    Ok(diff)
}

/// Minimal validation of a freshly parsed [`GatewayConfig`].
fn validate(cfg: &GatewayConfig) -> Result<(), AdminError> {
    if cfg.listeners.is_empty() {
        return Err(AdminError::Validation(
            "at least one listener is required".to_string(),
        ));
    }
    for listener in &cfg.listeners {
        if listener.name.is_empty() {
            return Err(AdminError::Validation(
                "listener name must not be empty".to_string(),
            ));
        }
        if listener.port == 0 {
            return Err(AdminError::Validation(format!(
                "listener '{}' has port 0 which is invalid",
                listener.name
            )));
        }
    }
    for cluster in &cfg.clusters {
        if cluster.name.is_empty() {
            return Err(AdminError::Validation(
                "cluster name must not be empty".to_string(),
            ));
        }
        if cluster.endpoints.is_empty() {
            return Err(AdminError::Validation(format!(
                "cluster '{}' has no endpoints",
                cluster.name
            )));
        }
    }
    Ok(())
}

// -- tests --

#[cfg(test)]
mod tests {
    use super::*;
    use armageddon_common::types::{
        CircuitBreakerConfig, Cluster, Endpoint, HealthCheckConfig, OutlierDetectionConfig,
        Protocol,
    };
    use armageddon_config::gateway::{
        ExtAuthzConfig, ListenerConfig, ListenerProtocol, XdsEndpoint,
    };
    use armageddon_common::types::{AuthMode, JwtConfig};

    fn minimal_cluster(name: &str) -> Cluster {
        Cluster {
            name: name.to_string(),
            endpoints: vec![Endpoint {
                address: "127.0.0.1".to_string(),
                port: 8080,
                weight: 1,
                healthy: true,
            }],
            health_check: HealthCheckConfig {
                interval_ms: 5000,
                timeout_ms: 2000,
                unhealthy_threshold: 3,
                healthy_threshold: 2,
                protocol: Protocol::Http,
                path: Some("/healthz".to_string()),
            },
            circuit_breaker: CircuitBreakerConfig::default(),
            outlier_detection: OutlierDetectionConfig::default(),
        }
    }

    fn minimal_listener(name: &str, port: u16) -> ListenerConfig {
        ListenerConfig {
            name: name.to_string(),
            address: "0.0.0.0".to_string(),
            port,
            tls: None,
            protocol: ListenerProtocol::Http,
        }
    }

    fn minimal_gateway_config() -> GatewayConfig {
        GatewayConfig {
            runtime: Default::default(),
            listeners: vec![minimal_listener("main", 8080)],
            routes: vec![],
            clusters: vec![minimal_cluster("backend")],
            auth_mode: AuthMode::Jwt,
            jwt: JwtConfig::default(),
            kratos: Default::default(),
            cors: vec![],
            ext_authz: ExtAuthzConfig::default(),
            xds: XdsEndpoint::default(),
            webhooks: Default::default(),
            // Vague 1 fields — all optional/defaulted
            quic: None,
            mesh: None,
            xds_consumer: None,
            lb: Default::default(),
            retry: Default::default(),
            cache: None,
            admin: None,
            admin_api: None,
            websocket_enabled: false,
            grpc_web_enabled: false,
            rate_limit: None,
            waf: None,
            shadow_mode: Default::default(),
        }
    }

    #[test]
    fn test_validate_valid_config() {
        let cfg = minimal_gateway_config();
        assert!(validate(&cfg).is_ok());
    }

    #[test]
    fn test_validate_no_listeners() {
        let mut cfg = minimal_gateway_config();
        cfg.listeners.clear();
        let err = validate(&cfg).unwrap_err();
        assert!(err.to_string().contains("listener"));
    }

    #[test]
    fn test_validate_cluster_no_endpoints() {
        let mut cfg = minimal_gateway_config();
        cfg.clusters[0].endpoints.clear();
        let err = validate(&cfg).unwrap_err();
        assert!(err.to_string().contains("no endpoints"));
    }

    #[test]
    fn test_diff_detects_added_removed() {
        let before = minimal_gateway_config();
        let mut after = before.clone();
        after.clusters.push(minimal_cluster("new-service"));
        after.clusters.retain(|c| c.name != "backend");

        let diff = ConfigDiff::compute(&before, &after);
        assert_eq!(diff.clusters_added, vec!["new-service"]);
        assert_eq!(diff.clusters_removed, vec!["backend"]);
    }

    #[tokio::test]
    async fn test_reload_invalid_yaml_does_not_swap() {
        use std::io::Write;
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        write!(tmp, "{{ invalid yaml: [[[").unwrap();

        let cfg = minimal_gateway_config();
        let swap = Arc::new(ArcSwap::from_pointee(cfg));
        let before_ptr = Arc::as_ptr(&swap.load_full());

        let result = reload(tmp.path(), &swap).await;
        assert!(result.is_err());

        // Config must not have changed.
        let after_ptr = Arc::as_ptr(&swap.load_full());
        assert_eq!(before_ptr, after_ptr);
    }
}


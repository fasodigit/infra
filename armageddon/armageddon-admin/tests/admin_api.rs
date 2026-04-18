// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Integration tests for the ARMAGEDDON Admin API.

use armageddon_admin::{AdminConfig, AdminState};
use armageddon_common::types::{
    AuthMode, CircuitBreakerConfig, Cluster, Endpoint, HealthCheckConfig, JwtConfig,
    OutlierDetectionConfig, Protocol,
};
use armageddon_config::{
    gateway::{ExtAuthzConfig, ListenerConfig, ListenerProtocol, XdsEndpoint},
    GatewayConfig,
};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::Value;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;
use tower::ServiceExt; // for `oneshot`

// -- helpers --

fn minimal_cluster(name: &str) -> Cluster {
    Cluster {
        name: name.to_string(),
        endpoints: vec![Endpoint {
            address: "10.0.0.1".to_string(),
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

fn minimal_gateway_config() -> GatewayConfig {
    GatewayConfig {
        listeners: vec![ListenerConfig {
            name: "main".to_string(),
            address: "0.0.0.0".to_string(),
            port: 8080,
            tls: None,
            protocol: ListenerProtocol::Http,
        }],
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
        websocket_enabled: false,
        grpc_web_enabled: false,
    }
}

fn test_state(cfg: GatewayConfig) -> Arc<AdminState> {
    AdminState::new(cfg, "/tmp/armageddon-test.yaml".to_string())
}

fn build_router_no_auth(state: Arc<AdminState>) -> axum::Router {
    armageddon_admin::routes::build_router(state, AdminConfig::default())
}

fn build_router_with_auth(state: Arc<AdminState>, token: &str) -> axum::Router {
    let cfg = AdminConfig {
        bind_addr: IpAddr::V4(Ipv4Addr::LOCALHOST),
        port: 9901,
        admin_token: Some(token.to_string()),
    };
    armageddon_admin::routes::build_router(state, cfg)
}

// -- tests --

/// Test 1: GET /admin/health returns 200 "OK".
#[tokio::test]
async fn test_health_returns_200() {
    let state = test_state(minimal_gateway_config());
    let router = build_router_no_auth(state);

    let req = Request::builder()
        .method("GET")
        .uri("/admin/health")
        .body(Body::empty())
        .unwrap();

    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(body.as_ref(), b"OK");
}

/// Test 2: GET /admin/clusters returns current cluster state JSON.
#[tokio::test]
async fn test_clusters_returns_state() {
    let state = test_state(minimal_gateway_config());
    let router = build_router_no_auth(state);

    let req = Request::builder()
        .method("GET")
        .uri("/admin/clusters")
        .body(Body::empty())
        .unwrap();

    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    let clusters = json["clusters"].as_array().unwrap();
    assert_eq!(clusters.len(), 1);
    assert_eq!(clusters[0]["name"], "backend");
    assert_eq!(clusters[0]["draining"], false);
}

/// Test 3: POST /admin/clusters/{name}/drain marks the cluster as draining.
#[tokio::test]
async fn test_cluster_drain() {
    let state = test_state(minimal_gateway_config());
    let router = build_router_no_auth(Arc::clone(&state));

    let req = Request::builder()
        .method("POST")
        .uri("/admin/clusters/backend/drain")
        .body(Body::empty())
        .unwrap();

    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Verify the state actually changed.
    let breaker = state.cluster_breakers.get("backend").unwrap();
    assert!(breaker.is_draining());
}

/// Test 4: POST /admin/config/reload with a valid YAML returns 200 + diff.
#[tokio::test]
async fn test_config_reload_valid_yaml() {
    use std::io::Write;

    let cfg = minimal_gateway_config();
    // Serialise the config to a temp file so we can reload it.
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    let yaml = serde_yaml::to_string(&cfg).unwrap();
    write!(tmp, "{}", yaml).unwrap();

    let state = AdminState::new(cfg, tmp.path().to_string_lossy().to_string());
    let router = build_router_no_auth(state);

    let req = Request::builder()
        .method("POST")
        .uri("/admin/config/reload")
        .body(Body::empty())
        .unwrap();

    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "reloaded");
    assert!(json["diff"].is_object());
}

/// Test 5: POST /admin/config/reload with invalid YAML returns 400, config unchanged.
#[tokio::test]
async fn test_config_reload_invalid_yaml_returns_400() {
    use std::io::Write;

    let cfg = minimal_gateway_config();
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    write!(tmp, "{{ invalid yaml: [[[broken").unwrap();

    let state = AdminState::new(cfg, tmp.path().to_string_lossy().to_string());
    let router = build_router_no_auth(state);

    let req = Request::builder()
        .method("POST")
        .uri("/admin/config/reload")
        .body(Body::empty())
        .unwrap();

    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert!(json["error"].as_str().is_some());
}

/// Test 6: POST /admin/config/reload without X-Admin-Token when auth is enabled → 401.
#[tokio::test]
async fn test_config_reload_missing_token_returns_401() {
    let state = test_state(minimal_gateway_config());
    let router = build_router_with_auth(state, "super-secret");

    let req = Request::builder()
        .method("POST")
        .uri("/admin/config/reload")
        .body(Body::empty())
        .unwrap();

    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

/// Test 7: POST /admin/config/reload WITH correct token → auth succeeds (does not 401).
/// (The reload itself may fail because the path is not set to a valid file, but that
/// is a 400, not 401.)
#[tokio::test]
async fn test_config_reload_correct_token_passes_auth() {
    let state = test_state(minimal_gateway_config());
    let router = build_router_with_auth(Arc::clone(&state), "my-token");

    let req = Request::builder()
        .method("POST")
        .uri("/admin/config/reload")
        .header("X-Admin-Token", "my-token")
        .body(Body::empty())
        .unwrap();

    let resp = router.oneshot(req).await.unwrap();
    // Could be 400 (file not found) but must NOT be 401.
    assert_ne!(resp.status(), StatusCode::UNAUTHORIZED);
}

/// Test 8: Loopback bind check — AdminConfig with non-loopback bind_addr is accepted
/// by the struct but the server emits a warning. We verify the struct accepts it and
/// that a loopback config is the default.
#[tokio::test]
async fn test_default_admin_config_is_loopback() {
    let cfg = AdminConfig::default();
    assert!(cfg.bind_addr.is_loopback(),
        "default bind_addr must be loopback, got {}", cfg.bind_addr);
}

/// Test 9: GET /admin/stats returns valid JSON.
#[tokio::test]
async fn test_stats_returns_json() {
    let state = test_state(minimal_gateway_config());
    let router = build_router_no_auth(state);

    let req = Request::builder()
        .method("GET")
        .uri("/admin/stats")
        .body(Body::empty())
        .unwrap();

    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    // Must be valid JSON.
    let _json: Value = serde_json::from_slice(&body).unwrap();
}

/// Test 10: POST /admin/reset_counters returns 200 with auth disabled.
#[tokio::test]
async fn test_reset_counters() {
    let state = test_state(minimal_gateway_config());
    let router = build_router_no_auth(state);

    let req = Request::builder()
        .method("POST")
        .uri("/admin/reset_counters")
        .body(Body::empty())
        .unwrap();

    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "counters_reset");
}
